//! Git integration module for reading HEAD content and checking file status.

use git2::Repository;
use std::path::Path;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn create_git_repo() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to init git repo");

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to configure git email");

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to configure git name");

        dir
    }

    fn add_and_commit_file(dir: &TempDir, filename: &str, content: &str) {
        let file_path = dir.path().join(filename);
        fs::write(&file_path, content).unwrap();

        Command::new("git")
            .args(["add", filename])
            .current_dir(dir.path())
            .output()
            .expect("Failed to add file");

        Command::new("git")
            .args(["commit", "-m", "Add file"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to commit");
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
}
