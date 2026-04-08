/// A file that matched the selected text, with optional line numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct FileMatch {
    pub path: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

/// Context captured from the frontmost editor when recording starts.
#[derive(Debug)]
pub struct EditorContext {
    pub selected_text: String,
    /// All files that contain the selected text (typically 1, occasionally 2-3).
    pub file_matches: Vec<FileMatch>,
}

impl EditorContext {
    /// First file path (convenience for callers that only need one).
    pub fn file_path(&self) -> Option<&str> {
        self.file_matches.first().map(|m| m.path.as_str())
    }

    /// Format editor context with transcript for pasting.
    ///
    /// Single match:
    /// - File + lines (same line): `file#Lline\ntranscript`
    /// - File + lines (range): `file#Lstart-end\ntranscript`
    /// - File, no lines: `file\n\n```lang\ncode\n```\n\ntranscript`
    ///
    /// Multiple matches (let the LLM pick):
    /// - `Possible files:\nfile1#L1-10\nfile2#L20-30\n\n```lang\ncode\n```\n\ntranscript`
    ///
    /// No file: `transcript`
    pub fn format_with_transcript(&self, transcript: &str) -> String {
        let trimmed = transcript.trim();
        let word_count = trimmed.split_whitespace().count();
        if word_count < 2 && !self.file_matches.is_empty() {
            // Short/no speech — just paste the LOC reference
            let loc = match self.file_matches.len() {
                1 => {
                    let m = &self.file_matches[0];
                    format_path_with_lines(&m.path, m.start_line, m.end_line)
                }
                _ => {
                    let paths: Vec<String> = self.file_matches.iter()
                        .map(|m| format_path_with_lines(&m.path, m.start_line, m.end_line))
                        .collect();
                    paths.join("\n")
                }
            };
            return format!("{loc}\n");
        }

        match self.file_matches.len() {
            0 if self.selected_text.trim().is_empty() => transcript.to_string(),
            0 => {
                // No file match but we have selected code — include it as a
                // fenced reference so the pasted output mirrors the pill.
                format!("```\n{}\n```\n\n{}\n\n\n", self.selected_text, transcript)
            }
            1 => {
                let m = &self.file_matches[0];
                format_single_match(m, &self.selected_text, transcript)
            }
            _ => {
                let paths: Vec<String> = self.file_matches.iter()
                    .map(|m| format_path_with_lines(&m.path, m.start_line, m.end_line))
                    .collect();
                let all_precise = self.file_matches.iter().all(has_precise_lines);
                if self.selected_text.trim().is_empty() || all_precise {
                    format!(
                        "Possible files:\n{}\n\n{}\n\n\n",
                        paths.join("\n"),
                        transcript,
                    )
                } else {
                    let lang = self.file_matches.iter()
                        .find_map(|m| lang_from_extension(&m.path)
                            .filter(|l| !matches!(*l, "markdown" | "text")));
                    let code_block = match lang {
                        Some(l) => format!("```{}\n{}\n```", l, self.selected_text),
                        None => format!("```\n{}\n```", self.selected_text),
                    };
                    format!(
                        "Possible files:\n{}\n\n{}\n\n{}\n\n\n",
                        paths.join("\n"),
                        code_block,
                        transcript,
                    )
                }
            }
        }
    }

    /// Format the resolved editor reference as markdown-style prelude for live display.
    /// When a single match has precise line numbers, omit the code block — the
    /// path#L anchor is sufficient context.
    pub fn format_markdown_reference(&self) -> String {
        let all_precise = !self.file_matches.is_empty()
            && self.file_matches.iter().all(has_precise_lines);

        let code_block = if self.selected_text.trim().is_empty() || all_precise {
            String::new()
        } else {
            let lang = self.file_matches.iter()
                .find_map(|m| lang_from_extension(&m.path)
                    .filter(|l| !matches!(*l, "markdown" | "text")));
            match lang {
                Some(lang) => format!("```{}\n{}\n```", lang, self.selected_text),
                None => format!("```\n{}\n```", self.selected_text),
            }
        };

        let reference = match self.file_matches.len() {
            0 => String::new(),
            1 => {
                let m = &self.file_matches[0];
                format_path_with_lines(&m.path, m.start_line, m.end_line)
            }
            _ => {
                let paths: Vec<String> = self.file_matches.iter()
                    .map(|m| format_path_with_lines(&m.path, m.start_line, m.end_line))
                    .collect();
                format!("Possible files:\n{}", paths.join("\n"))
            }
        };

        match (reference.is_empty(), code_block.is_empty()) {
            (true, true) => String::new(),
            (false, true) => format!("{reference}\n\n"),
            (true, false) => format!("{code_block}\n\n"),
            (false, false) => format!("{reference}\n\n{code_block}\n\n"),
        }
    }
}

