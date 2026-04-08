#![cfg(target_os = "macos")]

use voxcode::window_ext;

#[test]
fn get_monitor_names_returns_nonempty() {
    let names = window_ext::get_monitor_names();
    // CI runners have at least one virtual display
    assert!(!names.is_empty(), "Expected at least one monitor name");
    for name in &names {
        assert!(!name.is_empty(), "Monitor name should not be empty");
    }
}

#[test]
fn get_frontmost_app_pid_returns_some() {
    let pid = window_ext::get_frontmost_app_pid();
    // On macOS CI there's always a frontmost app (Finder at minimum)
    assert!(pid.is_some(), "Expected a frontmost app PID");
    assert!(pid.unwrap() > 0, "PID should be positive");
}

#[test]
fn play_system_sound_does_not_panic() {
    // Verify the FFI call doesn't crash — sound may not actually play in CI
    window_ext::play_system_sound("Tink");
}

#[test]
fn reactivate_app_by_pid_does_not_panic_for_invalid_pid() {
    // Should handle non-existent PID gracefully (logs warning, doesn't crash)
    window_ext::reactivate_app_by_pid(999_999);
}
