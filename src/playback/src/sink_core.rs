//! Shared scaffolding for OS "now playing" media sinks (macOS
//! MPNowPlayingInfoCenter, Windows SMTC). Both platforms drove an
//! identical queue + consumer-thread harness and the same
//! kind→phase / command-dispatch logic; that lives here once. Each
//! platform supplies only a [`QueuedSink`] whose `deliver` drives its
//! native transport.
//!
//! The Linux MPRIS sink (jfn-mpris) has its own zbus-reactor thread and
//! does not use [`run_sink`], but it shares [`MediaCommand`] /
//! [`seek_to_ms`] so transport command semantics live in one place.

// =====================================================================
// Transport commands — shared by every sink (macOS / Windows / MPRIS).
// =====================================================================

/// A media-key / remote command the OS transport can raise.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum MediaCommand {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
}

/// Execute a transport command: play/pause/stop go straight to mpv;
/// next/previous route to the JS UI (the queue lives in jellyfin-web).
pub fn execute(cmd: MediaCommand) {
    match cmd {
        MediaCommand::Play => jfn_mpv::api::jfn_mpv_play(),
        MediaCommand::Pause => jfn_mpv::api::jfn_mpv_pause(),
        MediaCommand::PlayPause => jfn_mpv::api::jfn_mpv_toggle_pause(),
        MediaCommand::Stop => jfn_mpv::api::jfn_mpv_stop(),
        MediaCommand::Next => {
            crate::exec_js::call("if(window._nativeHostInput) window._nativeHostInput(['next']);")
        }
        MediaCommand::Previous => crate::exec_js::call(
            "if(window._nativeHostInput) window._nativeHostInput(['previous']);",
        ),
    }
}

/// Seek the UI to an absolute position in milliseconds. Routes to the JS
/// UI, which is the seek authority; mpv follows once the UI re-issues play.
pub fn seek_to_ms(ms: i64) {
    crate::exec_js::call(&format!("if(window._nativeSeek) window._nativeSeek({ms});"));
}

/// Rate-limits now-playing timeline pushes.
#[derive(Default)]
pub struct PositionThrottle {
    last: Option<std::time::Instant>,
    forced: bool,
}

impl PositionThrottle {
    /// Minimum wall-clock gap between two unforced pushes.
    pub const INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

    #[must_use]
    pub const fn new() -> Self {
        Self {
            last: None,
            forced: false,
        }
    }

    /// Make the next [`due`](Self::due) call report true whatever the elapsed
    /// time.
    pub const fn force_next(&mut self) {
        self.forced = true;
    }

    /// True when a push is due at `now`. A `force` argument or a pending
    /// [`force_next`](Self::force_next) bypasses the elapsed check, as does the
    /// first call after construction. A true result records the push.
    pub fn due(&mut self, now: std::time::Instant, force: bool) -> bool {
        let forced = force || self.forced;
        let elapsed_ok = self.last.is_none_or(|last| now - last >= Self::INTERVAL);
        if !forced && !elapsed_ok {
            return false;
        }
        self.forced = false;
        self.last = Some(now);
        true
    }
}

// =====================================================================
// Consumer-thread harness — used only by the macOS / Windows sinks; the
// Linux MPRIS sink runs its own zbus thread.
// =====================================================================

mod harness {
    use parking_lot::Condvar;
    use parking_lot::Mutex;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::Once;

    use crate::types::{PlaybackEvent, PlaybackEventKind};

    /// Coarse playback phase the OS transports care about. mpv's richer
    /// `PlaybackPhase` collapses to these three for now-playing display.
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub enum Phase {
        Playing,
        Paused,
        Stopped,
    }

    /// Map a playback event kind to the coarse transport phase.
    pub fn map_kind_to_phase(kind: PlaybackEventKind) -> Phase {
        match kind {
            PlaybackEventKind::Started => Phase::Playing,
            PlaybackEventKind::Paused | PlaybackEventKind::TrackLoaded => Phase::Paused,
            PlaybackEventKind::Finished
            | PlaybackEventKind::Canceled
            | PlaybackEventKind::Error => Phase::Stopped,
            _ => Phase::Stopped,
        }
    }

    /// A platform now-playing transport. The harness owns the event queue
    /// and the consumer thread; the impl only reacts to events.
    ///
    /// Not `Send`: the impl is built on, and never leaves, the consumer
    /// thread — only the `build` closure crosses the thread boundary. This
    /// lets backends hold thread-affine handles (e.g. Windows COM SMTC
    /// interfaces) directly.
    pub trait QueuedSink {
        /// Called once on the consumer thread before draining begins.
        fn init(&mut self);
        /// Called for every queued event, in order.
        fn deliver(&mut self, ev: &PlaybackEvent);
        /// Called once on the consumer thread after the queue disconnects.
        fn teardown(&mut self);
    }

