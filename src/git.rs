//! Git integration module for reading HEAD content and checking file status.

use git2::{DiffOptions, Repository, Status};
use std::path::Path;

use crate::file_tree::GitChangedFile;

/// Error types for git operations
#[derive(Debug)]
pub enum GitError {
    /// File is not in a git repository
    NotARepo,
    /// File is not tracked (untracked/new file)
    NotTracked,
    /// File does not exist in HEAD (new file that's staged but not committed)
    NotInHead,
    /// Other git error
    Git(git2::Error),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::NotARepo => write!(f, "Not a git repository"),
            GitError::NotTracked => write!(f, "File is not tracked"),
            GitError::NotInHead => write!(f, "File does not exist in HEAD"),
            GitError::Git(e) => write!(f, "Git error: {}", e),
        }
    }
}

impl From<git2::Error> for GitError {
    fn from(err: git2::Error) -> Self {
        GitError::Git(err)
    }
}

/// Check if a file is inside a git repository
pub fn is_git_available(path: &str) -> bool {
    let path = Path::new(path);
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };

    Repository::discover(&abs_path).is_ok()
}

/// Check if a file is tracked in the git repository (not new/untracked)
pub fn is_file_tracked(path: &str) -> bool {
    let path = Path::new(path);
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };

    // Canonicalize to resolve symlinks (important on macOS where /var -> /private/var)
    let abs_path = abs_path.canonicalize().unwrap_or(abs_path);

    let repo = match Repository::discover(&abs_path) {
        Ok(r) => r,
        Err(_) => return false,
    };

    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return false,
    };

    let relative_path = match abs_path.strip_prefix(workdir) {
        Ok(p) => p,
        Err(_) => return false,
    };

    // Check if file exists in HEAD tree
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return false, // No HEAD means no commits yet
    };

    let commit = match head.peel_to_commit() {
        Ok(c) => c,
        Err(_) => return false,
    };

    let tree = match commit.tree() {
        Ok(t) => t,
        Err(_) => return false,
    };

    // If file exists in HEAD tree, it's tracked
    tree.get_path(relative_path).is_ok()
}

/// Get the content of a file from HEAD
pub fn get_head_content(path: &str) -> Result<String, GitError> {
    let path = Path::new(path);
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|_| GitError::NotARepo)?
    };

    // Canonicalize to resolve symlinks (important on macOS where /var -> /private/var)
    let abs_path = abs_path.canonicalize().map_err(|_| GitError::NotARepo)?;

    let repo = Repository::discover(&abs_path).map_err(|_| GitError::NotARepo)?;

    let workdir = repo.workdir().ok_or(GitError::NotARepo)?;

    let relative_path = abs_path
        .strip_prefix(workdir)
        .map_err(|_| GitError::NotARepo)?;

    // Check if file is tracked
    if !is_file_tracked(&path.to_string_lossy()) {
        return Err(GitError::NotTracked);
    }

    // Get HEAD commit
    let head = repo.head()?;
    let commit = head.peel_to_commit()?;
    let tree = commit.tree()?;

    // Get the file from the tree
    let entry = tree
        .get_path(relative_path)
        .map_err(|_| GitError::NotInHead)?;

    let blob = repo.find_blob(entry.id())?;
    let content = std::str::from_utf8(blob.content())
        .map_err(|_| GitError::Git(git2::Error::from_str("Invalid UTF-8 content")))?;

    Ok(content.to_string())
}

