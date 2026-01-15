mod diff;
mod editor;
mod event_handler;
mod file;
mod file_tree;
mod git;
mod highlighting;
mod models;
mod navigation;
mod text;
mod theme;
mod ui;
mod ui_diff;
mod ui_tree;

use editor::Editor;
use std::io;
use std::path::Path;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && (args[1] == "-h" || args[1] == "--help") {
        println!("nanot - nano for annotations and code review");
        println!("\nUsage: nanot [file|directory]");
        println!("\nKeyboard shortcuts:");
        println!("  ^X        Exit");
        println!("  ^O        Save file");
        println!("  ^W        Search");
        println!("  ^T        Toggle theme");
        println!("  F1        Help overlay");
        println!("  ^G        Toggle tree/git changed files (directory mode)");
        println!("  ^D        Toggle diff view");
        println!("  Del/Bksp  Delete annotation");
        println!("  ^N        Next annotation");
        println!("  ^P        Prev annotation");
        println!("  Enter     Add/edit annotation");
        println!("  Tab       Switch focus tree/editor (directory mode)");
        println!("  ↑↓        Navigate lines");
        println!("  ←→        Collapse/Expand folder (tree mode)");
        println!("  PgUp/PgDn Page navigation");
        return Ok(());
    }

    let path = if args.len() > 1 {
        args[1].clone()
    } else {
        println!("Error: No file or directory specified.");
        println!("Usage: nanot <file|directory>");
        std::process::exit(1);
    };

    let path_ref = Path::new(&path);

    if !path_ref.exists() {
        println!("Error: '{}' does not exist.", path);
        std::process::exit(1);
    }

    let mut editor = if path_ref.is_dir() {
        Editor::new_with_directory(path)?
    } else {
        Editor::new(path)?
    };

    editor.run()?;

    Ok(())
}