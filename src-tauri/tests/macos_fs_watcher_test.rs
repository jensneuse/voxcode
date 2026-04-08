#![cfg(target_os = "macos")]

use voxcode::repo_index;

#[test]
fn fs_watcher_detects_new_git_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let tmp_path = tmp.path().canonicalize().unwrap();

    let _watcher = repo_index::start_fs_watcher_on(&tmp_path)
        .expect("start_fs_watcher_on should succeed");

    // Create a new repo inside the watched directory
    let new_repo = tmp_path.join("new-project");
    std::fs::create_dir_all(&new_repo).unwrap();
    std::fs::write(new_repo.join("main.rs"), "fn main() {}").unwrap();

    // git init to create .git directory
    let output = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&new_repo)
        .output()
        .expect("git init should succeed");
    assert!(output.status.success(), "git init failed");

    // Give FSEvents time to deliver the event
    std::thread::sleep(std::time::Duration::from_secs(2));

    let repos = repo_index::get_repos();
    assert!(
        repos.contains(&new_repo),
        "Watcher should have detected new repo at {:?}, repos: {:?}",
        new_repo,
        repos,
    );

    // Cleanup
    repo_index::remove_repo(&new_repo);
}
