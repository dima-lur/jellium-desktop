use std::ffi::c_int;
use std::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use crate::LogicalPoint;
use crate::context_menu::{ChannelValue, Item, MenuBuilder};

/// Identifies one popup across its whole life; a surface drops anything
/// naming a generation it no longer owns.
pub type Generation = NonZeroU64;

/// Selection value meaning "nothing was chosen".
pub const MENU_DISMISSED: c_int = -1;

pub struct MenuRequest {
    pub groups: Arc<Vec<Vec<Item>>>,
    pub anchor: LogicalPoint,
    pub width: Option<NonZeroU32>,
    current: Option<usize>,
    on_closed: Vec<ChannelValue>,
}

impl MenuRequest {
    pub fn current(&self) -> Option<usize> {
        self.current
    }

    pub fn select(self, row: usize) -> Result<(), Box<dyn std::error::Error>> {
        let Some(item) = self.groups.iter().flatten().nth(row) else {
            return Err(format!("menu row {row} out of range").into());
        };
        if !item.enabled {
            return Err(format!("menu row {row} is disabled").into());
        }
        item.value.send();
        Ok(())
    }
}

impl From<MenuBuilder> for MenuRequest {
    fn from(builder: MenuBuilder) -> MenuRequest {
        MenuRequest {
            groups: Arc::new(builder.items),
            anchor: builder.anchor,
            width: builder.width,
            current: builder.current,
            on_closed: builder.on_closed,
        }
    }
}

impl Drop for MenuRequest {
    fn drop(&mut self) {
        self.on_closed.iter().for_each(ChannelValue::send);
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MenuKind {
    ContextMenu,
    Dropdown,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MenuScript {
    SelectMenu,
}

pub fn menu_scripts(kind: MenuKind) -> &'static [MenuScript] {
    let Some(lease) = crate::try_lease() else {
        return &[];
    };
    match (lease.menu_delivery(kind), kind) {
        (MenuDelivery::Page, MenuKind::Dropdown) => &[MenuScript::SelectMenu],
        _ => &[],
    }
}

/// Native menu access is borrowed from the platform lease.
///
/// ```compile_fail
/// fn escape(lease: &jfn_platform_abi::PlatformLease) -> jfn_platform_abi::MenuDelivery<'static> {
///     lease.menu_delivery(jfn_platform_abi::MenuKind::ContextMenu)
/// }
/// ```
#[derive(Copy, Clone)]
pub enum MenuDelivery<'a> {
    Host(&'a dyn MenuHost),
    Composited,
    Page,
}

pub trait MenuHost: Send + Sync {
    fn warm(&self) {}

    /// Replaces any menu already open, and returns before the menu is drawn.
    fn open(&self, req: MenuRequest);

    fn hide(&self) {}

    fn shutdown(&self) {}
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct MenuPlacement {
    /// Anchor in logical (view) coordinates.
    pub anchor: crate::geometry::LogicalPoint,
    /// The visible menu's own size in both spaces.
    pub view: crate::geometry::WindowExtent,
}

pub struct MenuPaint {
    pub generation: Generation,
    /// Premultiplied BGRA, `buffer.w` x `buffer.h`.
    pub pixels: Vec<u8>,
    /// Physical (buffer) size of the whole menu.
    pub buffer: crate::geometry::PhysicalSize,
    /// Scroll offset into the buffer, physical px.
    pub scroll: c_int,
    /// The visible crop's own size in both spaces.
    pub view: crate::geometry::WindowExtent,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct MenuMetrics {
    /// Physical pixels per logical pixel.
    pub scale: crate::geometry::Scale,
    /// Window height, physical px, that a width-constrained menu is clamped to;
    /// `None` leaves every menu full height.
    pub clamp_ph: Option<c_int>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MenuClose {
    Finished,
    Speculative,
    External,
}

/// The platform surface a software-rendered menu drives. Every method is called
/// from the menu's own thread and must not block it.
pub trait PopupSurface: Send + Sync {
    fn metrics(&self) -> MenuMetrics;

    /// Puts up the surface that holds the grab, anchored at `anchor`, with
    /// no menu on it.
    ///
    /// That surface has no menu, so it has no logical size, no physical size
    /// and no scale: each backend maps it at the smallest size its own
    /// protocol admits, and no caller supplies one.
    ///
    /// `serial` is the input serial a grab must cite; backends that do not
    /// grab on a serial ignore it.
    fn arm(&self, generation: Generation, anchor: crate::geometry::LogicalPoint, serial: u32);

    /// Maps the armed surface, so the grab takes effect before the menu has
    /// pixels. A backend whose armed surface maps at [`PopupSurface::arm`]
    /// states that here and does nothing else.
    fn map_armed(&self, generation: Generation);

    fn reposition(&self, generation: Generation, place: MenuPlacement);

    fn present(&self, paint: MenuPaint);

    fn destroy(&self, generation: Generation, reason: MenuClose);
}
