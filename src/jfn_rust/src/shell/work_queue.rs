//! Coalesce producer state before it enters the render queue. Input barriers
//! (clicks/keys) retain ordering; each render pass takes a finite snapshot.
use super::actor::Work;
use iced_core::{Event, mouse};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(super) struct WorkSender {
    queue: Arc<Mutex<VecDeque<Work>>>,
    wake: mpsc::SyncSender<()>,
}
pub(super) struct WorkReceiver {
    queue: Arc<Mutex<VecDeque<Work>>>,
    wake: mpsc::Receiver<()>,
}
pub(super) fn work_channel() -> (WorkSender, WorkReceiver) {
    // One wake token describes a nonempty queue, not a capacity policy.
    let (tx, rx) = mpsc::sync_channel(1);
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    (
        WorkSender {
            queue: queue.clone(),
            wake: tx,
        },
        WorkReceiver { queue, wake: rx },
    )
}
fn replaces(previous: &Work, next: &Work) -> bool {
    matches!(
        (previous, next),
        (
            Work::Event(Event::Mouse(mouse::Event::CursorMoved { .. })),
            Work::Event(Event::Mouse(mouse::Event::CursorMoved { .. }))
        ) | (Work::Resize { .. }, Work::Resize { .. })
            | (Work::Chrome(_), Work::Chrome(_))
            | (Work::ChromeBackground(_), Work::ChromeBackground(_))
    )
}
impl WorkSender {
    pub(super) fn send(&self, work: Work) -> Result<(), QueueClosed> {
        let mut queue = self.queue.lock();
        if matches!(work, Work::Shutdown) {
            queue.clear();
        } else if matches!(work, Work::Redraw) && !queue.is_empty() {
            return Ok(());
        } else if queue
            .back()
            .is_some_and(|previous| replaces(previous, &work))
        {
            queue.pop_back();
        }
        queue.push_back(work);
        match self.wake.try_send(()) {
            Ok(()) | Err(mpsc::TrySendError::Full(())) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(())) => {
                queue.clear();
                Err(QueueClosed)
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct QueueClosed;

impl WorkReceiver {
    pub(super) fn take(&self) -> VecDeque<Work> {
        std::mem::take(&mut *self.queue.lock())
    }
    pub(super) fn recv(&self) -> Result<Work, mpsc::RecvError> {
        loop {
            if let Some(work) = self.queue.lock().pop_front() {
                return Ok(work);
            }
            self.wake.recv()?;
        }
    }
    pub(super) fn recv_timeout(&self, timeout: Duration) -> Result<Work, mpsc::RecvTimeoutError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(work) = self.queue.lock().pop_front() {
                return Ok(work);
            }
            self.wake
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer(x: f32) -> Work {
        Work::Event(Event::Mouse(mouse::Event::CursorMoved {
            position: iced_core::Point::new(x, 0.0),
        }))
    }

    #[test]
    fn pointer_burst_keeps_latest_position_without_crossing_clicks() {
        let (tx, rx) = work_channel();
        for x in 0..1000 {
            assert!(tx.send(pointer(x as f32)).is_ok());
        }
        assert!(
            tx.send(Work::Event(Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left
            ))))
            .is_ok()
        );
        assert!(tx.send(pointer(1000.0)).is_ok());
        let batch = rx.take();
        assert_eq!(batch.len(), 3);
        assert!(
            matches!(batch.front(), Some(Work::Event(Event::Mouse(mouse::Event::CursorMoved { position }))) if position.x == 999.0)
        );
    }

    #[test]
    fn redraws_coalesce_and_shutdown_supersedes_pending_work() {
        let (tx, rx) = work_channel();
        for _ in 0..1000 {
            assert!(tx.send(Work::Redraw).is_ok());
        }
        assert_eq!(rx.take().len(), 1);
        assert!(tx.send(pointer(1.0)).is_ok());
        assert!(tx.send(Work::Shutdown).is_ok());
        let batch = rx.take();
        assert_eq!(batch.len(), 1);
        assert!(matches!(batch.front(), Some(Work::Shutdown)));
    }
}
