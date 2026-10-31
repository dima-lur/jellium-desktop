use std::num::NonZeroU32;
use std::sync::Arc;

use cef::{ImplBrowserHost, KeyEvent, sys};
use jfn_platform_abi::{ChannelValue, Item, LogicalPoint};

use crate::platform_ops::MenuDelivery;
use crate::receiver::CefMsg;

use super::{Inner, PopupState};

// Windows virtual-key codes CEF expects in KeyEvent::windows_key_code.
const VK_RETURN: i32 = 0x0D;
const VK_ESCAPE: i32 = 0x1B;
const VK_UP: i32 = 0x26;
const VK_DOWN: i32 = 0x28;

impl Inner {
    pub(super) fn dropdown(&self) -> MenuDelivery<'_> {
        self.surface()
            .platform()
            .menu_delivery(jfn_platform_abi::MenuKind::Dropdown)
    }
    fn reset_popup_state(p: &mut PopupState) {
        p.size_received = false;
        p.options_received = false;
        p.options.clear();
        p.selected_idx = None;
        p.selectable.clear();
        p.anchor = None;
    }

    pub(crate) fn on_popup_show(&self, show: bool) {
        {
            let mut p = self.popup.lock();
            p.visible = show;
            Self::reset_popup_state(&mut p);
        }
        if !show {
            self.hide_dropdown();
            return;
        }
        self.send_process_message_named("getPopupOptions");
    }

    pub(crate) fn on_popup_size(self: &Arc<Self>, x: i32, y: i32, w: i32, h: i32) {
        {
            let mut p = self.popup.lock();
            p.x = x;
            p.y = y;
            p.w = w;
            p.h = h;
            p.size_received = true;
        }
        self.try_show_popup();
    }

    pub(crate) fn set_popup_options(
        self: &Arc<Self>,
        opts: Vec<String>,
        selected: Option<usize>,
        selectable: Vec<i32>,
        anchor: Option<(i32, i32)>,
    ) {
        {
            let mut p = self.popup.lock();
            p.options = opts;
            p.selected_idx = selected;
            p.selectable = selectable;
            p.anchor = anchor;
            p.options_received = true;
        }
        self.try_show_popup();
    }

    fn try_show_popup(self: &Arc<Self>) {
        let (x, y, w, h, opts, selected, selectable) = {
            let p = self.popup.lock();
            if !p.visible || !p.size_received || !p.options_received {
                return;
            }
            // Blink's popup rect (p.x/p.y) flips above the element near the
            // window bottom; the anchor keeps the menu under the box.
            let (x, y) = if matches!(self.dropdown(), MenuDelivery::Host(_)) {
                p.anchor.unwrap_or((p.x, p.y))
            } else {
                (p.x, p.y)
            };
            (
                x,
                y,
                p.w,
                p.h,
                p.options.clone(),
                p.selected_idx,
                p.selectable.clone(),
            )
        };

        match self.dropdown() {
            MenuDelivery::Host(_) => {
                let builder = self.menu.lock().app.new_menu(LogicalPoint { x, y });
                let serial = builder.serial();
                let send_keys = |inner: &Inner, codes: &[i32]| {
                    if !inner.popup.lock().visible {
                        return;
                    }
                    let Some(host) = inner.host() else {
                        return;
                    };
                    for &code in codes {
                        let down = KeyEvent {
                            type_: sys::cef_key_event_type_t::KEYEVENT_RAWKEYDOWN.into(),
                            windows_key_code: code,
                            native_key_code: code,
                            ..KeyEvent::default()
                        };
                        let up = KeyEvent {
                            type_: sys::cef_key_event_type_t::KEYEVENT_KEYUP.into(),
                            windows_key_code: code,
                            native_key_code: code,
                            ..KeyEvent::default()
                        };
                        host.send_key_event(Some(&down));
                        host.send_key_event(Some(&up));
                    }
                };
                let message = |codes: Vec<i32>| {
                    let weak = Arc::downgrade(self);
                    CefMsg::Menu {
                        serial,
                        callback: Box::new(move || {
                            if let Some(inner) = weak.upgrade() {
                                send_keys(&inner, &codes);
                            }
                        }),
                    }
                };
                let pos = |opt: usize| {
                    i32::try_from(opt)
                        .ok()
                        .and_then(|opt| selectable.iter().position(|&v| v == opt))
                };
                let from = selected.and_then(pos).unwrap_or(0);
                let items = opts
                    .iter()
                    .enumerate()
                    .map(|(i, label)| {
                        let codes = match pos(i) {
                            Some(to) => {
                                let step = if to >= from { VK_DOWN } else { VK_UP };
                                let mut codes = vec![step; to.abs_diff(from)];
                                codes.push(VK_RETURN);
                                codes
                            }
                            None => vec![VK_ESCAPE],
                        };
                        Item {
                            label: label.clone(),
                            enabled: pos(i).is_some(),
                            value: ChannelValue::new(self.channel.sender(), message(codes)),
                        }
                    })
                    .collect();
                let builder = builder
                    .add_items(items)
                    .add_on_closed(ChannelValue::new(
                        self.channel.sender(),
                        message(vec![VK_ESCAPE]),
                    ))
                    .current(selected)
                    .width(u32::try_from(w).ok().and_then(NonZeroU32::new));
                self.menu.lock().open_dropdown(builder);
            }
            MenuDelivery::Composited => self.surface().popup_show(x, y, w, h),
            MenuDelivery::Page => {}
        }
    }

    fn hide_dropdown(&self) {
        match self.dropdown() {
            MenuDelivery::Host(_) => {
                let menu = self.menu.lock();
                if let Some(serial) = menu.showing {
                    menu.app.close_menu(serial);
                }
            }
            MenuDelivery::Composited => self.surface().popup_hide(),
            MenuDelivery::Page => {}
        }
    }

    pub(crate) fn on_deactivated(&self) {
        let was_visible = {
            let mut p = self.popup.lock();
            let was = p.visible;
            if was {
                p.visible = false;
                Self::reset_popup_state(&mut p);
            }
            was
        };
        if !was_visible {
            return;
        }
        self.hide_dropdown();
    }

    pub(super) fn popup_rect(&self) -> (i32, i32) {
        let p = self.popup.lock();
        (p.w, p.h)
    }
}