/// Get list of changed files in a git repository
pub fn get_changed_files(root_path: &Path) -> Result<Vec<GitChangedFile>, GitError> {
    let abs_path = if root_path.is_absolute() {
        root_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(root_path))
            .map_err(|_| GitError::NotARepo)?
    };

    // Canonicalize to resolve symlinks (important on macOS where /var -> /private/var)
    let abs_path = abs_path.canonicalize().map_err(|_| GitError::NotARepo)?;

    let repo = Repository::discover(&abs_path).map_err(|_| GitError::NotARepo)?;
    let workdir = repo.workdir().ok_or(GitError::NotARepo)?;

    let mut changed_files = Vec::new();

    // Get status of all files
    let statuses = repo.statuses(None)?;

    for entry in statuses.iter() {
        let status = entry.status();

        // Skip ignored files
        if status.is_ignored() {
            continue;
        }

        // Check if file is changed (modified, new, deleted, etc.)
        let is_changed = status.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE
                | Status::WT_NEW
                | Status::WT_MODIFIED
                | Status::WT_DELETED
                | Status::WT_RENAMED
                | Status::WT_TYPECHANGE,
        );

        if !is_changed {
            continue;
        }

        let is_untracked = status.is_wt_new() && !status.is_index_new();
        let file_path = entry.path().map(|p| workdir.join(p));

        if let Some(file_path) = file_path {
            // Only include files under the requested root path
            if !file_path.starts_with(&abs_path) {
                continue;
            }

            // Get diff stats for this file
            let (added, removed) = get_file_diff_stats(&repo, &file_path, is_untracked)?;

            changed_files.push(GitChangedFile {
                path: file_path,
                added_lines: added,
                removed_lines: removed,
                is_untracked,
            });
        }
    }

    // Sort by path for consistent ordering
    changed_files.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(changed_files)
}

