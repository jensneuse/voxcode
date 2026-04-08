use voxcode::editor_context::{resolve_editor_context, RawEditorCapture};

#[test]
fn no_hints_no_selection_returns_empty_matches() {
    let raw = RawEditorCapture {
        selected_text: String::new(),
        ax_document: None,
        window_title: None,
        ax_line_start: None,
        ax_line_end: None,
    };
    let ctx = resolve_editor_context(raw);
    assert!(ctx.file_matches.is_empty());
}

#[test]
fn ax_document_resolves_to_relative_path() {
    // Create a temp dir with a file
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

    let url = format!("file://{}", file_path.display());
    let raw = RawEditorCapture {
        selected_text: "println!(\"hello\")".to_string(),
        ax_document: Some(url),
        window_title: Some(format!(
            "project [{}] \u{2013} main.rs",
            tmp.path().display()
        )),
        ax_line_start: None,
        ax_line_end: None,
    };

    let ctx = resolve_editor_context(raw);
    assert_eq!(ctx.file_matches.len(), 1);
    assert_eq!(ctx.file_matches[0].path, "main.rs");
}

#[test]
fn content_search_fallback_finds_correct_file() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    let file_path = src.join("handler.rs");
    std::fs::write(
        &file_path,
        "fn unique_handler_content_for_test_abc123() {\n    let x = 42;\n    x + 1\n}\n",
    )
    .unwrap();

    // No AXDocument, only window title with project root
    let raw = RawEditorCapture {
        selected_text:
            "fn unique_handler_content_for_test_abc123() {\n    let x = 42;\n    x + 1\n}"
                .to_string(),
        ax_document: None,
        window_title: Some(format!(
            "project [{}] \u{2013} handler.rs",
            tmp.path().display()
        )),
        ax_line_start: None,
        ax_line_end: None,
    };

    let ctx = resolve_editor_context(raw);
    assert!(!ctx.file_matches.is_empty());
    assert_eq!(ctx.file_matches[0].path, "src/handler.rs");
}

#[test]
fn goland_diff_viewer_wrong_title_correct_content() {
    // Regression: GoLand diff viewer shows wrong file in title,
    // but content search should find the right file.
    let tmp = tempfile::tempdir().unwrap();
    let wrong_file = tmp.path().join("wrong.rs");
    let right_file = tmp.path().join("right.rs");
    std::fs::write(&wrong_file, "fn wrong_content() {}\n").unwrap();
    std::fs::write(
        &right_file,
        "fn goland_regression_unique_marker_xyz() {\n    let val = 99;\n    val * 2\n}\n",
    )
    .unwrap();

    let raw = RawEditorCapture {
        selected_text:
            "fn goland_regression_unique_marker_xyz() {\n    let val = 99;\n    val * 2\n}"
                .to_string(),
        ax_document: None,
        window_title: Some(format!(
            "project [{}] \u{2013} Commit: wrong.rs",
            tmp.path().display()
        )),
        ax_line_start: None,
        ax_line_end: None,
    };

    let ctx = resolve_editor_context(raw);
    assert!(!ctx.file_matches.is_empty());
    // Content search should find right.rs, not wrong.rs
    assert_eq!(ctx.file_matches[0].path, "right.rs");
}
