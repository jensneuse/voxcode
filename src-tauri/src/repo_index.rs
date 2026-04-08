//! Pre-indexed repository directory list for fast content search.
//!
//! At app start, scans `~/` for directories containing `.git` (depth ≤ 4),
//! counts files per repo (respecting .gitignore), and sorts lightest-first.
//! Repos that produce search hits are promoted to the front of the list
//! so repeated searches are near-instant.

use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use memchr::memmem;
use notify::{EventKind, RecursiveMode, Watcher};

/// A repo with its file count for sorting.
#[derive(Debug, Clone)]
struct IndexedRepo {
    path: PathBuf,
    file_count: usize,
}

static REPO_INDEX: OnceLock<RwLock<Vec<IndexedRepo>>> = OnceLock::new();

fn index() -> &'static RwLock<Vec<IndexedRepo>> {
    REPO_INDEX.get_or_init(|| RwLock::new(Vec::new()))
}

/// Start background indexing of code repositories under the home directory.
/// Returns immediately — the scan runs on a background thread.
pub fn start_background_index() {
    std::thread::spawn(|| {
        let start = std::time::Instant::now();
        let mut repos = scan_repos();
        // Sort lightest repos first — they search fastest
        repos.sort_by_key(|r| r.file_count);
        let count = repos.len();
        let total_files: usize = repos.iter().map(|r| r.file_count).sum();
        let mut idx = index().write().unwrap();
        *idx = repos;
        log::info!(
            "Repo index: {} repos, {} total files, indexed in {:?}",
            count,
            total_files,
            start.elapsed(),
        );
    });
}

/// Start watching the home directory for new `.git` directories.
/// When a new repo is detected, it is added to the index automatically.
/// Returns the watcher handle — drop it to stop watching.
pub fn start_fs_watcher() -> notify::Result<impl Watcher> {
    let home = dirs::home_dir().expect("no home directory");
    start_fs_watcher_on(&home)
}

/// Start watching a specific directory for new `.git` directories.
/// Used by `start_fs_watcher` (watches `~/`) and by tests (watches a temp dir).
pub fn start_fs_watcher_on(root: &Path) -> notify::Result<impl Watcher> {
    let mut watcher = notify::recommended_watcher(|res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        // Only care about newly created files/dirs
        if !matches!(event.kind, EventKind::Create(_)) {
            return;
        }
        for path in &event.paths {
            if path.file_name().map(|n| n == ".git").unwrap_or(false) {
                if let Some(repo_dir) = path.parent() {
                    log::info!("Watcher detected new repo: {}", repo_dir.display());
                    add_repo(repo_dir.to_path_buf());
                }
            }
        }
    })?;
    watcher.watch(root, RecursiveMode::Recursive)?;
    log::info!("FS watcher started on {}", root.display());
    Ok(watcher)
}

/// Return a snapshot of all indexed repository paths.
pub fn get_repos() -> Vec<PathBuf> {
    index().read().unwrap().iter().map(|r| r.path.clone()).collect()
}

/// Add a single repository to the index (e.g. when a new repo is detected).
pub fn add_repo(path: PathBuf) {
    let file_count = count_repo_files(&path);
    let mut idx = index().write().unwrap();
    // Avoid duplicates
    if !idx.iter().any(|r| r.path == path) {
        idx.push(IndexedRepo { path, file_count });
    }
}

/// Remove a repository from the index.
pub fn remove_repo(path: &Path) {
    let mut idx = index().write().unwrap();
    idx.retain(|r| r.path != path);
}

/// Promote repos that produced hits to the front of the list.
fn promote_repos(hit_paths: &[PathBuf]) {
    if hit_paths.is_empty() {
        return;
    }
    let mut idx = index().write().unwrap();
    // Move hit repos to the front, preserving their relative order
    let mut hits = Vec::new();
    let mut rest = Vec::new();
    for repo in idx.drain(..) {
        if hit_paths.iter().any(|h| h == &repo.path) {
            hits.push(repo);
        } else {
            rest.push(repo);
        }
    }
    hits.extend(rest);
    *idx = hits;
}

