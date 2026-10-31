use std::collections::BTreeMap;
use std::num::NonZeroU32;

use crate::lang::{ContextMenuLabel, LanguageLoader};
use parking_lot::Mutex;

use crate::LogicalPoint;
use crate::app::MenuSerial;

/// Dispatches menu actions to either a standard channel or an actor queue.
pub trait MessageSender<T>: Send + Sync + 'static {
    fn send_message(&self, message: T);
}

impl<T: Send + 'static> MessageSender<T> for std::sync::mpsc::Sender<T> {
    fn send_message(&self, message: T) {
        drop(self.send(message));
    }
}

pub struct ChannelValue {
    send: Box<dyn Fn() + Send + Sync>,
}

impl ChannelValue {
    pub fn new<T: Send + 'static>(sender: impl MessageSender<T>, value: T) -> ChannelValue {
        let value = Mutex::new(Some(value));
        ChannelValue {
            send: Box::new(move || {
                if let Some(value) = value.lock().take() {
                    sender.send_message(value);
                }
            }),
        }
    }

    pub(crate) fn send(&self) {
        (self.send)();
    }
}

pub struct Item {
    pub label: String,
    pub enabled: bool,
    pub value: ChannelValue,
}

pub struct MenuBuilder {
    pub(crate) serial: MenuSerial,
    pub(crate) anchor: LogicalPoint,
    pub(crate) items: Vec<Vec<Item>>,
    pub(crate) on_closed: Vec<ChannelValue>,
    pub(crate) current: Option<usize>,
    pub(crate) width: Option<NonZeroU32>,
}

impl MenuBuilder {
    pub fn serial(&self) -> MenuSerial {
        self.serial
    }

    #[must_use]
    pub fn current(mut self, row: Option<usize>) -> MenuBuilder {
        self.current = row;
        self
    }

    #[must_use]
    pub fn width(mut self, width: Option<NonZeroU32>) -> MenuBuilder {
        self.width = width;
        self
    }

    #[must_use]
    pub fn add_items(mut self, items: Vec<Item>) -> MenuBuilder {
        self.items.push(items);
        self
    }

    #[must_use]
    pub fn add_on_closed(mut self, value: ChannelValue) -> MenuBuilder {
        self.on_closed.push(value);
        self
    }
}

pub struct ContextMenuBuilder {
    menu: MenuBuilder,
    locale: LanguageLoader,
    items: BTreeMap<ContextMenuLabel, Item>,
}

impl ContextMenuBuilder {
    pub(crate) fn new(menu: MenuBuilder, locale: LanguageLoader) -> ContextMenuBuilder {
        ContextMenuBuilder {
            menu,
            locale,
            items: BTreeMap::new(),
        }
    }

    pub fn serial(&self) -> MenuSerial {
        self.menu.serial()
    }

    #[must_use]
    pub fn add(
        mut self,
        label: ContextMenuLabel,
        enabled: bool,
        value: ChannelValue,
    ) -> ContextMenuBuilder {
        let item = Item {
            label: self.locale.menu_label(label),
            enabled,
            value,
        };
        self.items.insert(label, item);
        self
    }

    #[must_use]
    pub fn add_on_closed(mut self, value: ChannelValue) -> ContextMenuBuilder {
        self.menu = self.menu.add_on_closed(value);
        self
    }
}

impl From<ContextMenuBuilder> for MenuBuilder {
    fn from(builder: ContextMenuBuilder) -> MenuBuilder {
        let mut navigation = Vec::new();
        let mut undo = Vec::new();
        let mut clipboard = Vec::new();
        let mut app = Vec::new();
        for (label, item) in builder.items {
            match label {
                ContextMenuLabel::Back | ContextMenuLabel::Forward | ContextMenuLabel::Reload => {
                    navigation.push(item)
                }
                ContextMenuLabel::Undo | ContextMenuLabel::Redo => undo.push(item),
                ContextMenuLabel::Cut
                | ContextMenuLabel::Copy
                | ContextMenuLabel::Paste
                | ContextMenuLabel::SelectAll => clipboard.push(item),
                ContextMenuLabel::Fullscreen
                | ContextMenuLabel::Settings
                | ContextMenuLabel::Exit => app.push(item),
            }
        }
        [navigation, undo, clipboard, app]
            .into_iter()
            .filter(|group| !group.is_empty())
            .fold(builder.menu, MenuBuilder::add_items)
    }
}
