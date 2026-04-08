use voxcode::repo_index;

#[test]
fn add_and_get_repos() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_path = tmp.path().to_path_buf();

    // Create a file so count_repo_files finds something
    std::fs::write(repo_path.join("test.txt"), "hello").unwrap();

    repo_index::add_repo(repo_path.clone());
    let repos = repo_index::get_repos();
    assert!(repos.contains(&repo_path));
}

#[test]
fn add_repo_avoids_duplicates() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_path = tmp.path().to_path_buf();
    std::fs::write(repo_path.join("test.txt"), "hello").unwrap();

    repo_index::add_repo(repo_path.clone());
    let count_before = repo_index::get_repos()
        .iter()
        .filter(|r| **r == repo_path)
        .count();

    repo_index::add_repo(repo_path.clone());
    let count_after = repo_index::get_repos()
        .iter()
        .filter(|r| **r == repo_path)
        .count();

    assert_eq!(count_before, count_after);
}

#[test]
fn remove_repo_removes_from_index() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_path = tmp.path().to_path_buf();
    std::fs::write(repo_path.join("test.txt"), "hello").unwrap();

    repo_index::add_repo(repo_path.clone());
    assert!(repo_index::get_repos().contains(&repo_path));

    repo_index::remove_repo(&repo_path);
    assert!(!repo_index::get_repos().contains(&repo_path));
}

#[test]
fn search_repos_finds_file_by_content() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_path = tmp.path().to_path_buf();

    // Create a file with unique content (must be >= 15 chars for probe lines)
    let unique_content = "repo_index_test_unique_search_marker_42";
    std::fs::write(repo_path.join("searchable.txt"), unique_content).unwrap();

    repo_index::add_repo(repo_path.clone());

    let results = repo_index::search_repos_for_content(unique_content);
    assert!(!results.is_empty(), "Expected to find file with unique content");
    assert_eq!(results[0].0, repo_path);
    assert_eq!(results[0].1, "searchable.txt");
}

#[test]
fn search_repos_empty_text_returns_empty() {
    let results = repo_index::search_repos_for_content("");
    assert!(results.is_empty());
}

#[test]
fn search_repos_short_text_returns_empty() {
    // Lines under 15 chars are filtered by pick_probe_lines
    let results = repo_index::search_repos_for_content("short");
    assert!(results.is_empty());
}

#[test]
fn search_repos_nonexistent_content_returns_empty() {
    let results = repo_index::search_repos_for_content(
        "this_content_absolutely_does_not_exist_anywhere_xyz_999",
    );
    assert!(results.is_empty());
}