fn format_path_with_lines(path: &str, start: Option<usize>, end: Option<usize>) -> String {
    match (start, end) {
        (Some(s), Some(e)) if s == e => format!("{}#L{}", path, s),
        (Some(s), Some(e)) => format!("{}#L{}-{}", path, s, e),
        _ => path.to_string(),
    }
}

fn has_precise_lines(m: &FileMatch) -> bool {
    m.start_line.is_some() && m.end_line.is_some()
}

fn format_single_match(m: &FileMatch, selected_text: &str, transcript: &str) -> String {
    let path_line = format_path_with_lines(&m.path, m.start_line, m.end_line);
    if selected_text.trim().is_empty() || has_precise_lines(m) {
        return format!("{path_line}\n{transcript}\n\n\n");
    }
    let lang = lang_from_extension(&m.path);
    let code_block = match lang {
        Some(l) => format!("```{l}\n{selected_text}\n```"),
        None => format!("```\n{selected_text}\n```"),
    };
    format!("{path_line}\n\n{code_block}\n\n{transcript}\n\n\n")
}

/// Info extracted from the IDE window title.
#[derive(Debug, PartialEq)]
pub struct TitleInfo {
    pub filename: Option<String>,
    pub project_root: Option<String>,
}

/// Parse an IDE window title to extract filename and project root.
pub fn parse_window_title(title: &str) -> TitleInfo {
    // Try JetBrains format: `project [path] – filename` (en dash)
    // In diff viewer mode, title may be: `project [path] – Commit: filename`
    if let Some(idx) = title.rfind(" \u{2013} ") {
        let after = title[idx + " \u{2013} ".len()..].trim();
        let project_root = extract_bracketed_path(title).or_else(|| {
            // No bracketed path — use the project name (text before en dash) to find root
            let project_name = title[..idx].trim();
            // Strip any bracketed suffix from project name itself
            let project_name = project_name.split('[').next().unwrap_or(project_name).trim();
            if !project_name.is_empty() {
                find_project_root_by_name(project_name)
            } else {
                None
            }
        });
        // Strip known JetBrains prefixes (diff viewer, etc.)
        let cleaned = strip_jetbrains_prefix(after);
        // GoLand truncates long filenames with "..." or "…"
        let (cleaned, was_truncated) = strip_truncation_suffix(cleaned);
        if looks_like_filename(cleaned) {
            return TitleInfo {
                filename: Some(cleaned.to_string()),
                project_root,
            };
        }
        // Truncated name without extension — return as prefix for search
        if was_truncated && !cleaned.is_empty() {
            return TitleInfo {
                filename: Some(cleaned.to_string()),
                project_root,
            };
        }
    }

    // Try VS Code / Xcode format: `filename — project` (em dash)
    // In diff view: `file.rs (abc123) ↔ file.rs (def456) (file.rs) — project`
    if let Some(idx) = title.find(" \u{2014} ") {
        let before = title[..idx].trim();
        let project_name = title[idx + " \u{2014} ".len()..].trim();
        if looks_like_filename(before) {
            return TitleInfo {
                filename: Some(before.to_string()),
                project_root: find_project_root_by_name(project_name),
            };
        }
        // VS Code diff view: extract filename from before the first ` (`
        if let Some(paren_idx) = before.find(" (") {
            let candidate = before[..paren_idx].trim();
            if looks_like_filename(candidate) {
                return TitleInfo {
                    filename: Some(candidate.to_string()),
                    project_root: find_project_root_by_name(project_name),
                };
            }
        }
    }

    TitleInfo {
        filename: None,
        project_root: None,
    }
}

fn extract_bracketed_path(title: &str) -> Option<String> {
    let start = title.find('[')?;
    let end = title.find(']')?;
    if end > start + 1 {
        Some(title[start + 1..end].to_string())
    } else {
        None
    }
}

/// Strip trailing "..." or "…" from a filename that GoLand truncated.
/// Returns (stripped_str, was_truncated).
fn strip_truncation_suffix(s: &str) -> (&str, bool) {
    if let Some(rest) = s.strip_suffix("...") {
        (rest, true)
    } else if let Some(rest) = s.strip_suffix('\u{2026}') {
        (rest, true)
    } else {
        (s, false)
    }
}

/// Strip known JetBrains title prefixes (e.g., "Commit: " in diff viewer).
fn strip_jetbrains_prefix(s: &str) -> &str {
    for prefix in &["Commit: ", "Diff: ", "Merge: ", "Stash: "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            return rest;
        }
    }
    s
}

