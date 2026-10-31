//! Background thread that drains `mpv_wait_event` and delivers owned
//! [`Event`]s to a consumer.
//!
//! libmpv's event queue must be drained from somewhere; the standard pattern
//! is a dedicated thread blocked in `mpv_wait_event`. This module wraps that
//! loop so callers see a typed [`Receiver<Event>`] instead of raw FFI.
//!
//! The loop exits when:
//! - libmpv delivers `MPV_EVENT_SHUTDOWN` (forwarded to the consumer first), or
//! - [`EventLoop::stop`] is called, or
//! - the consumer drops the receiver (send error breaks the loop).

use crate::event::Event;
use crate::handle::Handle;
use crossbeam_channel::{Receiver, unbounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

pub struct EventLoop {
    handle: Arc<Handle>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl EventLoop {
    /// Spawn the drain thread. Returns the loop owner and a [`Receiver`]
    /// for typed events. The wakeup callback on `handle` is left untouched —
    /// `mpv_wait_event(-1)` blocks until libmpv has something to deliver.
    pub fn spawn(handle: Arc<Handle>) -> std::io::Result<(Self, Receiver<Event>)> {
        let (tx, rx) = unbounded();
        let owner = Self::spawn_with(handle, move |event| tx.send(event).is_ok())?;
        Ok((owner, rx))
    }

    /// Consume events inline on the drain thread. The consumer must not call
    /// synchronous mpv APIs; use property observations or async commands.
    pub fn spawn_with(
        handle: Arc<Handle>,
        consume: impl FnMut(Event) -> bool + Send + 'static,
    ) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let handle = Arc::clone(&handle);
            let stop = Arc::clone(&stop);
            thread::Builder::new()
                .name("jfn-mpv-events".into())
                .spawn(move || drain(handle, stop, consume))?
        };
        Ok(Self {
            handle,
            stop,
            thread: Some(thread),
        })
    }

    /// Signal the loop to exit and wake `mpv_wait_event` so the next
    /// iteration observes the flag. Joins the thread.
    pub fn stop(&mut self) {
        if self.thread.is_none() {
            return;
        }
        self.stop.store(true, Ordering::Release);
        self.handle.wakeup();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for EventLoop {
    fn drop(&mut self) {
        self.stop();
    }
}

fn drain(handle: Arc<Handle>, stop: Arc<AtomicBool>, mut consume: impl FnMut(Event) -> bool) {
    loop {
        if stop.load(Ordering::Acquire) {
            return;
        }
        let event = handle.wait_event(-1.0);
        match event {
            // Timeout sentinel; spurious wakeup (e.g. from `Handle::wakeup`).
            Event::None => continue,
            // Log messages go straight to tracing; consumers never see them.
            Event::LogMessage(ref m) => crate::log::forward_to_tracing(m),
            Event::Shutdown => {
                // Forward shutdown so consumers can react, then exit.
                consume(Event::Shutdown);
                return;
            }
            other => {
                if !consume(other) {
                    return;
                }
            }
        }
    }
}
