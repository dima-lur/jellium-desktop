//! Windows menus — native Win32 popups tracked on the input thread.
//!
//! `TrackPopupMenuEx` blocks its caller for the whole life of the menu, so a
//! request is parked here and handed to the input thread through a posted
//! message; the CEF UI thread that opened it returns immediately.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, Ordering};

use jfn_platform_abi::{MenuHost, MenuRequest};
use parking_lot::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, EndMenu, HMENU, MENU_ITEM_FLAGS, MF_GRAYED,
    MF_SEPARATOR, MF_STRING, PostMessageW, SetForegroundWindow, TPM_LEFTALIGN, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TPM_TOPALIGN, TrackPopupMenuEx, WM_APP, WM_NULL,
};
use windows::core::PCWSTR;

use crate::input::input_hwnd;
use crate::platform::win_get_scale;

/// Asks the input thread to track the parked request.
pub(crate) const WM_JFN_MENU_TRACK: u32 = WM_APP + 0x100;

/// Asks the input thread to end the menu it is tracking.
pub(crate) const WM_JFN_MENU_END: u32 = WM_APP + 0x101;

static PENDING: Mutex<Option<MenuRequest>> = Mutex::new(None);

static TRACKING: AtomicBool = AtomicBool::new(false);

static CANCELLED: AtomicBool = AtomicBool::new(false);

pub(crate) struct WinMenuHost;

pub(crate) fn owner_destroyed() {
    CANCELLED.store(true, Ordering::Release);
    let undelivered = PENDING.lock().take();
    drop(undelivered);
}

impl MenuHost for WinMenuHost {
    fn open(&self, req: MenuRequest) {
        let Some(hwnd) = input_hwnd() else {
            return;
        };
        let mut slot = PENDING.lock();
        let displaced = slot.replace(req);
        let undelivered = post_track(hwnd, &mut slot);
        drop(slot);
        drop((displaced, undelivered));
    }

    fn hide(&self) {
        CANCELLED.store(true, Ordering::Release);
        let pending = PENDING.lock().take();
        drop(pending);
        post_end();
    }

    fn shutdown(&self) {
        owner_destroyed();
        post_end();
    }
}

fn post_end() {
    if let Some(hwnd) = input_hwnd()
        && let Err(e) = unsafe { PostMessageW(Some(hwnd), WM_JFN_MENU_END, WPARAM(0), LPARAM(0)) }
    {
        tracing::error!(target: "platform", "menu dismissal could not be delivered: {e}");
    }
}

fn post_track(hwnd: HWND, slot: &mut Option<MenuRequest>) -> Option<MenuRequest> {
    if let Err(e) = unsafe { PostMessageW(Some(hwnd), WM_JFN_MENU_TRACK, WPARAM(0), LPARAM(0)) } {
        tracing::error!(target: "platform", "menu request could not be delivered: {e}");
        return slot.take();
    }
    None
}

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Runs on the input thread, from `input_wndproc`.
pub(crate) fn on_input_message(hwnd: HWND, msg: u32) {
    if msg == WM_JFN_MENU_END {
        if TRACKING.load(Ordering::Acquire)
            && let Err(e) = unsafe { EndMenu() }
        {
            tracing::error!(target: "platform", "ending menu: {e}");
        }
        return;
    }
    if msg != WM_JFN_MENU_TRACK {
        return;
    }
    if TRACKING.load(Ordering::Acquire) {
        if let Err(e) = unsafe { EndMenu() } {
            tracing::error!(target: "platform", "replacing menu: {e}");
            let undelivered = PENDING.lock().take();
            drop(undelivered);
            return;
        }
        let undelivered = post_track(hwnd, &mut PENDING.lock());
        drop(undelivered);
        return;
    }
    let Some(pending) = PENDING.lock().take() else {
        return;
    };
    let menu = match unsafe { CreatePopupMenu() } {
        Ok(menu) => menu,
        Err(e) => {
            tracing::error!(target: "platform", "creating menu: {e}");
            return;
        }
    };
    let picked = track(hwnd, menu, &pending);
    if let Err(e) = unsafe { DestroyMenu(menu) } {
        tracing::error!(target: "platform", "destroying menu: {e}");
    }
    match picked {
        Ok(Some(row)) => {
            if let Err(e) = pending.select(row) {
                tracing::error!(target: "platform", "menu selection failed: {e}");
            }
        }
        Ok(None) => {}
        Err(e) => tracing::error!(target: "platform", "tracking menu: {e}"),
    }
}

fn track(hwnd: HWND, menu: HMENU, pending: &MenuRequest) -> windows::core::Result<Option<usize>> {
    let mut command: usize = 0;
    for (g, group) in pending.groups.iter().enumerate() {
        if g > 0 {
            unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) }?;
        }
        for item in group {
            command += 1;
            let flags = if item.enabled {
                MF_STRING
            } else {
                MENU_ITEM_FLAGS(MF_STRING.0 | MF_GRAYED.0)
            };
            let label = wide(&item.label);
            unsafe { AppendMenuW(menu, flags, command, PCWSTR::from_raw(label.as_ptr())) }?;
        }
    }

    let scale = win_get_scale();
    let anchor = pending.anchor;
    let (Some(x), Some(y)) = (scale.to_physical(anchor.x), scale.to_physical(anchor.y)) else {
        tracing::error!(target: "platform", "menu anchor {},{} is unrepresentable at scale {scale}", anchor.x, anchor.y);
        return Ok(None);
    };
    let mut pt = POINT { x, y };
    unsafe { ClientToScreen(hwnd, &mut pt) }.ok()?;

    if let Some(toplevel) = crate::platform::win_hwnd() {
        let _ = unsafe { SetForegroundWindow(toplevel) };
    }

    CANCELLED.store(false, Ordering::Release);
    TRACKING.store(true, Ordering::Release);
    let flags =
        TPM_RETURNCMD.0 | TPM_NONOTIFY.0 | TPM_LEFTALIGN.0 | TPM_TOPALIGN.0 | TPM_RIGHTBUTTON.0;
    let picked = unsafe { TrackPopupMenuEx(menu, flags, pt.x, pt.y, hwnd, None) };
    TRACKING.store(false, Ordering::Release);
    let _ = unsafe { PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0)) };

    if CANCELLED.swap(false, Ordering::AcqRel) {
        return Ok(None);
    }
    Ok(usize::try_from(picked.0)
        .ok()
        .and_then(|picked| picked.checked_sub(1)))
}
