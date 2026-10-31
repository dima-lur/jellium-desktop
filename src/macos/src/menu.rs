//! CEF's Alloy OSR popup renders `<select>` hover/selection highlights as
//! opaque black on macOS, so its popup runs invisibly and a native NSMenu is
//! presented in its place.

use std::ffi::c_int;

use jfn_platform_abi::{MenuHost, MenuRequest};

use crate::ns_menu::{MenuEntry, MenuSpec, present_on_main};

pub(crate) struct NsMenuHost;

impl MenuHost for NsMenuHost {
    fn open(&self, req: MenuRequest) {
        let current = req.current().and_then(|row| c_int::try_from(row).ok());
        let mut row: c_int = 0;
        let groups = req
            .groups
            .iter()
            .map(|group| {
                group
                    .iter()
                    .map(|it| {
                        let tag = row;
                        row += 1;
                        MenuEntry {
                            checked: Some(tag) == current,
                            title: it.label.clone(),
                            tag,
                            enabled: it.enabled,
                        }
                    })
                    .collect()
            })
            .collect();
        let spec = MenuSpec {
            groups,
            x: req.anchor.x,
            y: req.anchor.y,
            positioning_tag: current,
            min_width: req.width.and_then(|w| c_int::try_from(w.get()).ok()),
        };
        present_on_main(spec, req);
    }
}
