use voxcode::editor_context::find_lines_in_content;

#[test]
fn unique_match_returns_line_range() {
    let content = "line one\nline two\nunique target line\nline four\n";
    let (start, end) = find_lines_in_content(content, "unique target line");
    assert_eq!(start, Some(3));
    assert_eq!(end, Some(3));
}

#[test]
fn multiline_match_returns_start_and_end() {
    let content = "aaa\nbbb\nccc\nddd\neee\n";
    let (start, end) = find_lines_in_content(content, "bbb\nccc\nddd");
    assert_eq!(start, Some(2));
    assert_eq!(end, Some(4));
}

#[test]
fn duplicate_match_returns_none() {
    let content = "hello world\nfoo bar\nhello world\n";
    let (start, end) = find_lines_in_content(content, "hello world");
    assert_eq!(start, None);
    assert_eq!(end, None);
}

#[test]
fn no_match_returns_none() {
    let content = "aaa\nbbb\nccc\n";
    let (start, end) = find_lines_in_content(content, "zzz not here");
    assert_eq!(start, None);
    assert_eq!(end, None);
}

#[test]
fn empty_selected_text_returns_none() {
    let content = "aaa\nbbb\n";
    let (start, end) = find_lines_in_content(content, "");
    assert_eq!(start, None);
    assert_eq!(end, None);
}

#[test]
fn whitespace_only_selected_text_returns_none() {
    let content = "aaa\nbbb\n";
    let (start, end) = find_lines_in_content(content, "   \n  ");
    assert_eq!(start, None);
    assert_eq!(end, None);
}

#[test]
fn match_on_first_line() {
    let content = "first line here\nsecond line\n";
    let (start, end) = find_lines_in_content(content, "first line here");
    assert_eq!(start, Some(1));
    assert_eq!(end, Some(1));
}

#[test]
fn match_on_last_line() {
    let content = "aaa\nbbb\nlast line unique\n";
    let (start, end) = find_lines_in_content(content, "last line unique");
    assert_eq!(start, Some(3));
    assert_eq!(end, Some(3));
}
