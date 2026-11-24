use jfn_platform_abi::geometry::{Bounds, RestoreScreen};
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::core::{BOOL, Result};

pub(crate) fn info(handle: HMONITOR) -> Result<(String, RECT)> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    unsafe { GetMonitorInfoW(handle, std::ptr::from_mut(&mut info).cast::<MONITORINFO>()) }.ok()?;
    let end = info
        .szDevice
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(info.szDevice.len());
    Ok((
        String::from_utf16_lossy(&info.szDevice[..end]),
        info.monitorInfo.rcWork,
    ))
}

unsafe extern "system" fn collect(
    monitor: HMONITOR,
    _dc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) }.push(monitor);
    true.into()
}

pub(crate) fn find(name: &str) -> Result<Option<RestoreScreen>> {
    // mpv's w32 backend assigns --screen indices in EnumDisplayMonitors order.
    let mut handles: Vec<HMONITOR> = Vec::new();
    let data = LPARAM(std::ptr::from_mut(&mut handles) as isize);
    unsafe { EnumDisplayMonitors(None, None, Some(collect), data) }.ok()?;
    for (index, handle) in handles.into_iter().enumerate() {
        let (device, rect) = info(handle)?;
        if device == name {
            let (mut x, mut y) = (0, 0);
            let scale = unsafe { GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut x, &mut y) }
                .ok()
                .and_then(|()| crate::scale::scale_from_dpi(x));
            return Ok(Some(RestoreScreen {
                index,
                scale,
                bounds: Bounds {
                    w: rect.right - rect.left,
                    h: rect.bottom - rect.top,
                },
            }));
        }
    }
    Ok(None)
}
