use crate::LogicalPoint;
use crate::context_menu::{ContextMenuBuilder, MenuBuilder, MessageSender};
use crate::lang::LanguageLoader;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct MenuSerial(usize);

impl MenuSerial {
    pub fn new(value: usize) -> MenuSerial {
        MenuSerial(value)
    }
}

pub enum AppMsg {
    OpenContextMenu(MenuSerial, ContextMenuBuilder),
    OpenDropdown(MenuSerial, MenuBuilder),
    CloseMenu(MenuSerial),
    OpenClientSettings,
    OpenAbout,
}

pub struct App {
    channel: Box<dyn Fn(AppMsg) + Send + Sync>,
    next_serial: Box<dyn Fn() -> MenuSerial + Send + Sync>,
    locale: LanguageLoader,
}

impl App {
    pub fn new<T: From<AppMsg> + Send + 'static>(
        channel: impl MessageSender<T>,
        next_serial: impl Fn() -> MenuSerial + Send + Sync + 'static,
        locale: LanguageLoader,
    ) -> App {
        App {
            channel: Box::new(move |msg| channel.send_message(T::from(msg))),
            next_serial: Box::new(next_serial),
            locale,
        }
    }

    pub fn locale(&self) -> &LanguageLoader {
        &self.locale
    }

    pub fn new_menu(&self, anchor: LogicalPoint) -> MenuBuilder {
        MenuBuilder {
            serial: (self.next_serial)(),
            anchor,
            items: Vec::new(),
            on_closed: Vec::new(),
            current: None,
            width: None,
        }
    }

    pub fn new_context_menu(&self, anchor: LogicalPoint) -> ContextMenuBuilder {
        ContextMenuBuilder::new(self.new_menu(anchor), self.locale.clone())
    }

    pub fn open_context_menu(&self, builder: ContextMenuBuilder) {
        (self.channel)(AppMsg::OpenContextMenu(builder.serial(), builder));
    }

    pub fn open_dropdown(&self, builder: MenuBuilder) {
        (self.channel)(AppMsg::OpenDropdown(builder.serial, builder));
    }

    pub fn close_menu(&self, serial: MenuSerial) {
        (self.channel)(AppMsg::CloseMenu(serial));
    }

    pub fn open_client_settings(&self) {
        (self.channel)(AppMsg::OpenClientSettings);
    }

    pub fn open_about(&self) {
        (self.channel)(AppMsg::OpenAbout);
    }
}
