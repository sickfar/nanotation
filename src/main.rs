mod theme;
mod models;
mod text;
mod file;
mod ui;
mod event_handler;
mod editor;

use editor::Editor;
use std::io;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 && (args[1] == "-h" || args[1] == "--help") {
        println!("nanot - nano for annotations and code review");
        println!("\nUsage: nanot [file]");
        println!("\nKeyboard shortcuts:");
        println!("  ^X        Exit");
        println!("  ^O        Save file");
        println!("  ^W        Search");
        println!("  ^T        Toggle theme");
        println!("  ^D        Delete annotation");
        println!("  Enter     Add/edit annotation");
        println!("  ↑↓        Navigate lines");
        println!("  PgUp/PgDn Page navigation");
        return Ok(());
    }

    let file_path = args.get(1).cloned();
    let mut editor = Editor::new(file_path)?;
    editor.run()?;

    Ok(())
}