/// Try to find a project root directory by searching for a directory with the
/// given name under common locations (home dir, ~/repos, etc.).
fn find_project_root_by_name(name: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }
    let home = dirs::home_dir()?;
    let start = std::time::Instant::now();

    // Search under home with limited depth
    for entry in walkdir::WalkDir::new(&home)
        .max_depth(5)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                if let Some(n) = e.file_name().to_str() {
                    // Skip hidden dirs (except . itself), node_modules, target, etc.
                    if n.starts_with('.') && e.depth() > 0 {
                        return false;
                    }
                    if matches!(n, "node_modules" | "target" | "Library" | "Applications" | ".Trash") {
                        return false;
                    }
                }
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if start.elapsed().as_millis() > 200 {
            log::debug!("Project root search timed out after 200ms");
            break;
        }
        if entry.file_type().is_dir() {
            if let Some(dir_name) = entry.file_name().to_str() {
                if dir_name == name && looks_like_code_project(entry.path()) {
                    let path = entry.path().to_string_lossy().to_string();
                    log::info!("Found project root by name '{}': {}", name, path);
                    return Some(path);
                }
            }
        }
    }

    None
}

/// Check if a directory looks like a code project (has .git, Cargo.toml, package.json, etc.)
fn looks_like_code_project(path: &std::path::Path) -> bool {
    for marker in &[".git", "Cargo.toml", "package.json", "go.mod", "pyproject.toml", "Makefile", "src", "lib"] {
        if path.join(marker).exists() {
            return true;
        }
    }
    false
}

fn looks_like_filename(s: &str) -> bool {
    if let Some(dot_pos) = s.rfind('.') {
        let ext = &s[dot_pos + 1..];
        !ext.is_empty() && ext.len() <= 10 && ext.chars().all(|c| c.is_alphanumeric())
    } else {
        false
    }
}

