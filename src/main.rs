mod theme;
mod models;
mod text;
mod file;
mod ui;
mod event_handler;
mod editor;
mod highlighting;

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
        println!("  ^G        Toggle Help Overlay");
        println!("  ^D        Delete annotation");
        println!("  ^N        Next annotation");
        println!("  ^P        Prev annotation");
        println!("  Enter     Add/edit annotation");
        println!("  ↑↓        Navigate lines");
        println!("  PgUp/PgDn (Alt+↑/↓) Page navigation");
        return Ok(());
    }

    let file_path = if args.len() > 1 {
        args[1].clone()
    } else {
        println!("Error: No file specified.");
        println!("Usage: nanot <file>");
        std::process::exit(1);
    };

    if !std::path::Path::new(&file_path).exists() {
        println!("Error: File '{}' does not exist.", file_path);
        std::process::exit(1);
    }

    let mut editor = Editor::new(file_path)?;
    editor.run()?;

    Ok(())
}