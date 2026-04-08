#![cfg(target_os = "macos")]

use voxcode::paste::copy_to_clipboard;

// Clipboard tests must run serially — concurrent access to the system pasteboard crashes.
// Use `cargo test --test macos_clipboard_test -- --test-threads=1`
// The CI workflow sets --test-threads=1 for macOS tests.

#[test]
fn copy_to_clipboard_and_read_back() {
    let text = "voxcode_clipboard_test_marker_42";
    copy_to_clipboard(text).expect("copy_to_clipboard should succeed");

    let mut clipboard = arboard::Clipboard::new().expect("open clipboard");
    let result = clipboard.get_text().expect("read clipboard");
    assert_eq!(result, text);
}

#[test]
fn copy_to_clipboard_unicode() {
    let text = "Ünîcödé test voxcode";
    copy_to_clipboard(text).expect("copy_to_clipboard should succeed");

    let mut clipboard = arboard::Clipboard::new().expect("open clipboard");
    let result = clipboard.get_text().expect("read clipboard");
    assert_eq!(result, text);
}

#[test]
fn copy_to_clipboard_multiline() {
    let text = "line one\nline two\nline three";
    copy_to_clipboard(text).expect("copy_to_clipboard should succeed");

    let mut clipboard = arboard::Clipboard::new().expect("open clipboard");
    let result = clipboard.get_text().expect("read clipboard");
    assert_eq!(result, text);
}

#[test]
fn copy_to_clipboard_empty_string() {
    copy_to_clipboard("").expect("copy_to_clipboard should succeed for empty string");
}
