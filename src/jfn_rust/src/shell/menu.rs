//! The edit menu, raised through the platform's own menu host.

use jfn_platform_abi::lang::ContextMenuLabel;
use jfn_platform_abi::{App, ChannelValue, LogicalPoint};

use crate::shell::actor::{Target, Work};
use crate::shell::fields::Snapshot;

/// The edit menu's items, in order.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Item {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
}

impl Item {
    pub const ALL: [Item; 6] = [
        Item::Undo,
        Item::Redo,
        Item::Cut,
        Item::Copy,
        Item::Paste,
        Item::SelectAll,
    ];

    pub fn command(self) -> jfn_input::EditCommand {
        use jfn_input::EditCommand as E;
        match self {
            Item::Undo => E::Undo,
            Item::Redo => E::Redo,
            Item::Cut => E::Cut,
            Item::Copy => E::Copy,
            Item::Paste => E::Paste,
            Item::SelectAll => E::SelectAll,
        }
    }

    pub fn label(self) -> ContextMenuLabel {
        match self {
            Item::Undo => ContextMenuLabel::Undo,
            Item::Redo => ContextMenuLabel::Redo,
            Item::Cut => ContextMenuLabel::Cut,
            Item::Copy => ContextMenuLabel::Copy,
            Item::Paste => ContextMenuLabel::Paste,
            Item::SelectAll => ContextMenuLabel::SelectAll,
        }
    }

    /// Enabled exactly when it would change something; Paste always.
    pub fn enabled(self, field: &Snapshot) -> bool {
        let edit = field.edit_state();
        match self {
            Item::Undo => edit.undo,
            Item::Redo => edit.redo,
            Item::Cut => edit.cut,
            Item::Copy => edit.copy,
            Item::Paste => true,
            Item::SelectAll => edit.select_all,
        }
    }
}

/// Raises the edit menu for `field` at `anchor`, in window coordinates. A
/// selection posts [`crate::shell::actor::Work::EditAt`] naming `field`; a dismissal
/// posts nothing. No accelerator text is drawn.
pub fn open_edit(app: &App, field: &Snapshot, anchor: LogicalPoint) {
    let sender = crate::shell::shell_sender();
    let builder = Item::ALL
        .into_iter()
        .fold(app.new_context_menu(anchor), |builder, item| {
            builder.add(
                item.label(),
                item.enabled(field),
                ChannelValue::new(
                    sender.clone(),
                    Work::EditAt {
                        field: Target::Named(field.id.clone()),
                        command: item.command(),
                    },
                ),
            )
        });
    app.open_context_menu(builder);
}
