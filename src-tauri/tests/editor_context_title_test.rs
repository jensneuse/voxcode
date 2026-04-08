use voxcode::editor_context::parse_window_title;

#[test]
fn jetbrains_format_extracts_filename_and_path() {
    let info = parse_window_title("cosmo [~/code/cosmo] \u{2013} router.go");
    assert_eq!(info.filename.as_deref(), Some("router.go"));
}

#[test]
fn jetbrains_diff_viewer_strips_commit_prefix() {
    let info = parse_window_title("cosmo [~/code/cosmo] \u{2013} Commit: handler.go");
    assert_eq!(info.filename.as_deref(), Some("handler.go"));
}

#[test]
fn jetbrains_truncated_filename() {
    let info = parse_window_title("cosmo [~/code/cosmo] \u{2013} very_long_filenam...");
    assert_eq!(info.filename.as_deref(), Some("very_long_filenam"));
}

#[test]
fn jetbrains_truncated_unicode_ellipsis() {
    let info = parse_window_title("cosmo [~/code/cosmo] \u{2013} very_long_filenam\u{2026}");
    assert_eq!(info.filename.as_deref(), Some("very_long_filenam"));
}

#[test]
fn vscode_format_extracts_filename() {
    let info = parse_window_title("main.rs \u{2014} voxcode");
    assert_eq!(info.filename.as_deref(), Some("main.rs"));
}

#[test]
fn vscode_diff_view_extracts_filename() {
    let info = parse_window_title("file.rs (abc123) \u{2194} file.rs (def456) (file.rs) \u{2014} voxcode");
    // Should extract "file.rs" from before the first " ("
    assert_eq!(info.filename.as_deref(), Some("file.rs"));
}

#[test]
fn xcode_format_extracts_filename() {
    let info = parse_window_title("ViewController.swift \u{2014} MyApp");
    assert_eq!(info.filename.as_deref(), Some("ViewController.swift"));
}

#[test]
fn unknown_format_returns_none() {
    let info = parse_window_title("Just some random window title");
    assert!(info.filename.is_none());
}

#[test]
fn empty_string_returns_none() {
    let info = parse_window_title("");
    assert!(info.filename.is_none());
    assert!(info.project_root.is_none());
}

#[test]
fn jetbrains_extracts_bracketed_path() {
    let info = parse_window_title("project [/Users/jens/code/project] \u{2013} main.go");
    assert_eq!(info.filename.as_deref(), Some("main.go"));
    // project_root comes from bracket extraction
    assert!(info.project_root.is_some());
    assert_eq!(info.project_root.as_deref(), Some("/Users/jens/code/project"));
}

#[test]
fn jetbrains_strips_diff_prefix() {
    let info = parse_window_title("cosmo \u{2013} Diff: router.go");
    assert_eq!(info.filename.as_deref(), Some("router.go"));
}

#[test]
fn jetbrains_strips_merge_prefix() {
    let info = parse_window_title("cosmo \u{2013} Merge: config.go");
    assert_eq!(info.filename.as_deref(), Some("config.go"));
}
