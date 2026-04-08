use std::sync::atomic::Ordering;
use voxcode::state::{AppState, RecordingState, SessionState};

#[test]
fn push_transcript_history_adds_to_front() {
    let mut state = AppState {
        recording_state: RecordingState::Idle,
        session_id: 0,
        selected_monitor: 0,
        last_recording_start: std::time::Instant::now(),
        previous_app_pid: None,
        transcript_history: Default::default(),
    };

    state.push_transcript_history("first".into());
    state.push_transcript_history("second".into());

    assert_eq!(state.transcript_history[0], "second");
    assert_eq!(state.transcript_history[1], "first");
}

#[test]
fn push_transcript_history_caps_at_three() {
    let mut state = AppState {
        recording_state: RecordingState::Idle,
        session_id: 0,
        selected_monitor: 0,
        last_recording_start: std::time::Instant::now(),
        previous_app_pid: None,
        transcript_history: Default::default(),
    };

    state.push_transcript_history("a".into());
    state.push_transcript_history("b".into());
    state.push_transcript_history("c".into());
    state.push_transcript_history("d".into());

    assert_eq!(state.transcript_history.len(), 3);
    assert_eq!(state.transcript_history[0], "d");
    assert_eq!(state.transcript_history[1], "c");
    assert_eq!(state.transcript_history[2], "b");
}

#[test]
fn push_transcript_history_empty_string() {
    let mut state = AppState {
        recording_state: RecordingState::Idle,
        session_id: 0,
        selected_monitor: 0,
        last_recording_start: std::time::Instant::now(),
        previous_app_pid: None,
        transcript_history: Default::default(),
    };

    state.push_transcript_history("".into());
    assert_eq!(state.transcript_history.len(), 1);
    assert_eq!(state.transcript_history[0], "");
}

#[test]
fn session_state_new_signals_are_false() {
    let ss = SessionState::new();
    assert!(!ss.stop_signal.load(Ordering::Relaxed));
    assert!(!ss.cancel_signal.load(Ordering::Relaxed));
}

#[test]
fn session_state_begin_session_resets() {
    let ss = SessionState::new();
    ss.signal_cancel();
    ss.set_reference_text("hello");

    ss.begin_session();

    assert!(!ss.stop_signal.load(Ordering::Relaxed));
    assert!(!ss.cancel_signal.load(Ordering::Relaxed));
    assert_eq!(ss.get_reference_text(), "");
}

#[test]
fn session_state_signal_stop() {
    let ss = SessionState::new();
    ss.signal_stop();

    assert!(ss.stop_signal.load(Ordering::Relaxed));
    assert!(!ss.cancel_signal.load(Ordering::Relaxed));
}

#[test]
fn session_state_signal_cancel_sets_both() {
    let ss = SessionState::new();
    ss.signal_cancel();

    assert!(ss.stop_signal.load(Ordering::Relaxed));
    assert!(ss.cancel_signal.load(Ordering::Relaxed));
}

#[test]
fn session_state_reference_text_round_trip() {
    let ss = SessionState::new();
    ss.set_reference_text("some code snippet");
    assert_eq!(ss.get_reference_text(), "some code snippet");
}

#[test]
fn recording_state_serializes_to_lowercase() {
    let json = serde_json::to_string(&RecordingState::Idle).unwrap();
    assert_eq!(json, "\"idle\"");

    let json = serde_json::to_string(&RecordingState::Recording).unwrap();
    assert_eq!(json, "\"recording\"");

    let json = serde_json::to_string(&RecordingState::Processing).unwrap();
    assert_eq!(json, "\"processing\"");
}

#[test]
fn recording_state_deserializes_from_lowercase() {
    let state: RecordingState = serde_json::from_str("\"idle\"").unwrap();
    assert_eq!(state, RecordingState::Idle);

    let state: RecordingState = serde_json::from_str("\"recording\"").unwrap();
    assert_eq!(state, RecordingState::Recording);

    let state: RecordingState = serde_json::from_str("\"processing\"").unwrap();
    assert_eq!(state, RecordingState::Processing);
}
