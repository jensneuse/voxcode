use voxcode::editor_context::{EditorContext, FileMatch};

fn ctx(selected_text: &str, matches: Vec<FileMatch>) -> EditorContext {
    EditorContext {
        selected_text: selected_text.to_string(),
        file_matches: matches,
    }
}

fn file_match(path: &str, start: Option<usize>, end: Option<usize>) -> FileMatch {
    FileMatch {
        path: path.to_string(),
        start_line: start,
        end_line: end,
    }
}

// --- format_path_with_lines (tested via format_with_transcript) ---

#[test]
fn single_match_with_same_start_end_line() {
    let ec = ctx("some code", vec![file_match("src/main.rs", Some(5), Some(5))]);
    let output = ec.format_with_transcript("refactor this");
    assert!(output.starts_with("src/main.rs#L5\n"));
    assert!(output.contains("refactor this"));
}

#[test]
fn single_match_with_line_range() {
    let ec = ctx("some code", vec![file_match("src/main.rs", Some(5), Some(10))]);
    let output = ec.format_with_transcript("refactor this");
    assert!(output.starts_with("src/main.rs#L5-10\n"));
}

#[test]
fn single_match_no_lines_includes_code_fence() {
    let ec = ctx(
        "let x = 42;",
        vec![file_match("src/main.rs", None, None)],
    );
    let output = ec.format_with_transcript("refactor this");
    assert!(output.contains("```rust\nlet x = 42;\n```"));
    assert!(output.contains("refactor this"));
}

#[test]
fn no_file_no_selection_returns_transcript_only() {
    let ec = ctx("", vec![]);
    let output = ec.format_with_transcript("just a note");
    assert_eq!(output, "just a note");
}

#[test]
fn no_file_with_selection_includes_code_fence() {
    let ec = ctx("let x = 42;", vec![]);
    let output = ec.format_with_transcript("refactor this");
    assert!(output.contains("```\nlet x = 42;\n```"));
    assert!(output.contains("refactor this"));
}

#[test]
fn short_transcript_with_file_returns_loc_only() {
    let ec = ctx("code", vec![file_match("src/main.rs", Some(5), Some(10))]);
    let output = ec.format_with_transcript("ok");
    assert_eq!(output, "src/main.rs#L5-10\n");
}

#[test]
fn short_transcript_single_word_loc_only() {
    let ec = ctx("code", vec![file_match("src/lib.rs", Some(1), Some(1))]);
    let output = ec.format_with_transcript("here");
    assert_eq!(output, "src/lib.rs#L1\n");
}

#[test]
fn multiple_matches_all_precise_shows_possible_files() {
    let ec = ctx(
        "shared code",
        vec![
            file_match("src/a.rs", Some(1), Some(5)),
            file_match("src/b.rs", Some(10), Some(15)),
        ],
    );
    let output = ec.format_with_transcript("update these files");
    assert!(output.contains("Possible files:"));
    assert!(output.contains("src/a.rs#L1-5"));
    assert!(output.contains("src/b.rs#L10-15"));
    assert!(output.contains("update these files"));
}

#[test]
fn multiple_matches_not_precise_includes_code() {
    let ec = ctx(
        "let shared = true;",
        vec![
            file_match("src/a.rs", None, None),
            file_match("src/b.rs", None, None),
        ],
    );
    let output = ec.format_with_transcript("fix this");
    assert!(output.contains("Possible files:"));
    assert!(output.contains("let shared = true;"));
}

// --- format_markdown_reference ---

#[test]
fn markdown_ref_single_with_lines() {
    let ec = ctx("code", vec![file_match("src/main.rs", Some(5), Some(10))]);
    let output = ec.format_markdown_reference();
    assert!(output.contains("src/main.rs#L5-10"));
    // Precise lines => no code block
    assert!(!output.contains("```"));
}

#[test]
fn markdown_ref_single_without_lines() {
    let ec = ctx("let x = 42;", vec![file_match("src/main.rs", None, None)]);
    let output = ec.format_markdown_reference();
    assert!(output.contains("src/main.rs"));
    assert!(output.contains("```rust\nlet x = 42;\n```"));
}

#[test]
fn markdown_ref_no_matches() {
    let ec = ctx("", vec![]);
    let output = ec.format_markdown_reference();
    assert!(output.is_empty());
}

#[test]
fn markdown_ref_multiple_matches() {
    let ec = ctx(
        "code",
        vec![
            file_match("src/a.rs", Some(1), Some(5)),
            file_match("src/b.rs", Some(10), Some(15)),
        ],
    );
    let output = ec.format_markdown_reference();
    assert!(output.contains("Possible files:"));
    assert!(output.contains("src/a.rs#L1-5"));
    assert!(output.contains("src/b.rs#L10-15"));
}

// --- lang_from_extension (tested via code fence output) ---

#[test]
fn rust_extension_produces_rust_fence() {
    let ec = ctx("code here", vec![file_match("src/main.rs", None, None)]);
    let output = ec.format_with_transcript("fix this code");
    assert!(output.contains("```rust\n"));
}

#[test]
fn go_extension_produces_go_fence() {
    let ec = ctx("code here", vec![file_match("cmd/main.go", None, None)]);
    let output = ec.format_with_transcript("fix this code");
    assert!(output.contains("```go\n"));
}

#[test]
fn typescript_extension_produces_typescript_fence() {
    let ec = ctx("code here", vec![file_match("src/app.ts", None, None)]);
    let output = ec.format_with_transcript("fix this code");
    assert!(output.contains("```typescript\n"));
}

#[test]
fn unknown_extension_produces_plain_fence() {
    let ec = ctx("code here", vec![file_match("data.xyz", None, None)]);
    let output = ec.format_with_transcript("fix this code");
    assert!(output.contains("```\ncode here\n```"));
}
