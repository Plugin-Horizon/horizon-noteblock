//! The song files of `songs/` (read on demand) with their titles and authors: `songs.yml` overrides first,
//! then the song's own title, then the file name.

use crate::horizon_noteblock::config::SongMeta;
use crate::horizon_noteblock::nbs::{self, Song};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A loaded song and what to show for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub song: Song,
    pub title: String,
    pub author: String,
}

#[derive(Debug)]
pub struct Library {
    dir: PathBuf,
    meta: BTreeMap<String, SongMeta>,
    songs: BTreeMap<String, Entry>,
}

/// A song name as a file name: no path parts.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\'])
        && !name.contains("..")
}

/// Titles collapse their whitespace (a block scalar ends with a line break, and a carriage return is a space).
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn title_of(meta: &SongMeta, song: &Song, name: &str) -> String {
    let override_title = normalize(&meta.title);
    if !override_title.is_empty() {
        return override_title;
    }
    let own = normalize(&song.title);
    if own.is_empty() {
        name.to_string()
    } else {
        own
    }
}

fn author_of(meta: &SongMeta, song: &Song) -> String {
    let override_author = normalize(&meta.author);
    if !override_author.is_empty() {
        return override_author;
    }
    normalize(&song.author)
}

impl Library {
    pub fn new(dir: PathBuf, meta: BTreeMap<String, SongMeta>) -> Library {
        Library {
            dir,
            meta,
            songs: BTreeMap::new(),
        }
    }

    /// Loads `songs/<name>.nbs` unless it is loaded already. False (with a line in `log`) when it is missing or
    /// does not parse.
    pub fn ensure(&mut self, name: &str, log: &mut Vec<String>) -> bool {
        if self.songs.contains_key(name) {
            return true;
        }
        if !valid_name(name) {
            log.push(format!("song name '{name}' is not a file name"));
            return false;
        }
        let bytes = match std::fs::read(self.dir.join(format!("{name}.nbs"))) {
            Ok(bytes) => bytes,
            Err(_) => {
                log.push(format!("song file '{name}.nbs' is missing"));
                return false;
            }
        };
        match nbs::parse(&bytes) {
            Ok(song) => {
                self.insert(name, song);
                true
            }
            Err(e) => {
                log.push(format!("song '{name}' is corrupted: {e}"));
                false
            }
        }
    }

    /// Stores a parsed song under a name (titles and authors from `songs.yml` apply).
    pub fn insert(&mut self, name: &str, song: Song) {
        let meta = self.meta.get(name).cloned().unwrap_or_default();
        let title = title_of(&meta, &song, name);
        let author = author_of(&meta, &song);
        self.songs.insert(
            name.to_string(),
            Entry {
                song,
                title,
                author,
            },
        );
    }

    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.songs.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.songs.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::horizon_noteblock::nbs::testutil::{FileSpec, note};

    fn song_with_title(title: &str) -> Song {
        let mut spec = FileSpec::new(0, 4, 1, 1000, vec![note(0, 0, 0, 45)]);
        spec.title = title.to_string();
        nbs::parse(&spec.bytes()).expect("synthetic song parses")
    }

    #[test]
    fn override_title_wins_then_the_song_title_then_the_name() {
        let mut meta = BTreeMap::new();
        meta.insert(
            "carol".to_string(),
            SongMeta {
                author: String::new(),
                title: "\nCarol of the Bells".to_string(),
            },
        );
        meta.insert(
            "frosty".to_string(),
            SongMeta {
                author: "Someone".into(),
                title: String::new(),
            },
        );
        let mut lib = Library::new(PathBuf::from("unused"), meta);
        lib.insert("carol", song_with_title("File title"));
        lib.insert("frosty", song_with_title("Frosty file"));
        lib.insert("plain", song_with_title(""));
        assert_eq!(lib.get("carol").unwrap().title, "Carol of the Bells");
        assert_eq!(lib.get("frosty").unwrap().title, "Frosty file");
        assert_eq!(lib.get("frosty").unwrap().author, "Someone");
        assert_eq!(lib.get("plain").unwrap().title, "plain");
    }

    #[test]
    fn missing_and_bad_names_are_logged_not_loaded() {
        let mut lib = Library::new(
            std::env::temp_dir().join("horizon-noteblock-no-such-songs"),
            BTreeMap::new(),
        );
        let mut log = Vec::new();
        assert!(!lib.ensure("missing_song", &mut log));
        assert!(!lib.ensure("../escape", &mut log));
        assert_eq!(log.len(), 2);
        assert!(!lib.contains("missing_song"));
    }

    #[test]
    fn valid_name_rejects_paths() {
        assert!(valid_name("Canon_in_D"));
        assert!(valid_name("Nutcracker___Russian_Dance_(Trepak)"));
        assert!(!valid_name("a/b"));
        assert!(!valid_name(".."));
        assert!(!valid_name(""));
    }
}
