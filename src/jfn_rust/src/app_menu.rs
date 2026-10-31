use std::sync::atomic::{AtomicUsize, Ordering};

use jfn_platform_abi::{MenuBuilder, MenuDelivery, MenuKind, MenuRequest, MenuSerial};
use parking_lot::Mutex;

pub(crate) struct MenuManager {
    next_serial: AtomicUsize,
    showing: Mutex<Option<MenuSerial>>,
}

impl MenuManager {
    pub(crate) fn new() -> MenuManager {
        MenuManager {
            next_serial: AtomicUsize::new(0),
            showing: Mutex::new(None),
        }
    }

    pub(crate) fn next_serial(&self) -> MenuSerial {
        MenuSerial::new(self.next_serial.fetch_add(1, Ordering::Relaxed))
    }

    pub(crate) fn open(&self, serial: MenuSerial, builder: MenuBuilder) {
        let request = MenuRequest::from(builder);
        let Some(platform) = jfn_platform_abi::try_lease() else {
            return;
        };
        let MenuDelivery::Host(host) = platform.menu_delivery(MenuKind::ContextMenu) else {
            return;
        };
        *self.showing.lock() = Some(serial);
        host.open(request);
    }

    pub(crate) fn close(&self, serial: MenuSerial) {
        let mut showing = self.showing.lock();
        if *showing != Some(serial) {
            return;
        }
        *showing = None;
        drop(showing);
        let Some(platform) = jfn_platform_abi::try_lease() else {
            return;
        };
        if let MenuDelivery::Host(host) = platform.menu_delivery(MenuKind::ContextMenu) {
            host.hide();
        }
    }
}