/// Scan for git repositories under the home directory.
/// Counts files per repo using `rg --files` which respects .gitignore.
fn scan_repos() -> Vec<IndexedRepo> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };

    let skip_names: &[&str] = &[
        "Library", "Applications", "Movies", "Music", "Pictures",
        "Public", ".Trash", "node_modules", "target", ".build",
        "build", "dist", "vendor", ".cache", ".npm", ".cargo",
        ".rustup", ".local", ".docker", "go",
    ];

    let mut repo_paths = Vec::new();

    for entry in walkdir::WalkDir::new(&home)
        .min_depth(1)
        .max_depth(5)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if !e.file_type().is_dir() {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            if name.starts_with('.') && e.depth() > 1 {
                return name == ".git";
            }
            !skip_names.iter().any(|s| *s == name.as_ref())
        })
        .filter_map(|entry| entry.ok())
    {
        if entry.file_name() == ".git" {
            // .git is a directory in regular repos, a file in worktrees
            if entry.file_type().is_dir() || entry.file_type().is_file() {
                if let Some(parent) = entry.path().parent() {
                    repo_paths.push(parent.to_path_buf());
                }
            }
        }
    }

    // Count files per repo (respects .gitignore via ignore crate)
    repo_paths
        .into_iter()
        .map(|path| {
            let file_count = count_repo_files(&path);
            IndexedRepo { path, file_count }
        })
        .collect()
}

/// Count searchable files in a repo using ignore::Walk (respects .gitignore).
fn count_repo_files(repo: &Path) -> usize {
    ignore::WalkBuilder::new(repo)
        .hidden(true)
        .git_ignore(true)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()))
        .count()
}

/// Search all indexed repos for files containing the given text.
/// Searches lightest repos first, stops all workers when a match is found.
/// Returns `(repo_root, relative_path)` pairs.
pub fn search_repos_for_content(selected_text: &str) -> Vec<(PathBuf, String)> {
    let repos: Vec<IndexedRepo> = index().read().unwrap().clone();
    if repos.is_empty() {
        return Vec::new();
    }

    let probe_lines = pick_probe_lines(selected_text);
    if probe_lines.is_empty() {
        return Vec::new();
    }

    // Build memchr finders for all probes
    let finders: Vec<memmem::Finder<'_>> = probe_lines
        .iter()
        .map(|p| memmem::Finder::new(p.as_bytes()))
        .collect();

    // Single parallel walk across all repos using ignore crate.
    // All repo roots are added to one WalkBuilder; the thread pool is managed
    // by ignore. WalkState::Quit provides early exit on first match.
    let done = std::sync::atomic::AtomicBool::new(false);
    let results_mu = std::sync::Mutex::new(Vec::new());

    let repo_paths: Vec<PathBuf> = repos.iter().map(|r| r.path.clone()).collect();

    let mut builder = ignore::WalkBuilder::new(&repo_paths[0]);
    for p in &repo_paths[1..] {
        builder.add(p);
    }
    builder
        .hidden(true)
        .git_ignore(true)
        .threads(
            std::thread::available_parallelism()
                .map(|n| n.get().min(16))
                .unwrap_or(8),
        )
        .build_parallel()
        .run(|| {
            let finders = &finders;
            let done = &done;
            let results_mu = &results_mu;
            let repo_paths = &repo_paths;
            Box::new(move |entry| {
                if done.load(std::sync::atomic::Ordering::Relaxed) {
                    return ignore::WalkState::Quit;
                }
                let Ok(entry) = entry else {
                    return ignore::WalkState::Continue;
                };
                if !entry.file_type().map_or(false, |ft| ft.is_file()) {
                    return ignore::WalkState::Continue;
                }
                if let Ok(contents) = std::fs::read(entry.path()) {
                    if finders.iter().all(|f| f.find(&contents).is_some()) {
                        let path = entry.path();
                        for repo in repo_paths {
                            if let Ok(rel) = path.strip_prefix(repo) {
                                results_mu
                                    .lock()
                                    .unwrap()
                                    .push((repo.clone(), rel.to_string_lossy().to_string()));
                                done.store(true, std::sync::atomic::Ordering::Relaxed);
                                return ignore::WalkState::Quit;
                            }
                        }
                    }
                }
                ignore::WalkState::Continue
            })
        });

    let results = results_mu.into_inner().unwrap();

    // Promote hit repos to the front for next search
    let hit_paths: Vec<PathBuf> = results.iter().map(|(r, _)| r.clone()).collect();
    promote_repos(&hit_paths);

    results
}

/// Pick up to 3 non-trivial lines from selected text for content search probes.
fn pick_probe_lines(text: &str) -> Vec<&str> {
    let candidates: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| l.len() >= 15)
        .filter(|l| {
            !l.chars().all(|c| matches!(c, '{' | '}' | '(' | ')' | '[' | ']' | ';' | ',' | ' '))
        })
        .collect();

    if candidates.is_empty() {
        return Vec::new();
    }
    if candidates.len() <= 2 {
        return candidates;
    }

    let mid = candidates.len() / 2;
    vec![candidates[0], candidates[mid], candidates[candidates.len() - 1]]
}