    #[derive(Default)]
    struct Pending {
        events: VecDeque<PlaybackEvent>,
        stopped: bool,
    }
    #[derive(Default)]
    struct Queue {
        pending: Mutex<Pending>,
        wake: Condvar,
    }
    static EVENT_TX: Mutex<Option<Arc<Queue>>> = Mutex::new(None);

    fn same_state(a: PlaybackEventKind, b: PlaybackEventKind) -> bool {
        use PlaybackEventKind::*;
        let phase = |kind| {
            matches!(
                kind,
                Started | Paused | Finished | Canceled | Error | TrackLoaded
            )
        };
        a == b || (phase(a) && phase(b))
    }

    impl Pending {
        fn push(&mut self, event: PlaybackEvent) {
            // Bound storage by state fields, not elapsed time or an arbitrary
            // capacity. A final stop/pause always replaces stale phase updates.
            self.events.retain(|old| !same_state(old.kind, event.kind));
            self.events.push_back(event);
        }
    }

    /// The coordinator sink registration is process-wide and survives
    /// stop/run cycles, so it happens at most once.
    static REGISTER_SINK: Once = Once::new();

    // Coordinator-side hook: jfn-playback invokes this for every event.
    // Cloning is cheap relative to the OS transport round-trips that follow.
    fn on_event(ev: &PlaybackEvent) {
        if let Some(tx) = EVENT_TX.lock().as_ref() {
            let mut pending = tx.pending.lock();
            if !pending.stopped {
                pending.push(ev.clone());
            }
            drop(pending);
            tx.wake.notify_one();
        }
    }

    /// Start the process-wide media sink. `build` constructs the platform
    /// [`QueuedSink`] on the consumer thread (so native transport handles are
    /// created there). No-op if already running.
    pub fn run_sink<S, F>(thread_name: &str, build: F)
    where
        S: QueuedSink,
        F: FnOnce() -> S + Send + 'static,
    {
        let tx = Arc::new(Queue::default());
        let rx = Arc::clone(&tx);
        {
            let mut slot = EVENT_TX.lock();
            if slot.is_some() {
                return;
            }
            *slot = Some(tx);
        }

        REGISTER_SINK.call_once(|| crate::ffi::register_event_sink(Box::new(on_event)));

        if let Err(e) = std::thread::Builder::new()
            .name(thread_name.to_owned())
            .spawn(move || consumer_thread(rx, build))
        {
            drop(EVENT_TX.lock().take());
            eprintln!("[playback] failed to spawn media-sink thread: {e}");
        }
    }

    /// Drop the producer end. The consumer delivers whatever is already
    /// queued, tears down, and exits. No-op if not running.
    pub fn stop() {
        if let Some(queue) = EVENT_TX.lock().take() {
            queue.pending.lock().stopped = true;
            queue.wake.notify_one();
        }
    }

    fn consumer_thread<S: QueuedSink>(rx: Arc<Queue>, build: impl FnOnce() -> S) {
        let mut sink = build();
        sink.init();

        loop {
            let (events, stopped) = {
                let mut pending = rx.pending.lock();
                while pending.events.is_empty() && !pending.stopped {
                    rx.wake.wait(&mut pending);
                }
                (std::mem::take(&mut pending.events), pending.stopped)
            };
            for event in events {
                sink.deliver(&event);
            }
            if stopped {
                break;
            }
        }

        sink.teardown();
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn slow_sink_retains_final_phase_and_latest_position() {
            let mut pending = Pending::default();
            for _ in 0..1000 {
                pending.push(PlaybackEvent::new(PlaybackEventKind::Started));
                pending.push(PlaybackEvent::new(PlaybackEventKind::PositionChanged));
            }
            pending.push(PlaybackEvent::new(PlaybackEventKind::Paused));
            assert_eq!(pending.events.len(), 2);
            assert_eq!(
                pending.events.back().map(|e| e.kind),
                Some(PlaybackEventKind::Paused)
            );
            pending.push(PlaybackEvent::new(PlaybackEventKind::Finished));
            assert_eq!(pending.events.len(), 2);
            assert_eq!(
                pending.events.back().map(|e| e.kind),
                Some(PlaybackEventKind::Finished)
            );
        }
    }
}

pub use harness::{Phase, QueuedSink, map_kind_to_phase, run_sink, stop};
