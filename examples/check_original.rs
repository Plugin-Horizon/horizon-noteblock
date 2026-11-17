//! `cargo run --example check_original -- <original file>...`
//!
//! For each file: an `.nbs` song prints its format version, length, tempo, layer counts, note count and custom
//! instrument count. Any other file (`config.yml`, `playlists.yml`, `globalRadio.yml`, `positionRadio.yml`,
//! `songs.yml`, `holograms.yml`, `messages_<language>.yml`) is read with the port's own reader and prints the key
//! paths it reads and the key paths it does not read. Values, titles and names of songs are never printed.

use mod_noteblock::horizon_noteblock::config::report_for;
use mod_noteblock::horizon_noteblock::nbs;
use std::path::Path;

fn main() {
    for arg in std::env::args().skip(1) {
        let path = Path::new(&arg);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        println!("== {name}");
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(_) => {
                println!("  cannot read the file");
                continue;
            }
        };
        if path.extension().is_some_and(|e| e == "nbs") {
            match nbs::parse(&bytes) {
                Ok(song) => {
                    let s = nbs::summarize(&song);
                    println!("  format {}", s.version);
                    println!("  length {} ticks", s.length);
                    println!("  tempo {:.2} ticks per second", s.tempo);
                    println!(
                        "  layers {} in header, {} with notes",
                        s.header_layers, s.layers_with_notes
                    );
                    println!("  notes {}", s.notes);
                    println!("  custom instruments {}", s.custom);
                }
                Err(e) => println!("  parse error: {e}"),
            }
            continue;
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let report = report_for(&name, &text);
        if let Some(error) = report.parse_error {
            println!("  parse error: {error}");
            continue;
        }
        println!("  read {} key paths:", report.read.len());
        for key in &report.read {
            println!("    {key}");
        }
        println!("  not read {} key paths:", report.unread.len());
        for key in &report.unread {
            println!("    {key}");
        }
    }
}
