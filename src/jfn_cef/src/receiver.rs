use cef::rc::Rc;
use cef::{ImplTask, Task, ThreadId, WrapTask, post_task, wrap_task};
use jfn_platform_abi::MenuSerial;
use parking_lot::Mutex;
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;

use crate::client::Inner;

pub(crate) enum CefMsg {
    Menu {
        serial: MenuSerial,
        callback: Box<dyn FnOnce() + Send>,
    },
}

pub(crate) struct Channel {
    sender: std::sync::mpsc::Sender<CefMsg>,
    _thread: JoinHandle<()>,
}

impl Channel {
    pub(crate) fn new(inner: Weak<Inner>) -> Channel {
        let (sender, receiver) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            while let Ok(message) = receiver.recv() {
                let Some(inner) = inner.upgrade() else {
                    continue;
                };
                let session = Arc::clone(&inner.session);
                session.dispatch(|| {
                    let mut task = CefMsgTask::new(inner, Arc::new(Mutex::new(Some(message))));
                    let _ = post_task(ThreadId::UI, Some(&mut task));
                });
            }
        });
        Channel {
            sender,
            _thread: thread,
        }
    }

    pub(crate) fn sender(&self) -> std::sync::mpsc::Sender<CefMsg> {
        self.sender.clone()
    }
}

wrap_task! {
    struct CefMsgTask {
        inner: Arc<Inner>,
        message: Arc<Mutex<Option<CefMsg>>>,
    }
    impl Task {
        fn execute(&self) {
            let Some(message) = self.message.lock().take() else {
                return;
            };
            self.inner.session.dispatch(|| {
                self.inner.menu.lock().finish(message);
            });
        }
    }
}
