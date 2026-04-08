use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingState {
    Idle,
    Recording,
    Processing,
}

/// State managed by Tauri (.manage()). Tauri wraps with Arc internally.
#[derive(Debug)]
pub struct AppState {
    pub recording_state: RecordingState,
    /// Monotonically increasing session ID for transcript event isolation.
    pub session_id: u64,
    /// Index of the selected monitor for pill placement (0-based)
    pub selected_monitor: usize,
    /// When the last recording started — for debouncing accidental double-taps
    pub last_recording_start: std::time::Instant,
    /// PID of the app that was frontmost when recording started (to restore focus after paste)
    pub previous_app_pid: Option<i32>,
    /// Last 3 transcribed texts (most recent first) for the tray "Recent" submenu
    pub transcript_history: VecDeque<String>,
}

impl AppState {
    /// Push a transcript to the history ring buffer (max 3, most recent first).
    pub fn push_transcript_history(&mut self, text: String) {
        self.transcript_history.push_front(text);
        if self.transcript_history.len() > 3 {
            self.transcript_history.pop_back();
        }
    }
}

/// Runtime ownership for the active recording session: signals, thread handles, and channels.
pub struct SessionState {
    pub stop_signal: Arc<AtomicBool>,
    pub cancel_signal: Arc<AtomicBool>,
    pub reference_text: Mutex<String>,
    pub worker_handle: Mutex<Option<JoinHandle<()>>>,
    pub amplitude_bridge_handle: Mutex<Option<JoinHandle<()>>>,
    pub accum_handle: Mutex<Option<JoinHandle<Vec<f32>>>>,
    pub accum_chunk_sender: Mutex<Option<Sender<Vec<f32>>>>,
}

impl SessionState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            stop_signal: Arc::new(AtomicBool::new(false)),
            cancel_signal: Arc::new(AtomicBool::new(false)),
            reference_text: Mutex::new(String::new()),
            worker_handle: Mutex::new(None),
            amplitude_bridge_handle: Mutex::new(None),
            accum_handle: Mutex::new(None),
            accum_chunk_sender: Mutex::new(None),
        })
    }

    pub fn begin_session(&self) {
        self.stop_signal.store(false, Ordering::Relaxed);
        self.cancel_signal.store(false, Ordering::Relaxed);
        self.reference_text.lock().unwrap().clear();
    }

    pub fn signal_stop(&self) {
        self.stop_signal.store(true, Ordering::Relaxed);
    }

    pub fn signal_cancel(&self) {
        self.cancel_signal.store(true, Ordering::Relaxed);
        self.stop_signal.store(true, Ordering::Relaxed);
    }

    pub fn set_reference_text(&self, text: impl Into<String>) {
        *self.reference_text.lock().unwrap() = text.into();
    }

    pub fn get_reference_text(&self) -> String {
        self.reference_text.lock().unwrap().clone()
    }
}
