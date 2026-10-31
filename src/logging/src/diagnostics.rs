//! Opt-in, payload-free action timeline. Never performs file IO on event threads.
use std::io::Write;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing_appender::non_blocking::{NonBlocking, NonBlockingBuilder, WorkerGuard};

static WRITER: OnceLock<NonBlocking> = OnceLock::new();

/// Only fixed labels can enter the exported timeline: no media URLs or titles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Start,
    Stop,
    Load,
    Loaded,
    End,
    Pause,
    Resume,
    Seek,
    PlaybackRestart,
    Settings,
    About,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Start => "app-start",
            Self::Stop => "app-stop",
            Self::Load => "load-start",
            Self::Loaded => "file-loaded",
            Self::End => "file-end",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Seek => "seek",
            Self::PlaybackRestart => "playback-restart",
            Self::Settings => "settings",
            Self::About => "about",
        }
    }
}

pub struct Session(Option<WorkerGuard>);

impl Drop for Session {
    fn drop(&mut self) {
        mark(Action::Stop);
        // Flush while the app's scope still owns the worker.
        drop(self.0.take());
    }
}

/// Called only in the browser process. The collector supplies a fresh path.
pub fn start() -> std::io::Result<Option<Session>> {
    let Some(path) = std::env::var_os("JELLIUM_DIAGNOSTICS_EVENTS") else {
        return Ok(None);
    };
    let file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)?;
    // Capacity is the logging library's default. Lossy mode avoids blocking mpv.
    let (writer, guard) = NonBlockingBuilder::default().lossy(true).finish(file);
    WRITER
        .set(writer)
        .map_err(|_| std::io::Error::other("diagnostics already started"))?;
    mark(Action::Start);
    Ok(Some(Session(Some(guard))))
}

pub fn mark(action: Action) {
    let Some(writer) = WRITER.get() else { return };
    let Ok(time) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return;
    };
    let line = format!(
        "{{\"type\":\"action\",\"time\":{},\"pid\":{},\"action\":\"{}\",\"dropped\":{}}}\n",
        time.as_millis(),
        std::process::id(),
        action.label(),
        writer.error_counter().dropped_lines()
    );
    let _ = writer.clone().write_all(line.as_bytes());
}
