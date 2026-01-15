//! File tree panel for directory browsing.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Tree display mode
#[derive(Clone, Debug, PartialEq)]
pub enum TreeMode {
    /// Show full directory tree
    FullTree,
    /// Show only git changed files
    GitChangedFiles,
}

impl Default for TreeMode {
    fn default() -> Self {
        TreeMode::FullTree
    }
}

/// Git status for a file
#[derive(Clone, Debug, PartialEq)]
pub struct GitFileStatus {
    pub added_lines: usize,
    pub removed_lines: usize,
    pub is_untracked: bool,
}

/// Type of tree entry
#[derive(Clone, Debug, PartialEq)]
pub enum TreeEntryType {
    Directory { is_expanded: bool },
    File { git_status: Option<GitFileStatus> },
}

/// A single entry in the tree (visible line)
#[derive(Clone, Debug)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub entry_type: TreeEntryType,
}

impl TreeEntry {
    pub fn is_directory(&self) -> bool {
        matches!(self.entry_type, TreeEntryType::Directory { .. })
    }

    pub fn is_expanded(&self) -> bool {
        matches!(self.entry_type, TreeEntryType::Directory { is_expanded: true })
    }

    pub fn is_selectable(&self) -> bool {
        // All entries are selectable except the "(empty)" placeholder
        !self.name.starts_with('(')
    }
}

/// File tree panel state
pub struct FileTreePanel {
    /// Root directory path
    pub root_path: PathBuf,
    /// Flat list of visible entries
    pub entries: Vec<TreeEntry>,
    /// Currently selected index in entries
    pub selected_index: usize,
    /// Scroll offset for rendering
    pub scroll_offset: usize,
    /// Set of expanded directory paths
    pub expanded_dirs: HashSet<PathBuf>,
    /// Current tree mode
    pub mode: TreeMode,
    /// Currently open file (highlighted differently)
    pub current_file: Option<PathBuf>,
    /// Cached git changed files
    git_changed_files: Option<Vec<GitChangedFile>>,
}

/// Git changed file info
#[derive(Clone, Debug)]
pub struct GitChangedFile {
    pub path: PathBuf,
    pub added_lines: usize,
    pub removed_lines: usize,
    pub is_untracked: bool,
}

impl FileTreePanel {
    /// Create a new file tree panel for a directory
    pub fn new(root_path: PathBuf) -> io::Result<Self> {
        let root_path = root_path.canonicalize()?;
        let mut panel = FileTreePanel {
            root_path,
            entries: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            expanded_dirs: HashSet::new(),
            mode: TreeMode::FullTree,
            current_file: None,
            git_changed_files: None,
        };
        panel.rebuild_entries()?;
        Ok(panel)
    }

    /// Rebuild the flat entry list based on current state
    pub fn rebuild_entries(&mut self) -> io::Result<()> {
        self.entries.clear();

        match self.mode {
            TreeMode::FullTree => {
                self.build_tree_entries(&self.root_path.clone(), 0)?;
            }
            TreeMode::GitChangedFiles => {
                self.build_git_changed_entries()?;
            }
        }

        // Ensure selected_index is valid
        if !self.entries.is_empty() {
            if self.selected_index >= self.entries.len() {
                self.selected_index = self.entries.len() - 1;
            }
            // Skip non-selectable entries
            self.ensure_selectable_selection();
        } else {
            self.selected_index = 0;
        }

        Ok(())
    }

    /// Build tree entries recursively
    fn build_tree_entries(&mut self, dir: &Path, depth: usize) -> io::Result<()> {
        let mut entries: Vec<_> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .collect();

        // Sort: directories first, then files, case-insensitive alphabetical
        entries.sort_by(|a, b| {
            let a_is_dir = a.path().is_dir();
            let b_is_dir = b.path().is_dir();
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().to_string_lossy().to_lowercase()
                    .cmp(&b.file_name().to_string_lossy().to_lowercase()),
            }
        });

        if entries.is_empty() {
            // Show (empty) placeholder for empty directories (including root)
            self.entries.push(TreeEntry {
                path: dir.to_path_buf(),
                name: "(empty)".to_string(),
                depth,
                entry_type: TreeEntryType::File { git_status: None },
            });
            return Ok(());
        }

        for entry in entries {
            let path = entry.path();
            // Follow symlinks
            let path = path.canonicalize().unwrap_or(path);
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = path.is_dir();

            if is_dir {
                let is_expanded = self.expanded_dirs.contains(&path);
                self.entries.push(TreeEntry {
                    path: path.clone(),
                    name,
                    depth,
                    entry_type: TreeEntryType::Directory { is_expanded },
                });

                if is_expanded {
                    self.build_tree_entries(&path, depth + 1)?;
                }
            } else {
                let git_status = self.get_git_status_for_file(&path);
                self.entries.push(TreeEntry {
                    path,
                    name,
                    depth,
                    entry_type: TreeEntryType::File { git_status },
                });
            }
        }

