use voxcode::editor_context::{find_file_in_dir, parse_ax_document_url, resolve_file_path};

// --- parse_ax_document_url ---

#[test]
fn ax_url_valid_path() {
    let result = parse_ax_document_url("file:///Users/jens/code/main.rs");
    assert_eq!(result.as_deref(), Some("/Users/jens/code/main.rs"));
}

#[test]
fn ax_url_percent_encoded_spaces() {
    let result = parse_ax_document_url("file:///Users/jens/my%20project/file.rs");
    assert_eq!(result.as_deref(), Some("/Users/jens/my project/file.rs"));
}

#[test]
fn ax_url_non_file_returns_none() {
    assert!(parse_ax_document_url("https://example.com/file.rs").is_none());
}

#[test]
fn ax_url_empty_path_returns_none() {
    assert!(parse_ax_document_url("file://").is_none());
}

// --- resolve_file_path ---

#[test]
fn resolve_absolute_with_root_returns_relative() {
    let result = resolve_file_path(
        Some("/Users/jens/code/project/src/main.rs"),
        None,
        Some("/Users/jens/code/project"),
    );
    assert_eq!(result.as_deref(), Some("src/main.rs"));
}

#[test]
fn resolve_absolute_without_root_returns_filename() {
    let result = resolve_file_path(Some("/Users/jens/code/main.rs"), None, None);
    assert_eq!(result.as_deref(), Some("main.rs"));
}

#[test]
fn resolve_all_none_returns_none() {
    assert!(resolve_file_path(None, None, None).is_none());
}

// --- find_file_in_dir ---

#[test]
fn find_file_exact_match() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let testfiles = format!("{manifest}/testfiles");
    let result = find_file_in_dir(&testfiles, "sample.rs");
    assert_eq!(result.as_deref(), Some("sample.rs"));
}

#[test]
fn find_file_nested() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let testfiles = format!("{manifest}/testfiles");
    let result = find_file_in_dir(&testfiles, "deep.ts");
    assert_eq!(result.as_deref(), Some("nested/deep.ts"));
}

#[test]
fn find_file_prefix_match_truncated() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let testfiles = format!("{manifest}/testfiles");
    // "sample" has no extension — should prefix-match "sample.rs" or "sample.go"
    let result = find_file_in_dir(&testfiles, "sample");
    assert!(result.is_some());
}

#[test]
fn find_file_nonexistent_root_returns_none() {
    assert!(find_file_in_dir("/nonexistent/path/xyz", "file.rs").is_none());
}

#[test]
fn find_file_nonexistent_file_returns_none() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let testfiles = format!("{manifest}/testfiles");
    assert!(find_file_in_dir(&testfiles, "does_not_exist_xyz.rs").is_none());
}
