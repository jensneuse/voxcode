use voxcode::editor_context::{find_file_by_content, find_files_by_content};

fn testfiles_dir() -> String {
    format!("{}/testfiles", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn finds_file_with_unique_content() {
    let result = find_file_by_content(
        &testfiles_dir(),
        "voxcode_test_content_marker_xyz",
    );
    assert_eq!(result.as_deref(), Some("sample.rs"));
}

#[test]
fn finds_nested_file_by_content() {
    let result = find_file_by_content(
        &testfiles_dir(),
        "uniqueTsIdentifier_5c8a2e",
    );
    assert_eq!(result.as_deref(), Some("nested/deep.ts"));
}

#[test]
fn finds_go_file_by_content() {
    let result = find_file_by_content(
        &testfiles_dir(),
        "uniqueGoIdentifier_7d4e1c",
    );
    assert_eq!(result.as_deref(), Some("sample.go"));
}

#[test]
fn returns_none_for_nonexistent_content() {
    let result = find_file_by_content(
        &testfiles_dir(),
        "this_string_does_not_exist_anywhere_in_testfiles_12345",
    );
    assert!(result.is_none());
}

#[test]
fn empty_text_returns_none() {
    let result = find_file_by_content(&testfiles_dir(), "");
    assert!(result.is_none());
}

#[test]
fn short_text_under_15_chars_returns_none() {
    // pick_probe_lines filters lines < 15 chars
    let result = find_file_by_content(&testfiles_dir(), "short");
    assert!(result.is_none());
}

#[test]
fn content_in_only_duplicate_file() {
    let result = find_file_by_content(
        &testfiles_dir(),
        "this_content_only_in_duplicate_file",
    );
    assert_eq!(result.as_deref(), Some("duplicate_content.rs"));
}

#[test]
fn shared_content_returns_both_files() {
    // Content shared between sample.rs and duplicate_content.rs
    let results = find_files_by_content(
        &testfiles_dir(),
        "fn voxcode_sample_fixture_alpha() -> u32 {\n    let unique_identifier_9a3f2b = 42;",
    );
    assert!(results.len() >= 2, "Expected 2+ matches, got {}: {:?}", results.len(), results);
    assert!(results.contains(&"sample.rs".to_string()));
    assert!(results.contains(&"duplicate_content.rs".to_string()));
}