        Ok(())
    }

    /// Build entries for git changed files mode
    fn build_git_changed_entries(&mut self) -> io::Result<()> {
        // Refresh git changed files if needed
        if self.git_changed_files.is_none() {
            self.refresh_git_changed_files();
        }

        if let Some(ref files) = self.git_changed_files {
            if files.is_empty() {
                self.entries.push(TreeEntry {
                    path: self.root_path.clone(),
                    name: "(no changes)".to_string(),
                    depth: 0,
                    entry_type: TreeEntryType::File { git_status: None },
                });
            } else {
                for file in files {
                    let name = file.path.strip_prefix(&self.root_path)
                        .unwrap_or(&file.path)
                        .to_string_lossy()
                        .to_string();

                    self.entries.push(TreeEntry {
                        path: file.path.clone(),
                        name,
                        depth: 0,
                        entry_type: TreeEntryType::File {
                            git_status: Some(GitFileStatus {
                                added_lines: file.added_lines,
                                removed_lines: file.removed_lines,
                                is_untracked: file.is_untracked,
                            }),
                        },
                    });
                }
            }
        } else {
            self.entries.push(TreeEntry {
                path: self.root_path.clone(),
                name: "(not a git repo)".to_string(),
                depth: 0,
                entry_type: TreeEntryType::File { git_status: None },
            });
        }

        Ok(())
    }

    /// Get git status for a single file (used in tree mode)
    fn get_git_status_for_file(&self, _path: &Path) -> Option<GitFileStatus> {
        // In tree mode, we don't show git status by default
        // This could be enhanced later if needed
        None
    }

    /// Refresh git changed files cache
    pub fn refresh_git_changed_files(&mut self) {
        self.git_changed_files = crate::git::get_changed_files(&self.root_path).ok();
    }

    /// Ensure the selected index points to a selectable entry
    fn ensure_selectable_selection(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        // First try to find a selectable entry at or after current selection
        for i in self.selected_index..self.entries.len() {
            if self.entries[i].is_selectable() {
                self.selected_index = i;
                return;
            }
        }

        // If not found, try before current selection
        for i in (0..self.selected_index).rev() {
            if self.entries[i].is_selectable() {
                self.selected_index = i;
                return;
            }
        }
    }

    /// Toggle between tree and git changed files mode
    pub fn toggle_mode(&mut self) -> io::Result<()> {
        self.mode = match self.mode {
            TreeMode::FullTree => {
                self.refresh_git_changed_files();
                TreeMode::GitChangedFiles
            }
            TreeMode::GitChangedFiles => TreeMode::FullTree,
        };
        self.rebuild_entries()
    }

    /// Navigate up in the tree
    pub fn navigate_up(&mut self) {
        if self.entries.is_empty() || self.selected_index == 0 {
            return;
        }

        // Find the next selectable entry above
        for i in (0..self.selected_index).rev() {
            if self.entries[i].is_selectable() {
                self.selected_index = i;
                break;
            }
        }
    }

    /// Navigate down in the tree
    pub fn navigate_down(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        // Find the next selectable entry below
        for i in (self.selected_index + 1)..self.entries.len() {
            if self.entries[i].is_selectable() {
                self.selected_index = i;
                break;
            }
        }
    }

    /// Navigate to the first entry
    pub fn navigate_home(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected_index = 0;
        self.ensure_selectable_selection();
        self.scroll_offset = 0;
    }

    /// Navigate to the last entry
    pub fn navigate_end(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected_index = self.entries.len() - 1;
        self.ensure_selectable_selection();
    }

    /// Page up navigation
    pub fn page_up(&mut self, page_size: usize) {
        if self.entries.is_empty() {
            return;
        }

        let new_index = self.selected_index.saturating_sub(page_size);
        self.selected_index = new_index;
        self.ensure_selectable_selection();
    }

    /// Page down navigation
    pub fn page_down(&mut self, page_size: usize) {
        if self.entries.is_empty() {
            return;
        }

        let new_index = (self.selected_index + page_size).min(self.entries.len() - 1);
        self.selected_index = new_index;
        self.ensure_selectable_selection();
    }

    /// Expand the currently selected directory
    pub fn expand_selected(&mut self) -> io::Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }

        let entry = &self.entries[self.selected_index];
        if let TreeEntryType::Directory { is_expanded: false } = entry.entry_type {
            let path = entry.path.clone();
            self.expanded_dirs.insert(path);
            self.rebuild_entries()?;
        }
        Ok(())
    }

    /// Collapse the currently selected directory
    pub fn collapse_selected(&mut self) -> io::Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }

        let entry = &self.entries[self.selected_index];

        // If it's an expanded directory, collapse it
        if let TreeEntryType::Directory { is_expanded: true } = entry.entry_type {
            let path = entry.path.clone();
            self.expanded_dirs.remove(&path);
            self.rebuild_entries()?;
            return Ok(());
        }

        // If it's a file or collapsed directory, go to parent
        if entry.depth > 0 {
            // Find the parent directory entry
            let entry_depth = entry.depth;
            for i in (0..self.selected_index).rev() {
                if self.entries[i].depth < entry_depth {
                    if let TreeEntryType::Directory { .. } = self.entries[i].entry_type {
                        self.selected_index = i;
                        // Collapse the parent
                        let parent_path = self.entries[i].path.clone();
                        self.expanded_dirs.remove(&parent_path);
                        self.rebuild_entries()?;
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Get the currently selected entry
    pub fn get_selected(&self) -> Option<&TreeEntry> {
        self.entries.get(self.selected_index)
    }

    /// Get the path of the currently selected file (not directory)
    pub fn get_selected_file_path(&self) -> Option<&Path> {
        self.get_selected().and_then(|entry| {
            if matches!(entry.entry_type, TreeEntryType::File { .. }) && entry.is_selectable() {
                Some(entry.path.as_path())
            } else {
                None
            }
        })
    }

    /// Set the current file (for highlighting)
    pub fn set_current_file(&mut self, path: Option<PathBuf>) {
        self.current_file = path.map(|p| p.canonicalize().unwrap_or(p));
    }

    /// Check if a path is the current file
    pub fn is_current_file(&self, path: &Path) -> bool {
        self.current_file.as_ref().map_or(false, |current| {
            current == path || current.canonicalize().ok().as_ref() == Some(&path.to_path_buf())
        })
    }

    /// Refresh the tree (e.g., after external file changes)
    pub fn refresh(&mut self) -> io::Result<()> {
        self.git_changed_files = None;
        self.rebuild_entries()
    }

    /// Adjust scroll offset to keep selected item visible
    pub fn adjust_scroll(&mut self, visible_height: usize) {
        if self.entries.is_empty() || visible_height == 0 {
            return;
        }

        // Ensure selected index is visible
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected_index - visible_height + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create some test files and directories
        fs::create_dir(dir.path().join("subdir")).unwrap();
        File::create(dir.path().join("file_a.txt")).unwrap();
        File::create(dir.path().join("file_b.txt")).unwrap();
        File::create(dir.path().join("subdir/nested.txt")).unwrap();
        File::create(dir.path().join(".hidden")).unwrap();

        dir
    }

    #[test]
    fn test_new_panel() {
        let dir = create_test_dir();
        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        assert!(!panel.entries.is_empty());
        assert_eq!(panel.mode, TreeMode::FullTree);
        assert_eq!(panel.selected_index, 0);
    }

    #[test]
    fn test_sort_folders_first() {
        let dir = create_test_dir();
        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // First entry should be a directory (subdir)
        // Note: .hidden is a file, not a directory
        let first_dir_index = panel.entries.iter()
            .position(|e| matches!(e.entry_type, TreeEntryType::Directory { .. }));
        let first_file_index = panel.entries.iter()
            .position(|e| matches!(e.entry_type, TreeEntryType::File { .. }));

        if let (Some(dir_idx), Some(file_idx)) = (first_dir_index, first_file_index) {
            assert!(dir_idx < file_idx, "Directories should come before files");
        }
    }

    #[test]
    fn test_sort_case_insensitive() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("Zebra.txt")).unwrap();
        File::create(dir.path().join("apple.txt")).unwrap();
        File::create(dir.path().join("Banana.txt")).unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        let names: Vec<_> = panel.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["apple.txt", "Banana.txt", "Zebra.txt"]);
    }

    #[test]
    fn test_hidden_files_shown() {
        let dir = create_test_dir();
        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        let has_hidden = panel.entries.iter().any(|e| e.name.starts_with('.'));
        assert!(has_hidden, "Hidden files should be shown");
    }

    #[test]
    fn test_expand_directory() {
        let dir = create_test_dir();
        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find the subdir entry
        let subdir_index = panel.entries.iter()
            .position(|e| e.name == "subdir")
            .unwrap();

        panel.selected_index = subdir_index;
        panel.expand_selected().unwrap();

        // After expanding, nested.txt should be visible
        let has_nested = panel.entries.iter().any(|e| e.name == "nested.txt");
        assert!(has_nested, "Nested file should be visible after expand");
    }

    #[test]
    fn test_collapse_directory() {
        let dir = create_test_dir();
        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find and expand subdir
        let subdir_index = panel.entries.iter()
            .position(|e| e.name == "subdir")
            .unwrap();

        panel.selected_index = subdir_index;
        panel.expand_selected().unwrap();

        // Now collapse
        panel.collapse_selected().unwrap();

        // After collapsing, nested.txt should not be visible
        let has_nested = panel.entries.iter().any(|e| e.name == "nested.txt");
        assert!(!has_nested, "Nested file should not be visible after collapse");
    }

    #[test]
    fn test_navigate_up_down() {
        let dir = create_test_dir();
        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        let initial_index = panel.selected_index;
        panel.navigate_down();
        assert!(panel.selected_index > initial_index || panel.entries.len() <= 1);

        panel.navigate_up();
        assert_eq!(panel.selected_index, initial_index);
    }

    #[test]
    fn test_navigate_bounds() {
        let dir = create_test_dir();
        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Navigate up at beginning should stay at 0
        panel.selected_index = 0;
        panel.navigate_up();
        assert_eq!(panel.selected_index, 0);

        // Navigate down at end should stay at end
        panel.selected_index = panel.entries.len() - 1;
        let end_index = panel.selected_index;
        panel.navigate_down();
        assert_eq!(panel.selected_index, end_index);
    }

    #[test]
    fn test_navigate_home_end() {
        let dir = create_test_dir();
        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        panel.navigate_end();
        let end_pos = panel.selected_index;

        panel.navigate_home();
        assert_eq!(panel.selected_index, 0);

        panel.navigate_end();
        assert_eq!(panel.selected_index, end_pos);
    }

    #[test]
    fn test_page_navigation() {
        let dir = TempDir::new().unwrap();
        // Create many files
        for i in 0..20 {
            File::create(dir.path().join(format!("file_{:02}.txt", i))).unwrap();
        }

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        panel.page_down(10);
        assert!(panel.selected_index >= 10 || panel.entries.len() < 10);

        panel.page_up(5);
        assert!(panel.selected_index >= 5 || panel.selected_index == 0);
    }

    #[test]
    fn test_toggle_mode() {
        let dir = create_test_dir();
        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        assert_eq!(panel.mode, TreeMode::FullTree);

        panel.toggle_mode().unwrap();
        assert_eq!(panel.mode, TreeMode::GitChangedFiles);

        panel.toggle_mode().unwrap();
        assert_eq!(panel.mode, TreeMode::FullTree);
    }

    #[test]
    fn test_set_current_file() {
        let dir = create_test_dir();
        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        let file_path = dir.path().join("file_a.txt");
        panel.set_current_file(Some(file_path.clone()));

        assert!(panel.is_current_file(&file_path));

        panel.set_current_file(None);
        assert!(!panel.is_current_file(&file_path));
    }

    #[test]
    fn test_get_selected_file_path() {
        let dir = create_test_dir();
        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find a file entry
        let file_index = panel.entries.iter()
            .position(|e| matches!(e.entry_type, TreeEntryType::File { .. }) && e.is_selectable())
            .unwrap();

        panel.selected_index = file_index;
        assert!(panel.get_selected_file_path().is_some());

        // Find a directory entry
        if let Some(dir_index) = panel.entries.iter()
            .position(|e| matches!(e.entry_type, TreeEntryType::Directory { .. }))
        {
            panel.selected_index = dir_index;
            assert!(panel.get_selected_file_path().is_none());
        }
    }

    #[test]
    fn test_empty_directory() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("empty_dir")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find and expand empty_dir
        let empty_dir_index = panel.entries.iter()
            .position(|e| e.name == "empty_dir")
            .unwrap();

        panel.selected_index = empty_dir_index;
        panel.expand_selected().unwrap();

        // Should have (empty) placeholder
        let has_empty = panel.entries.iter().any(|e| e.name == "(empty)");
        assert!(has_empty, "Empty directory should show (empty) placeholder");
    }

    #[test]
    fn test_empty_not_selectable() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("empty_dir")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find and expand empty_dir
        let empty_dir_index = panel.entries.iter()
            .position(|e| e.name == "empty_dir")
            .unwrap();

        panel.selected_index = empty_dir_index;
        panel.expand_selected().unwrap();

        // Find the (empty) entry
        let empty_entry = panel.entries.iter().find(|e| e.name == "(empty)").unwrap();
        assert!(!empty_entry.is_selectable());
    }

    #[test]
    fn test_adjust_scroll() {
        let dir = TempDir::new().unwrap();
        for i in 0..30 {
            File::create(dir.path().join(format!("file_{:02}.txt", i))).unwrap();
        }

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Select item beyond visible area
        panel.selected_index = 25;
        panel.adjust_scroll(10);

        // Scroll should adjust to show selected item
        assert!(panel.scroll_offset + 10 > panel.selected_index);
        assert!(panel.scroll_offset <= panel.selected_index);
    }

    #[test]
    fn test_refresh() {
        let dir = create_test_dir();
        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        let initial_count = panel.entries.len();

        // Add a new file
        File::create(dir.path().join("new_file.txt")).unwrap();

        // Refresh
        panel.refresh().unwrap();

        assert!(panel.entries.len() > initial_count);
    }

    #[test]
    fn test_tree_entry_methods() {
        let entry = TreeEntry {
            path: PathBuf::from("/test"),
            name: "test".to_string(),
            depth: 0,
            entry_type: TreeEntryType::Directory { is_expanded: true },
        };

        assert!(entry.is_directory());
        assert!(entry.is_expanded());
        assert!(entry.is_selectable());

        let file_entry = TreeEntry {
            path: PathBuf::from("/test.txt"),
            name: "test.txt".to_string(),
            depth: 0,
            entry_type: TreeEntryType::File { git_status: None },
        };

        assert!(!file_entry.is_directory());
        assert!(!file_entry.is_expanded());
        assert!(file_entry.is_selectable());
    }

    #[test]
    fn test_collapsed_directory_not_expanded() {
        let entry = TreeEntry {
            path: PathBuf::from("/test"),
            name: "test".to_string(),
            depth: 0,
            entry_type: TreeEntryType::Directory { is_expanded: false },
        };

        assert!(entry.is_directory());
        assert!(!entry.is_expanded());
    }

    // ========================================================================
    // Git Mode Tests
    // ========================================================================

    fn create_git_repo_for_tree() -> TempDir {
        use std::process::Command;

        let dir = TempDir::new().unwrap();

        let output = Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to run git init");
        assert!(output.status.success(), "git init failed");

        let output = Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to configure git email");
        assert!(output.status.success());

        let output = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to configure git name");
        assert!(output.status.success());

        let output = Command::new("git")
            .args(["config", "commit.gpgsign", "false"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to disable gpg signing");
        assert!(output.status.success());

        dir
    }

    fn git_add_and_commit(dir: &TempDir, filename: &str, content: &str) {
        use std::process::Command;

        fs::write(dir.path().join(filename), content).unwrap();

        let output = Command::new("git")
            .args(["add", filename])
            .current_dir(dir.path())
            .output()
            .expect("Failed to git add");
        assert!(output.status.success());

        let output = Command::new("git")
            .args(["commit", "-m", "Add file"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to git commit");
        assert!(output.status.success());
    }

    #[test]
    fn test_git_mode_toggle() {
        let dir = create_git_repo_for_tree();
        git_add_and_commit(&dir, "file.txt", "content");

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        assert!(matches!(panel.mode, TreeMode::FullTree));

        panel.toggle_mode().unwrap();
        assert!(matches!(panel.mode, TreeMode::GitChangedFiles));

        panel.toggle_mode().unwrap();
        assert!(matches!(panel.mode, TreeMode::FullTree));
    }

    #[test]
    fn test_git_mode_shows_changed_files() {
        let dir = create_git_repo_for_tree();
        git_add_and_commit(&dir, "file.txt", "initial content");

        // Modify the file
        fs::write(dir.path().join("file.txt"), "modified content").unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.toggle_mode().unwrap();

        // Should have the modified file in the list
        let has_file = panel.entries.iter().any(|e| e.name == "file.txt");
        assert!(has_file, "Git mode should show modified file");
    }

    #[test]
    fn test_git_mode_shows_untracked_files() {
        let dir = create_git_repo_for_tree();
        git_add_and_commit(&dir, "tracked.txt", "content");

        // Create untracked file
        fs::write(dir.path().join("untracked.txt"), "new content").unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.toggle_mode().unwrap();

        let has_untracked = panel.entries.iter().any(|e| e.name == "untracked.txt");
        assert!(has_untracked, "Git mode should show untracked files");
    }

    #[test]
    fn test_git_mode_no_changes_shows_message() {
        let dir = create_git_repo_for_tree();
        git_add_and_commit(&dir, "file.txt", "content");

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.toggle_mode().unwrap();

        // When there are no changes, should show "(no changes)" entry
        let has_no_changes = panel.entries.iter().any(|e| e.name.contains("no changes"));
        assert!(has_no_changes, "Git mode with no changes should show message");
    }

    #[test]
    fn test_git_mode_not_a_repo() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.toggle_mode().unwrap();

        // Should show "(not a git repo)" message
        let has_message = panel.entries.iter().any(|e| e.name.contains("not a git repo"));
        assert!(has_message, "Non-git directory should show message");
    }

    // ========================================================================
    // File Name Handling Tests
    // ========================================================================

    #[test]
    fn test_unicode_filenames() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("日本語.txt")).unwrap();
        File::create(dir.path().join("Привет.txt")).unwrap();
        File::create(dir.path().join("emoji_🎉.txt")).unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        let has_japanese = panel.entries.iter().any(|e| e.name == "日本語.txt");
        let has_russian = panel.entries.iter().any(|e| e.name == "Привет.txt");
        let has_emoji = panel.entries.iter().any(|e| e.name == "emoji_🎉.txt");

        assert!(has_japanese, "Should handle Japanese filenames");
        assert!(has_russian, "Should handle Russian filenames");
        assert!(has_emoji, "Should handle emoji in filenames");
    }

    #[test]
    fn test_filenames_with_spaces() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file with spaces.txt")).unwrap();
        File::create(dir.path().join("another file.txt")).unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        let has_spaces = panel.entries.iter().any(|e| e.name == "file with spaces.txt");
        assert!(has_spaces, "Should handle filenames with spaces");
    }

    #[test]
    fn test_filenames_with_special_chars() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file-with-dashes.txt")).unwrap();
        File::create(dir.path().join("file_with_underscores.txt")).unwrap();
        File::create(dir.path().join("file.multiple.dots.txt")).unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        assert!(panel.entries.iter().any(|e| e.name == "file-with-dashes.txt"));
        assert!(panel.entries.iter().any(|e| e.name == "file_with_underscores.txt"));
        assert!(panel.entries.iter().any(|e| e.name == "file.multiple.dots.txt"));
    }

    #[test]
    fn test_hidden_files_sorted_correctly() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join(".gitignore")).unwrap();
        File::create(dir.path().join(".hidden")).unwrap();
        File::create(dir.path().join("afile.txt")).unwrap();
        File::create(dir.path().join("zfile.txt")).unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Get positions
        let gitignore_pos = panel.entries.iter().position(|e| e.name == ".gitignore");
        let hidden_pos = panel.entries.iter().position(|e| e.name == ".hidden");
        let afile_pos = panel.entries.iter().position(|e| e.name == "afile.txt");

        assert!(gitignore_pos.is_some());
        assert!(hidden_pos.is_some());
        assert!(afile_pos.is_some());

        // Hidden files should be sorted with other files (. sorts before letters)
        assert!(gitignore_pos.unwrap() < afile_pos.unwrap());
    }

    // ========================================================================
    // Deep Nesting Tests
    // ========================================================================

    #[test]
    fn test_deep_nesting_five_levels() {
        let dir = TempDir::new().unwrap();

        // Create 5 levels deep
        let mut path = dir.path().to_path_buf();
        for i in 1..=5 {
            path = path.join(format!("level{}", i));
            fs::create_dir(&path).unwrap();
        }
        File::create(path.join("deep_file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Expand all levels
        for _ in 0..5 {
            let dir_idx = panel.entries.iter().position(|e| e.is_directory() && !e.is_expanded());
            if let Some(idx) = dir_idx {
                panel.selected_index = idx;
                panel.expand_selected().unwrap();
            }
        }

        // Should find the deep file
        let has_deep_file = panel.entries.iter().any(|e| e.name == "deep_file.txt");
        assert!(has_deep_file, "Should be able to navigate to deeply nested file");

        // Check depth is correct
        let deep_file = panel.entries.iter().find(|e| e.name == "deep_file.txt").unwrap();
        assert_eq!(deep_file.depth, 5, "Deep file should have depth 5");
    }

    #[test]
    fn test_collapse_parent_hides_all_children() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("parent")).unwrap();
        fs::create_dir(dir.path().join("parent/child")).unwrap();
        File::create(dir.path().join("parent/child/grandchild.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Expand parent
        let parent_idx = panel.entries.iter().position(|e| e.name == "parent").unwrap();
        panel.selected_index = parent_idx;
        panel.expand_selected().unwrap();

        // Expand child
        let child_idx = panel.entries.iter().position(|e| e.name == "child").unwrap();
        panel.selected_index = child_idx;
        panel.expand_selected().unwrap();

        // Verify grandchild is visible
        assert!(panel.entries.iter().any(|e| e.name == "grandchild.txt"));

        // Collapse parent
        let parent_idx = panel.entries.iter().position(|e| e.name == "parent").unwrap();
        panel.selected_index = parent_idx;
        panel.collapse_selected().unwrap();

        // Child and grandchild should both be hidden
        assert!(!panel.entries.iter().any(|e| e.name == "child"));
        assert!(!panel.entries.iter().any(|e| e.name == "grandchild.txt"));
    }

    #[test]
    fn test_depth_indicator_correct() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("level1")).unwrap();
        fs::create_dir(dir.path().join("level1/level2")).unwrap();
        File::create(dir.path().join("level1/level2/file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Expand all
        for _ in 0..2 {
            let dir_idx = panel.entries.iter().position(|e| e.is_directory() && !e.is_expanded());
            if let Some(idx) = dir_idx {
                panel.selected_index = idx;
                panel.expand_selected().unwrap();
            }
        }

        let level1 = panel.entries.iter().find(|e| e.name == "level1").unwrap();
        let level2 = panel.entries.iter().find(|e| e.name == "level2").unwrap();
        let file = panel.entries.iter().find(|e| e.name == "file.txt").unwrap();

        assert_eq!(level1.depth, 0);
        assert_eq!(level2.depth, 1);
        assert_eq!(file.depth, 2);
    }

    // ========================================================================
    // Scroll Behavior Edge Cases
    // ========================================================================

    #[test]
    fn test_scroll_on_expand_many_items() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("folder")).unwrap();
        for i in 0..20 {
            File::create(dir.path().join(format!("folder/file_{:02}.txt", i))).unwrap();
        }
        File::create(dir.path().join("after.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Expand folder
        let folder_idx = panel.entries.iter().position(|e| e.name == "folder").unwrap();
        panel.selected_index = folder_idx;
        panel.expand_selected().unwrap();

        // After expand, selected should still be visible
        panel.adjust_scroll(10);
        assert!(panel.scroll_offset <= panel.selected_index);
    }

    #[test]
    fn test_scroll_on_collapse_moves_selection() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("folder")).unwrap();
        for i in 0..5 {
            File::create(dir.path().join(format!("folder/file_{}.txt", i))).unwrap();
        }

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Expand folder
        let folder_idx = panel.entries.iter().position(|e| e.name == "folder").unwrap();
        panel.selected_index = folder_idx;
        panel.expand_selected().unwrap();

        // Select a file inside
        let file_idx = panel.entries.iter().position(|e| e.name == "file_2.txt").unwrap();
        panel.selected_index = file_idx;

        // Collapse folder - selection should move to folder
        let folder_idx = panel.entries.iter().position(|e| e.name == "folder").unwrap();
        panel.selected_index = folder_idx;
        panel.collapse_selected().unwrap();

        // Selection should be valid
        assert!(panel.selected_index < panel.entries.len());
    }

    #[test]
    fn test_scroll_at_boundary() {
        let dir = TempDir::new().unwrap();
        for i in 0..15 {
            File::create(dir.path().join(format!("file_{:02}.txt", i))).unwrap();
        }

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        let visible_height = 10;

        // Select item at exactly the boundary
        panel.selected_index = visible_height - 1;
        panel.adjust_scroll(visible_height);

        assert_eq!(panel.scroll_offset, 0, "Should not scroll when at boundary");

        // Select one past boundary
        panel.selected_index = visible_height;
        panel.adjust_scroll(visible_height);

        assert!(panel.scroll_offset > 0, "Should scroll when past boundary");
    }

    #[test]
    fn test_scroll_single_item() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("only_file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        panel.adjust_scroll(10);
        assert_eq!(panel.scroll_offset, 0);
        assert_eq!(panel.selected_index, 0);
    }

    // ========================================================================
    // Selection Preservation Tests
    // ========================================================================

    #[test]
    fn test_selection_preserved_after_refresh_file_exists() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file_a.txt")).unwrap();
        File::create(dir.path().join("file_b.txt")).unwrap();
        File::create(dir.path().join("file_c.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Select file_b
        let idx = panel.entries.iter().position(|e| e.name == "file_b.txt").unwrap();
        panel.selected_index = idx;

        // Refresh
        panel.refresh().unwrap();

        // Selection should still be valid
        assert!(panel.selected_index < panel.entries.len());
    }

    #[test]
    fn test_selection_adjusted_after_file_deleted() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file_a.txt")).unwrap();
        File::create(dir.path().join("file_b.txt")).unwrap();
        File::create(dir.path().join("file_c.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Select last file
        panel.selected_index = panel.entries.len() - 1;

        // Delete the last file
        fs::remove_file(dir.path().join("file_c.txt")).unwrap();

        // Refresh
        panel.refresh().unwrap();

        // Selection should be adjusted to be within bounds
        assert!(panel.selected_index < panel.entries.len());
    }

    #[test]
    fn test_selection_after_mode_toggle() {
        let dir = create_git_repo_for_tree();
        git_add_and_commit(&dir, "file.txt", "content");
        fs::write(dir.path().join("file.txt"), "modified").unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Select something in full tree mode
        panel.selected_index = 0;

        // Toggle to git mode
        panel.toggle_mode().unwrap();

        // Selection should be reset or valid
        assert!(panel.selected_index < panel.entries.len());

        // Toggle back
        panel.toggle_mode().unwrap();
        assert!(panel.selected_index < panel.entries.len());
    }

    #[test]
    fn test_selection_after_expand_collapse() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("folder")).unwrap();
        File::create(dir.path().join("folder/file.txt")).unwrap();
        File::create(dir.path().join("after.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Get position of "after.txt"
        let after_idx = panel.entries.iter().position(|e| e.name == "after.txt").unwrap();
        panel.selected_index = after_idx;

        // Expand folder (items shift)
        let folder_idx = panel.entries.iter().position(|e| e.name == "folder").unwrap();
        panel.selected_index = folder_idx;
        panel.expand_selected().unwrap();

        // Selection should still be valid
        assert!(panel.selected_index < panel.entries.len());
    }

    // ========================================================================
    // Error Scenarios
    // ========================================================================

    #[test]
    fn test_symlink_to_file() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let dir = TempDir::new().unwrap();
            File::create(dir.path().join("target.txt")).unwrap();
            symlink(
                dir.path().join("target.txt"),
                dir.path().join("link.txt"),
            ).unwrap();

            let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

            // Both should be visible
            assert!(panel.entries.iter().any(|e| e.name == "target.txt"));
            assert!(panel.entries.iter().any(|e| e.name == "link.txt"));
        }
    }

    #[test]
    fn test_symlink_to_directory() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let dir = TempDir::new().unwrap();
            fs::create_dir(dir.path().join("target_dir")).unwrap();
            File::create(dir.path().join("target_dir/file.txt")).unwrap();
            symlink(
                dir.path().join("target_dir"),
                dir.path().join("link_dir"),
            ).unwrap();

            let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

            // Link should be expandable as directory
            let link_idx = panel.entries.iter().position(|e| e.name == "link_dir");
            if let Some(idx) = link_idx {
                panel.selected_index = idx;
                // Should be able to expand symlinked directory
                let result = panel.expand_selected();
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn test_expand_on_file_is_noop() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        let file_idx = panel.entries.iter().position(|e| e.name == "file.txt").unwrap();
        panel.selected_index = file_idx;

        let entries_before = panel.entries.len();
        let result = panel.expand_selected();

        // Should succeed but not change anything
        assert!(result.is_ok());
        assert_eq!(panel.entries.len(), entries_before);
    }

    #[test]
    fn test_collapse_on_file_is_noop() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        let file_idx = panel.entries.iter().position(|e| e.name == "file.txt").unwrap();
        panel.selected_index = file_idx;

        let entries_before = panel.entries.len();
        let result = panel.collapse_selected();

        assert!(result.is_ok());
        assert_eq!(panel.entries.len(), entries_before);
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_empty_root_directory() {
        let dir = TempDir::new().unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Root directory shows "(empty)" placeholder
        assert_eq!(panel.entries.len(), 1, "Empty root should have (empty) placeholder");
        assert_eq!(panel.entries[0].name, "(empty)");
        assert!(!panel.entries[0].is_selectable(), "(empty) placeholder should not be selectable");
    }

    #[test]
    fn test_single_file_in_root() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("only_file.txt")).unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        assert_eq!(panel.entries.len(), 1);
        assert_eq!(panel.entries[0].name, "only_file.txt");
    }

    #[test]
    fn test_directory_with_only_subdirectories() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("dir_a")).unwrap();
        fs::create_dir(dir.path().join("dir_b")).unwrap();
        fs::create_dir(dir.path().join("dir_c")).unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // All entries should be directories
        for entry in &panel.entries {
            assert!(entry.is_directory(), "{} should be a directory", entry.name);
        }
        assert_eq!(panel.entries.len(), 3);
    }

    #[test]
    fn test_directory_with_only_files() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file_a.txt")).unwrap();
        File::create(dir.path().join("file_b.txt")).unwrap();
        File::create(dir.path().join("file_c.txt")).unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // All entries should be files
        for entry in &panel.entries {
            assert!(!entry.is_directory(), "{} should be a file", entry.name);
        }
        assert_eq!(panel.entries.len(), 3);
    }

    #[test]
    fn test_mixed_hidden_and_visible_sorting() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join(".hidden_a")).unwrap();
        File::create(dir.path().join("visible_a")).unwrap();
        File::create(dir.path().join(".hidden_b")).unwrap();
        File::create(dir.path().join("visible_b")).unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        let names: Vec<&str> = panel.entries.iter().map(|e| e.name.as_str()).collect();

        // Should be sorted case-insensitively (. comes before letters)
        let mut sorted_names = names.clone();
        sorted_names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        assert_eq!(names, sorted_names);
    }

    // ========================================================================
    // TreeEntry State Tests
    // ========================================================================

    #[test]
    fn test_entry_type_file_detection() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("test.txt")).unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        let entry = panel.entries.iter().find(|e| e.name == "test.txt").unwrap();

        assert!(matches!(entry.entry_type, TreeEntryType::File { .. }));
        assert!(!entry.is_directory());
    }

    #[test]
    fn test_entry_type_directory_detection() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        let entry = panel.entries.iter().find(|e| e.name == "subdir").unwrap();

        assert!(matches!(entry.entry_type, TreeEntryType::Directory { .. }));
        assert!(entry.is_directory());
    }

    #[test]
    fn test_expanded_state_persistence() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("folder")).unwrap();
        File::create(dir.path().join("folder/file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Initially collapsed
        let folder = panel.entries.iter().find(|e| e.name == "folder").unwrap();
        assert!(!folder.is_expanded());

        // Expand
        let idx = panel.entries.iter().position(|e| e.name == "folder").unwrap();
        panel.selected_index = idx;
        panel.expand_selected().unwrap();

        // Now expanded
        let folder = panel.entries.iter().find(|e| e.name == "folder").unwrap();
        assert!(folder.is_expanded());
    }

    #[test]
    fn test_empty_entry_not_selectable() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("empty_folder")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Expand empty folder
        let idx = panel.entries.iter().position(|e| e.name == "empty_folder").unwrap();
        panel.selected_index = idx;
        panel.expand_selected().unwrap();

        // Find the "(empty)" entry
        let empty_entry = panel.entries.iter().find(|e| e.name == "(empty)");
        assert!(empty_entry.is_some());
        assert!(!empty_entry.unwrap().is_selectable());
    }

    #[test]
    fn test_git_status_in_entry() {
        let dir = create_git_repo_for_tree();
        git_add_and_commit(&dir, "file.txt", "line1\nline2\nline3");

        // Modify file (add and remove lines)
        fs::write(dir.path().join("file.txt"), "line1\nmodified\nline3\nnew line").unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.toggle_mode().unwrap();

        // Find the file entry and check git status
        let file_entry = panel.entries.iter().find(|e| e.name == "file.txt");
        assert!(file_entry.is_some());

        if let Some(entry) = file_entry {
            if let TreeEntryType::File { git_status } = &entry.entry_type {
                assert!(git_status.is_some(), "Should have git status");
                let status = git_status.as_ref().unwrap();
                // Should have some added/removed lines
                assert!(status.added_lines > 0 || status.removed_lines > 0);
            }
        }
    }

    #[test]
    fn test_navigate_skips_non_selectable() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("empty_folder")).unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Expand empty folder
        let idx = panel.entries.iter().position(|e| e.name == "empty_folder").unwrap();
        panel.selected_index = idx;
        panel.expand_selected().unwrap();

        // Navigate down past empty entry
        panel.selected_index = idx;
        panel.navigate_down();

        // Should skip the "(empty)" entry and land on file.txt or next selectable
        let selected = &panel.entries[panel.selected_index];
        assert!(selected.is_selectable(), "Navigation should skip non-selectable entries");
    }

    #[test]
    fn test_current_file_tracking() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file_a.txt")).unwrap();
        File::create(dir.path().join("file_b.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Initially no current file
        assert!(panel.current_file.is_none());

        // Set current file
        panel.set_current_file(Some(dir.path().join("file_a.txt")));
        assert!(panel.current_file.is_some());

        // Check is_current_file
        let file_a_path = dir.path().join("file_a.txt");
        let file_b_path = dir.path().join("file_b.txt");
        assert!(panel.is_current_file(&file_a_path));
        assert!(!panel.is_current_file(&file_b_path));

        // Clear current file
        panel.set_current_file(None);
        assert!(panel.current_file.is_none());
    }
}