/// Get diff stats (added/removed lines) for a single file
fn get_file_diff_stats(
    repo: &Repository,
    file_path: &Path,
    is_untracked: bool,
) -> Result<(usize, usize), GitError> {
    let workdir = repo.workdir().ok_or(GitError::NotARepo)?;
    let relative_path = file_path
        .strip_prefix(workdir)
        .map_err(|_| GitError::NotARepo)?;

    if is_untracked {
        // For untracked files, count all lines as added
        if let Ok(content) = std::fs::read_to_string(file_path) {
            let line_count = content.lines().count();
            return Ok((line_count, 0));
        }
        return Ok((0, 0));
    }

    // Get HEAD tree
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => {
            // No HEAD means new repo, count all lines as added
            if let Ok(content) = std::fs::read_to_string(file_path) {
                return Ok((content.lines().count(), 0));
            }
            return Ok((0, 0));
        }
    };

    let head_tree = head.peel_to_tree().ok();

    // Create diff options to filter to just this file
    let mut opts = DiffOptions::new();
    opts.pathspec(relative_path);

    let diff = repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))?;

    let mut added = 0;
    let mut removed = 0;

    diff.foreach(
        &mut |_, _| true,
        None,
        None,
        Some(&mut |_, _, line| {
            match line.origin() {
                '+' => added += 1,
                '-' => removed += 1,
                _ => {}
            }
            true
        }),
    )?;

    Ok((added, removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn create_git_repo() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Initialize git repo and verify success
        let output = Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to run git init");
        assert!(output.status.success(), "git init failed: {:?}", output);

        // Configure git user for commits
        let output = Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to run git config email");
        assert!(output.status.success(), "git config email failed: {:?}", output);

        let output = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to run git config name");
        assert!(output.status.success(), "git config name failed: {:?}", output);

        // Disable commit signing for tests (avoid issues with signing hooks)
        let output = Command::new("git")
            .args(["config", "commit.gpgsign", "false"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to run git config gpgsign");
        assert!(output.status.success(), "git config gpgsign failed: {:?}", output);

        dir
    }

    fn add_and_commit_file(dir: &TempDir, filename: &str, content: &str) {
        let file_path = dir.path().join(filename);
        fs::write(&file_path, content).unwrap();

        let output = Command::new("git")
            .args(["add", filename])
            .current_dir(dir.path())
            .output()
            .expect("Failed to run git add");
        assert!(output.status.success(), "git add failed: {:?}", output);

        let output = Command::new("git")
            .args(["commit", "-m", "Add file"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to run git commit");
        assert!(output.status.success(), "git commit failed: {:?}", output);

        // Verify the commit exists by checking HEAD
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to run git rev-parse");
        assert!(output.status.success(), "git rev-parse HEAD failed - commit not created: {:?}", output);
    }

    #[test]
    fn test_is_git_available_no_repo() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "content").unwrap();

        assert!(!is_git_available(file_path.to_str().unwrap()));
    }

    #[test]
    fn test_is_git_available_with_repo() {
        let dir = create_git_repo();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "content").unwrap();

        assert!(is_git_available(file_path.to_str().unwrap()));
    }

    #[test]
    fn test_is_file_tracked_untracked() {
        let dir = create_git_repo();
        let file_path = dir.path().join("untracked.txt");
        fs::write(&file_path, "content").unwrap();

        assert!(!is_file_tracked(file_path.to_str().unwrap()));
    }

    #[test]
    fn test_is_file_tracked_tracked() {
        let dir = create_git_repo();
        add_and_commit_file(&dir, "tracked.txt", "content");

        let file_path = dir.path().join("tracked.txt");
        assert!(is_file_tracked(file_path.to_str().unwrap()));
    }

    #[test]
    fn test_is_file_tracked_new_file() {
        let dir = create_git_repo();

        // Create initial commit so HEAD exists
        add_and_commit_file(&dir, "initial.txt", "initial");

        // Create a new file that's not added
        let file_path = dir.path().join("new_file.txt");
        fs::write(&file_path, "new content").unwrap();

        assert!(!is_file_tracked(file_path.to_str().unwrap()));
    }

    #[test]
    fn test_get_head_content_success() {
        let dir = create_git_repo();
        let original_content = "line1\nline2\nline3";
        add_and_commit_file(&dir, "test.txt", original_content);

        // Modify the file in working directory
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "modified content").unwrap();

        // Should still get the HEAD content
        let result = get_head_content(file_path.to_str().unwrap());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), original_content);
    }

    #[test]
    fn test_get_head_content_untracked() {
        let dir = create_git_repo();

        // Create initial commit so HEAD exists
        add_and_commit_file(&dir, "initial.txt", "initial");

        let file_path = dir.path().join("untracked.txt");
        fs::write(&file_path, "content").unwrap();

        let result = get_head_content(file_path.to_str().unwrap());
        assert!(matches!(result, Err(GitError::NotTracked)));
    }

    #[test]
    fn test_get_head_content_no_repo() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "content").unwrap();

        let result = get_head_content(file_path.to_str().unwrap());
        assert!(matches!(result, Err(GitError::NotARepo)));
    }

    #[test]
    fn test_get_changed_files_no_changes() {
        let dir = create_git_repo();
        add_and_commit_file(&dir, "test.txt", "content");

        let result = get_changed_files(dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_get_changed_files_modified() {
        let dir = create_git_repo();
        add_and_commit_file(&dir, "test.txt", "line1\nline2\nline3");

        // Modify the file
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "line1\nmodified\nline3\nnew line").unwrap();

        let result = get_changed_files(dir.path());
        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 1);
        assert!(!files[0].is_untracked);
        // Should have added and removed lines
        assert!(files[0].added_lines > 0 || files[0].removed_lines > 0);
    }

    #[test]
    fn test_get_changed_files_untracked() {
        let dir = create_git_repo();
        add_and_commit_file(&dir, "initial.txt", "initial");

        // Create an untracked file
        let file_path = dir.path().join("new_file.txt");
        fs::write(&file_path, "line1\nline2\nline3").unwrap();

        let result = get_changed_files(dir.path());
        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].is_untracked);
        assert_eq!(files[0].added_lines, 3); // All lines counted as added
        assert_eq!(files[0].removed_lines, 0);
    }

    #[test]
    fn test_get_changed_files_not_a_repo() {
        let dir = TempDir::new().unwrap();
        let result = get_changed_files(dir.path());
        assert!(matches!(result, Err(GitError::NotARepo)));
    }

    #[test]
    fn test_get_changed_files_sorted() {
        let dir = create_git_repo();
        add_and_commit_file(&dir, "initial.txt", "initial");

        // Create multiple untracked files
        fs::write(dir.path().join("zebra.txt"), "content").unwrap();
        fs::write(dir.path().join("apple.txt"), "content").unwrap();
        fs::write(dir.path().join("banana.txt"), "content").unwrap();

        let result = get_changed_files(dir.path());
        assert!(result.is_ok());
        let files = result.unwrap();

        // Should be sorted by path
        let names: Vec<_> = files.iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }
}