fn lang_from_extension(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?;
    match ext {
        "rs" => Some("rust"),
        "go" => Some("go"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" => Some("javascript"),
        "py" => Some("python"),
        "rb" => Some("ruby"),
        "java" => Some("java"),
        "kt" | "kts" => Some("kotlin"),
        "swift" => Some("swift"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("cpp"),
        "cs" => Some("csharp"),
        "sql" => Some("sql"),
        "sh" | "bash" | "zsh" => Some("bash"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "json" => Some("json"),
        "html" | "htm" => Some("html"),
        "css" => Some("css"),
        "scss" | "sass" => Some("scss"),
        "md" => Some("markdown"),
        "graphql" | "gql" => Some("graphql"),
        "proto" => Some("protobuf"),
        "dockerfile" => Some("dockerfile"),
        _ => None,
    }
}

/// Parse a `file://` URL from AXDocument into an absolute path.
pub fn parse_ax_document_url(url: &str) -> Option<String> {
    let path = url.strip_prefix("file://")?;
    if path.is_empty() {
        return None;
    }
    Some(percent_decode(path))
}

/// Resolve a file path to a relative path suitable for @-mention output.
pub fn resolve_file_path(
    absolute_path: Option<&str>,
    filename: Option<&str>,
    project_root: Option<&str>,
) -> Option<String> {
    if let Some(abs) = absolute_path {
        let abs_path = std::path::Path::new(abs);
        if let Some(root) = project_root {
            let root_path = std::path::Path::new(root);
            if let Ok(rel) = abs_path.strip_prefix(root_path) {
                return Some(rel.to_string_lossy().to_string());
            }
        }
        return abs_path.file_name()
            .map(|f| f.to_string_lossy().to_string());
    }

    if let Some(name) = filename {
        if let Some(root) = project_root {
            let expanded_root = expand_tilde(root);
            if let Some(found) = find_file_in_dir(&expanded_root, name) {
                return Some(found);
            }
        }
        // Only return unverified filenames if they look complete (have extension).
        // Truncated names (no extension, e.g. from JetBrains "...") are unreliable.
        if looks_like_filename(name) {
            return Some(name.to_string());
        }
        return None;
    }

    None
}

/// Search for a filename in a directory, return relative path to the deepest match.
/// If no exact match and `filename` has no extension (truncated prefix), falls back
/// to prefix matching on file stems.
pub fn find_file_in_dir(root: &str, filename: &str) -> Option<String> {
    use std::time::Instant;

    let start = Instant::now();
    let root_path = std::path::Path::new(root);
    if !root_path.is_dir() {
        return None;
    }

    let mut best_match: Option<String> = None;
    let mut best_depth: usize = 0;
    // Collect prefix matches in the same pass for truncated filenames (no extension)
    let try_prefix = !filename.contains('.');
    let mut prefix_match: Option<String> = None;
    let mut prefix_depth: usize = 0;

    let skip_dirs = ["target", ".git", "node_modules", ".build", "build", "dist"];

    for entry in walkdir::WalkDir::new(root)
        .max_depth(10)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                if let Some(name) = e.file_name().to_str() {
                    if skip_dirs.contains(&name) {
                        return false;
                    }
                }
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if start.elapsed().as_millis() > 100 {
            log::debug!("File search timed out after 100ms");
            break;
        }

        if entry.file_type().is_file() {
            if let Some(name) = entry.file_name().to_str() {
                if name == filename {
                    let depth = entry.depth();
                    if depth > best_depth || best_match.is_none() {
                        if let Ok(rel) = entry.path().strip_prefix(root) {
                            best_match = Some(rel.to_string_lossy().to_string());
                            best_depth = depth;
                        }
                    }
                } else if try_prefix && best_match.is_none() {
                    if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                        if stem.starts_with(filename) {
                            let depth = entry.depth();
                            if depth > prefix_depth || prefix_match.is_none() {
                                if let Ok(rel) = entry.path().strip_prefix(root) {
                                    prefix_match = Some(rel.to_string_lossy().to_string());
                                    prefix_depth = depth;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    best_match.or(prefix_match)
}

/// Search a project for the file containing `selected_text` using ripgrep.
///
/// Picks up to 3 non-trivial lines from the selection, runs `rg -l --fixed-strings`
/// for each, and returns the file that all queries agree on.
///
/// When multiple files match, `filename_hint` (from the window title) is used to
/// prefer a file whose name starts with the hint. Returns None only if no file
/// matches at all.
pub fn find_file_by_content(project_root: &str, selected_text: &str) -> Option<String> {
    find_files_by_content(project_root, selected_text).into_iter().next()
}

/// Search for all files in `project_root` that contain `selected_text`.
/// Returns all unanimous matches (files that match every probe line).
/// When only one probe matches uniquely, returns that single file.
pub fn find_files_by_content(project_root: &str, selected_text: &str) -> Vec<String> {
    let root = std::path::Path::new(project_root);
    if !root.is_dir() {
        return Vec::new();
    }

    let probe_lines = pick_probe_lines(selected_text);
    if probe_lines.is_empty() {
        return Vec::new();
    }

    // Search in-process using ignore::Walk + memmem per probe line
    let mut file_sets: Vec<Vec<String>> = Vec::new();
    for line in &probe_lines {
        let finder = memchr::memmem::Finder::new(line.as_bytes());
        let mut files = Vec::new();
        for entry in ignore::WalkBuilder::new(root)
            .hidden(true)
            .git_ignore(true)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()))
        {
            if let Ok(contents) = std::fs::read(entry.path()) {
                if finder.find(&contents).is_some() {
                    if let Ok(rel) = entry.path().strip_prefix(root) {
                        files.push(rel.to_string_lossy().to_string());
                    }
                }
            }
        }
        if !files.is_empty() {
            file_sets.push(files);
        }
    }

    if file_sets.is_empty() {
        return Vec::new();
    }

    // Find files that appear in ALL result sets
    let first = &file_sets[0];
    let unanimous: Vec<String> = first
        .iter()
        .filter(|f| file_sets[1..].iter().all(|set| set.contains(f)))
        .cloned()
        .collect();

    if !unanimous.is_empty() {
        return unanimous;
    }

    // No unanimous agreement — count frequency across all sets
    let mut counts = std::collections::HashMap::new();
    for set in &file_sets {
        for f in set {
            *counts.entry(f.as_str()).or_insert(0usize) += 1;
        }
    }
    let max_count = counts.values().copied().max().unwrap_or(0);
    let top: Vec<String> = counts.iter()
        .filter(|(_, &c)| c == max_count)
        .map(|(f, _)| f.to_string())
        .collect();

    top
}

/// Pick up to 3 non-trivial lines from selected text for content search probes.
/// Skips empty lines, whitespace-only lines, and very short lines (< 15 chars).
/// Picks from start, middle, and end to maximize uniqueness.
fn pick_probe_lines(text: &str) -> Vec<&str> {
    let candidates: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| l.len() >= 15)
        .filter(|l| {
            // Skip lines that are just braces, parens, or common syntax
            !l.chars().all(|c| matches!(c, '{' | '}' | '(' | ')' | '[' | ']' | ';' | ',' | ' '))
        })
        .collect();

    if candidates.is_empty() {
        return Vec::new();
    }
    if candidates.len() == 1 {
        return vec![candidates[0]];
    }
    if candidates.len() == 2 {
        return vec![candidates[0], candidates[1]];
    }

    // Pick first, middle, last
    let mid = candidates.len() / 2;
    vec![candidates[0], candidates[mid], candidates[candidates.len() - 1]]
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

fn percent_decode(s: &str) -> String {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

/// Search file contents for selected_text, return 1-indexed line range.
/// Returns (None, None) if text appears 0 or 2+ times (ambiguous).
pub fn find_lines_in_content(contents: &str, selected_text: &str) -> (Option<usize>, Option<usize>) {
    let trimmed = selected_text.trim();
    if trimmed.is_empty() {
        return (None, None);
    }

    let mut matches = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = contents[search_from..].find(trimmed) {
        matches.push(search_from + pos);
        search_from += pos + 1;
        if matches.len() > 1 {
            return (None, None);
        }
    }

    if matches.len() != 1 {
        return (None, None);
    }

    let match_pos = matches[0];
    let start_line = contents[..match_pos].matches('\n').count() + 1;
    let end_line = start_line + trimmed.matches('\n').count();

    (Some(start_line), Some(end_line))
}
/// Find the line range of `selected_text` in a file at an absolute path.
pub fn find_lines_in_file_absolute(abs_path: &str, selected_text: &str) -> (Option<usize>, Option<usize>) {
    match std::fs::read_to_string(abs_path) {
        Ok(contents) => find_lines_in_content(&contents, selected_text),
        Err(_) => (None, None),
    }
}

/// Find the line range of `selected_text` in a file relative to a root directory.
pub fn find_lines_in_file(root: &str, relative_path: &str, selected_text: &str) -> (Option<usize>, Option<usize>) {
    let full_path = std::path::Path::new(root).join(relative_path);
    match std::fs::read_to_string(&full_path) {
        Ok(contents) => find_lines_in_content(&contents, selected_text),
        Err(_) => (None, None),
    }
}

#[cfg(target_os = "macos")]
mod ax {
    use std::ffi::c_void;

    pub type AXUIElementRef = *mut c_void;
    pub type CFTypeRef = *mut c_void;
    pub type CFStringRef = *mut c_void;
    pub type AXValueRef = *mut c_void;
    pub type CFIndex = isize;

    pub const K_AX_ERROR_SUCCESS: i32 = 0;
    pub const K_AX_VALUE_TYPE_CF_RANGE: i32 = 4;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct CFRange {
        pub location: CFIndex,
        pub length: CFIndex,
    }

    extern "C" {
        pub fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
        pub fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
        pub fn AXUIElementCopyParameterizedAttributeValue(
            element: AXUIElementRef,
            attr: CFStringRef,
            param: CFTypeRef,
            value: *mut CFTypeRef,
        ) -> i32;
        pub fn CFRelease(cf: CFTypeRef);
        pub fn CFStringCreateWithCString(
            alloc: CFTypeRef,
            c_str: *const u8,
            encoding: u32,
        ) -> CFStringRef;
        pub fn CFStringGetLength(s: CFStringRef) -> CFIndex;
        pub fn CFStringGetCString(
            s: CFStringRef,
            buf: *mut u8,
            buf_size: CFIndex,
            encoding: u32,
        ) -> bool;
        pub fn AXValueGetValue(
            value: AXValueRef,
            value_type: i32,
            value_out: *mut c_void,
        ) -> bool;
        pub fn CFNumberCreate(
            alloc: CFTypeRef,
            number_type: i32,
            value: *const c_void,
        ) -> CFTypeRef;
    }

    pub const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;
    pub const K_CF_NUMBER_CF_INDEX_TYPE: i32 = 14;

    pub unsafe fn cf_string(s: &str) -> CFStringRef {
        let cstr = std::ffi::CString::new(s).unwrap();
        CFStringCreateWithCString(std::ptr::null_mut(), cstr.as_ptr() as *const u8, K_CF_STRING_ENCODING_UTF8)
    }

    pub unsafe fn cfstring_to_string(s: CFStringRef) -> Option<String> {
        if s.is_null() {
            return None;
        }
        let len = CFStringGetLength(s);
        let buf_size = (len * 4 + 1) as usize;
        let mut buf = vec![0u8; buf_size];
        if CFStringGetCString(s, buf.as_mut_ptr(), buf_size as CFIndex, K_CF_STRING_ENCODING_UTF8) {
            let cstr = std::ffi::CStr::from_ptr(buf.as_ptr() as *const i8);
            Some(cstr.to_string_lossy().to_string())
        } else {
            None
        }
    }

    pub unsafe fn get_ax_string(element: AXUIElementRef, attr_name: &str) -> Option<String> {
        let attr = cf_string(attr_name);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(element, attr, &mut value);
        CFRelease(attr);
        if err != K_AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }
        let result = cfstring_to_string(value);
        CFRelease(value);
        result
    }

    pub unsafe fn get_ax_selected_range(element: AXUIElementRef) -> Option<CFRange> {
        let attr = cf_string("AXSelectedTextRange");
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(element, attr, &mut value);
        CFRelease(attr);
        if err != K_AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }
        let mut range = CFRange { location: 0, length: 0 };
        let ok = AXValueGetValue(
            value,
            K_AX_VALUE_TYPE_CF_RANGE,
            &mut range as *mut CFRange as *mut c_void,
        );
        CFRelease(value);
        if ok { Some(range) } else { None }
    }

    pub unsafe fn get_ax_line_for_index(element: AXUIElementRef, index: CFIndex) -> Option<usize> {
        let attr = cf_string("AXLineForIndex");
        let param = CFNumberCreate(
            std::ptr::null_mut(),
            K_CF_NUMBER_CF_INDEX_TYPE,
            &index as *const CFIndex as *const c_void,
        );
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = AXUIElementCopyParameterizedAttributeValue(element, attr, param, &mut value);
        CFRelease(attr);
        CFRelease(param);
        if err != K_AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }
        extern "C" {
            fn CFNumberGetValue(number: CFTypeRef, number_type: i32, value: *mut c_void) -> bool;
        }
        let mut line: CFIndex = 0;
        let ok = CFNumberGetValue(value, K_CF_NUMBER_CF_INDEX_TYPE, &mut line as *mut CFIndex as *mut c_void);
        CFRelease(value);
        if ok && line >= 0 {
            Some(line as usize)
        } else {
            None
        }
    }

    pub unsafe fn get_focused_window(app: AXUIElementRef) -> Option<AXUIElementRef> {
        let attr = cf_string("AXFocusedWindow");
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(app, attr, &mut value);
        CFRelease(attr);
        if err == K_AX_ERROR_SUCCESS && !value.is_null() {
            Some(value as AXUIElementRef)
        } else {
            None
        }
    }

    pub unsafe fn get_focused_element(app: AXUIElementRef) -> Option<AXUIElementRef> {
        let attr = cf_string("AXFocusedUIElement");
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(app, attr, &mut value);
        CFRelease(attr);
        if err == K_AX_ERROR_SUCCESS && !value.is_null() {
            Some(value as AXUIElementRef)
        } else {
            None
        }
    }
}

/// Raw data captured from the frontmost app's AX state.
/// Capturing this is fast (~5ms for AX, ~30ms with Cmd+C fallback).
/// File resolution and validation happen separately in `resolve_editor_context`.
pub struct RawEditorCapture {
    pub selected_text: String,
    pub ax_line_start: Option<usize>,
    pub ax_line_end: Option<usize>,
    pub ax_document: Option<String>,
    pub window_title: Option<String>,
}

/// Phase 1: Capture raw AX data from the frontmost app.
/// Fast — only AX queries + optional Cmd+C fallback. Must run before pill shows.
/// Returns None if no text is selected.
#[cfg(target_os = "macos")]
pub fn capture_editor_raw(pid: i32) -> Option<RawEditorCapture> {
    let start = std::time::Instant::now();

    unsafe {
        let app = ax::AXUIElementCreateApplication(pid);

        // 1. Try to get selected text and line numbers via AX
        let (selected_text, ax_line_start, ax_line_end) =
            if let Some(focused) = ax::get_focused_element(app) {
                let text = ax::get_ax_string(focused, "AXSelectedText");
                let lines = text.as_ref().and_then(|_| {
                    let range = ax::get_ax_selected_range(focused)?;
                    if range.length == 0 {
                        return None;
                    }
                    let start_line = ax::get_ax_line_for_index(focused, range.location)?;
                    let end_idx = range.location + range.length;
                    let end_line = ax::get_ax_line_for_index(focused, end_idx.saturating_sub(1))?;
                    Some((start_line + 1, end_line + 1))
                });
                ax::CFRelease(focused as ax::CFTypeRef);
                (
                    text,
                    lines.map(|(s, _)| s),
                    lines.map(|(_, e)| e),
                )
            } else {
                (None, None, None)
            };

        // 2. Get file path info from window
        let (ax_document, window_title) =
            if let Some(window) = ax::get_focused_window(app) {
                let doc = ax::get_ax_string(window, "AXDocument");
                let title = ax::get_ax_string(window, "AXTitle");
                ax::CFRelease(window as ax::CFTypeRef);
                (doc, title)
            } else {
                (None, None)
            };

        ax::CFRelease(app as ax::CFTypeRef);

        // If no selected text from AX, fall back to Cmd+C
        let selected_text = match selected_text {
            Some(ref t) if !t.is_empty() => {
                log::info!("Got selected text via AX ({} chars) in {:?}", t.len(), start.elapsed());
                selected_text.unwrap()
            }
            _ => {
                log::debug!("AX selected text unavailable, trying Cmd+C fallback");
                let text = copy_selection_via_clipboard()?;
                if text.is_empty() {
                    log::debug!("No selected text found via AX or clipboard");
                    return None;
                }
                log::info!("Got selected text via Cmd+C ({} chars) in {:?}", text.len(), start.elapsed());
                text
            }
        };

        Some(RawEditorCapture {
            selected_text,
            ax_line_start,
            ax_line_end,
            ax_document,
            window_title,
        })
    }
}

#[cfg(not(target_os = "macos"))]
pub fn capture_editor_raw(_pid: i32) -> Option<RawEditorCapture> {
    None
}

/// Phase 2: Resolve file path and line numbers from raw capture data.
/// This is the slow part (file I/O, walkdir, validation). Safe to run on a background thread.
///
/// Resolution strategy:
/// 1. Collect hints: AXDocument URL → abs path, window title → filename + project root
/// 2. Primary: rg content search — find file(s) containing selected text
///    - Unanimous (1 file) → use it
///    - Multiple matches → prefer the one matching filename/path hint
/// 3. Fallback (no rg, no project root): title-based resolve + walkdir validation
pub fn resolve_editor_context(raw: RawEditorCapture) -> EditorContext {
    let start = std::time::Instant::now();

    // Step 1: Collect hints from AX and window title
    let title_info = raw.window_title.as_deref().map(parse_window_title);
    let abs_path = raw.ax_document.as_deref().and_then(parse_ax_document_url);
    let project_root = title_info.as_ref().and_then(|t| t.project_root.clone());
    let _filename_hint = title_info.as_ref().and_then(|t| t.filename.clone());

    // Step 2: Try AXDocument first (most reliable when available)
    let ax_file = if let Some(ref abs) = abs_path {
        let abs_p = std::path::Path::new(abs);
        if let Some(ref root) = project_root {
            let expanded = expand_tilde(root);
            let root_p = std::path::Path::new(&expanded);
            abs_p.strip_prefix(root_p)
                .ok()
                .map(|rel| rel.to_string_lossy().to_string())
        } else {
            abs_p.file_name().map(|f| f.to_string_lossy().to_string())
        }
    } else {
        None
    };

    // Step 3: Resolve file matches
    let file_matches = if let Some(ax) = ax_file {
        // AXDocument gave us a definitive path — resolve lines for it
        let (s, e) = resolve_lines_for_path(
            &ax, &raw, &abs_path, &project_root,
        );
        vec![FileMatch { path: ax, start_line: s, end_line: e }]
    } else if let Some(ref root) = project_root {
        // Content search — may return multiple candidates
        let expanded_root = expand_tilde(root);
        let found = find_files_by_content(&expanded_root, &raw.selected_text);
        if found.is_empty() {
            // rg found nothing — try filename-based resolve
            match resolve_file_path(None, _filename_hint.as_deref(), Some(root)) {
                Some(fp) => {
                    let (s, e) = find_lines_in_file(&expanded_root, &fp, &raw.selected_text);
                    vec![FileMatch { path: fp, start_line: s, end_line: e }]
                }
                None => Vec::new(),
            }
        } else {
            // Resolve line numbers for each candidate
            found.into_iter().map(|fp| {
                let (s, e) = find_lines_in_file(&expanded_root, &fp, &raw.selected_text);
                FileMatch { path: fp, start_line: s, end_line: e }
            }).collect()
        }
    } else if let Some(ref filename) = _filename_hint {
        // No project root, no AXDocument — filename from window title + AX lines
        if looks_like_filename(filename) {
            // Complete filename (has extension) — trust it
            vec![FileMatch {
                path: filename.clone(),
                start_line: raw.ax_line_start,
                end_line: raw.ax_line_end,
            }]
        } else if !raw.selected_text.is_empty() {
            // Truncated filename (no extension, e.g. JetBrains "...") — try
            // repo index content search to find the real file
            let mut matches = search_repo_index(&raw.selected_text);
            if matches.is_empty() {
                // Content search failed — fall back to truncated name
                vec![FileMatch {
                    path: filename.clone(),
                    start_line: raw.ax_line_start,
                    end_line: raw.ax_line_end,
                }]
            } else {
                // Prefer AX line numbers when repo search didn't find them
                for m in &mut matches {
                    if m.start_line.is_none() {
                        m.start_line = raw.ax_line_start;
                        m.end_line = raw.ax_line_end;
                    }
                }
                matches
            }
        } else {
            vec![FileMatch {
                path: filename.clone(),
                start_line: raw.ax_line_start,
                end_line: raw.ax_line_end,
            }]
        }
    } else if !raw.selected_text.is_empty() {
        // Last resort: no project root, no filename — discover project dirs
        // from the app's open files via lsof, then search for the content.
        search_repo_index(&raw.selected_text)
    } else {
        Vec::new()
    };

    log::info!(
        "Editor context resolved in {:?}: matches={:?}, text_len={}",
        start.elapsed(),
        file_matches,
        raw.selected_text.len()
    );

    EditorContext {
        selected_text: raw.selected_text,
        file_matches,
    }
}

/// Fallback: search the pre-built repo index for files containing the selected
/// text. Each indexed repo is searched individually with rg (fast per repo).
fn search_repo_index(selected_text: &str) -> Vec<FileMatch> {
    let results = crate::repo_index::search_repos_for_content(selected_text);
    if results.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    for (repo_root, rel_path) in &results {
        let root_str = repo_root.to_string_lossy();
        let (s, e) = find_lines_in_file(&root_str, rel_path, selected_text);
        log::info!("Repo index found: {}", rel_path);
        matches.push(FileMatch {
            path: rel_path.clone(),
            start_line: s,
            end_line: e,
        });
    }

    // Prefer matches with precise line numbers (full content match)
    let precise: Vec<FileMatch> = matches
        .iter()
        .filter(|m| has_precise_lines(m))
        .cloned()
        .collect();
    if !precise.is_empty() {
        return precise;
    }

    matches
}

fn resolve_lines_for_path(
    path: &str,
    raw: &RawEditorCapture,
    abs_path: &Option<String>,
    project_root: &Option<String>,
) -> (Option<usize>, Option<usize>) {
    match (raw.ax_line_start, raw.ax_line_end) {
        (Some(s), Some(e)) => (Some(s), Some(e)),
        _ => {
            if let Some(ref root) = project_root {
                let expanded = expand_tilde(root);
                find_lines_in_file(&expanded, path, &raw.selected_text)
            } else if let Some(ref abs) = abs_path {
                find_lines_in_file_absolute(abs, &raw.selected_text)
            } else {
                (None, None)
            }
        }
    }
}

/// Copy the current selection to clipboard via Cmd+C, read it, then restore.
#[cfg(target_os = "macos")]
fn copy_selection_via_clipboard() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let saved = clipboard.get_text().ok();

    // Clear clipboard first so we can detect if Cmd+C wrote something
    let _ = clipboard.clear();

    simulate_cmd_c().ok()?;
    std::thread::sleep(std::time::Duration::from_millis(30));

    let copied = clipboard.get_text().ok().filter(|s| !s.is_empty());

    // Restore original clipboard
    if let Some(original) = saved {
        let _ = clipboard.set_text(original);
    } else {
        let _ = clipboard.clear();
    }

    copied
}

#[cfg(target_os = "macos")]
fn simulate_cmd_c() -> Result<(), String> {
    use std::ffi::c_void;

    type CGEventRef = *mut c_void;
    type CGEventSourceRef = *mut c_void;

    const K_CG_EVENT_SOURCE_STATE_HID: u32 = 1;
    const K_CG_HID_EVENT_TAP: u32 = 0;
    const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x0010_0000;
    const K_VK_C: u16 = 8;

    extern "C" {
        fn CGEventSourceCreate(state: u32) -> CGEventSourceRef;
        fn CGEventCreateKeyboardEvent(
            source: CGEventSourceRef,
            keycode: u16,
            key_down: bool,
        ) -> CGEventRef;
        fn CGEventSetFlags(event: CGEventRef, flags: u64);
        fn CGEventPost(tap: u32, event: CGEventRef);
        fn CFRelease(cf: *mut c_void);
    }

    unsafe {
        let source = CGEventSourceCreate(K_CG_EVENT_SOURCE_STATE_HID);
        if source.is_null() {
            return Err("Failed to create CGEventSource".into());
        }

        let key_down = CGEventCreateKeyboardEvent(source, K_VK_C, true);
        CGEventSetFlags(key_down, K_CG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(K_CG_HID_EVENT_TAP, key_down);

        std::thread::sleep(std::time::Duration::from_millis(10));

        let key_up = CGEventCreateKeyboardEvent(source, K_VK_C, false);
        CGEventSetFlags(key_up, K_CG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(K_CG_HID_EVENT_TAP, key_up);

        CFRelease(key_down);
        CFRelease(key_up);
        CFRelease(source);
    }

    Ok(())
}

/// A finalized segment — editor context paired with its transcribed text.
pub struct CompletedSegment {
    pub editor_context: Option<EditorContext>,
    pub transcript: String,
}

/// Assemble multiple completed segments into the final paste text.
/// Segments with empty/whitespace-only transcripts are omitted.
/// Each non-empty segment is formatted via `EditorContext::format_with_transcript()`
/// and segments are joined directly (each format_with_transcript already ends with \n\n\n).
pub fn assemble_segments(segments: &[CompletedSegment]) -> String {
    let parts: Vec<String> = segments
        .iter()
        .filter(|s| !s.transcript.trim().is_empty())
        .map(|s| match &s.editor_context {
            Some(ctx) => ctx.format_with_transcript(&s.transcript),
            None => s.transcript.clone(),
        })
        .collect();
    parts.join("")
}
