use crate::ipc::BrowserMessage;

use super::Inner;

impl Inner {
    pub(crate) fn invoke_message_handler(&self, message: BrowserMessage) -> bool {
        let g = self.message_handler.lock();
        g.as_ref().map(|f| f(message)).unwrap_or(false)
    }
}
