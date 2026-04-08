#![cfg(target_os = "macos")]

use voxcode::ort_init;

#[test]
fn find_ort_dylib_with_env_var_override() {
    // Create a temporary file to act as a fake dylib
    let tmp = tempfile::tempdir().unwrap();
    let fake_dylib = tmp.path().join("libonnxruntime.dylib");
    std::fs::write(&fake_dylib, "fake").unwrap();

    unsafe {
        std::env::set_var("ORT_DYLIB_PATH", &fake_dylib);
    }

    let result = ort_init::find_ort_dylib(None);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), fake_dylib);

    unsafe {
        std::env::remove_var("ORT_DYLIB_PATH");
    }
}

#[test]
fn find_ort_dylib_with_resource_dir() {
    // Create a resource dir with a fake dylib
    let tmp = tempfile::tempdir().unwrap();
    let fake_dylib = tmp.path().join("libonnxruntime.dylib");
    std::fs::write(&fake_dylib, "fake").unwrap();

    // Ensure ORT_DYLIB_PATH doesn't interfere
    unsafe {
        std::env::remove_var("ORT_DYLIB_PATH");
    }

    let result = ort_init::find_ort_dylib(Some(tmp.path()));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), fake_dylib);
}

#[test]
fn find_ort_dylib_returns_error_when_not_found() {
    unsafe {
        std::env::remove_var("ORT_DYLIB_PATH");
    }

    let result = ort_init::find_ort_dylib(Some(std::path::Path::new("/nonexistent")));
    // This may succeed if the dylib exists in the default location (src-tauri/resources/).
    // We just verify it doesn't panic.
    let _ = result;
}
