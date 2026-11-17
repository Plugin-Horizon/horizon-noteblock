//! The original plugin files of `plugins/NoteBlockMusicPlayer/`, read with the vendored YAML reader.
//!
//! Every reader records the key paths it looks at in a [`Tracker`]; [`report_for`] compares them with the
//! leaves of a file, so a file's keys that the port does not read are listed (key paths only, never values).
//! Nothing here writes a file except what the engine saves itself (`playlists.yml`, as NBMP does).

use crate::horizon_noteblock::messages::Messages;
use crate::horizon_noteblock::yaml::{self, Document, Node};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Key paths the readers looked at: `visited` for single keys, `whole` for values read as a whole (lists, texts).
#[derive(Debug, Default)]
pub struct Tracker {
    visited: BTreeSet<String>,
    whole: BTreeSet<String>,
}

fn join(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_string()
    } else {
        format!("{parent}.{key}")
    }
}

impl Tracker {
    /// The child `key` of `node` (which sits at `parent`), recorded when present.
    pub fn child<'a>(&mut self, node: &'a Node, parent: &str, key: &str) -> Option<&'a Node> {
        let found = node.get(key)?;
        self.visited.insert(join(parent, key));
        Some(found)
    }

    pub fn text(&mut self, node: &Node, parent: &str, key: &str) -> Option<String> {
        self.child(node, parent, key)
            .and_then(|n| n.as_str().map(str::to_string))
    }

    pub fn flag(&mut self, node: &Node, parent: &str, key: &str) -> Option<bool> {
        self.child(node, parent, key).and_then(|n| n.as_bool())
    }

    pub fn number(&mut self, node: &Node, parent: &str, key: &str) -> Option<f64> {
        self.child(node, parent, key).and_then(|n| n.as_f64())
    }

    /// The entries of a map, each key recorded (their values are read by the caller).
    pub fn entries<'a>(&mut self, node: &'a Node, path: &str) -> Vec<(String, &'a Node)> {
        if !path.is_empty() {
            self.visited.insert(path.to_string());
        }
        node.entries()
            .iter()
            .map(|e| {
                self.visited.insert(join(path, &e.key));
                (e.key.clone(), &e.value)
            })
            .collect()
    }

    /// Marks a whole value as read (every leaf below it).
    pub fn whole(&mut self, path: &str) {
        self.whole.insert(path.to_string());
    }

    /// Whether a leaf path was read (itself, or as part of a whole value).
    pub fn covers(&self, leaf: &str) -> bool {
        self.visited.contains(leaf)
            || self
                .whole
                .iter()
                .any(|w| leaf == w.as_str() || leaf.starts_with(&format!("{w}.")))
    }

    /// Every path read, sorted.
    pub fn read_paths(&self) -> Vec<String> {
        let mut all: BTreeSet<String> = self.visited.clone();
        all.extend(self.whole.iter().cloned());
        all.into_iter().collect()
    }
}

/// The leaf key paths of a tree (a map with entries is not a leaf; lists and empty maps are).
pub fn leaves(node: &Node, prefix: &str, out: &mut Vec<String>) {
    if node.is_map() && !node.entries().is_empty() {
        for e in node.entries() {
            leaves(&e.value, &join(prefix, &e.key), out);
        }
    } else if !prefix.is_empty() {
        out.push(prefix.to_string());
    }
}

/// `config.yml`: the language, the default player volume, the saved-volume and octave flags and the names of
/// the other files (`config_*`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigFile {
    pub language: String,
    pub default_player_volume: u8,
    pub save_player_volumes: bool,
    pub use_10_octave_range: bool,
    pub playlists_file: String,
    pub world_radio_file: String,
    pub server_radio_file: String,
    pub position_radio_file: String,
    pub holograms_file: String,
}

impl Default for ConfigFile {
    fn default() -> Self {
        ConfigFile {
            language: "en".into(),
            default_player_volume: 100,
            save_player_volumes: false,
            use_10_octave_range: false,
            playlists_file: "playlists.yml".into(),
            world_radio_file: "globalRadio.yml".into(),
            server_radio_file: "globalRadio.yml".into(),
            position_radio_file: "positionRadio.yml".into(),
            holograms_file: "holograms.yml".into(),
        }
    }
}

pub fn read_config(root: &Node, t: &mut Tracker) -> ConfigFile {
    let mut c = ConfigFile::default();
    if let Some(v) = t.text(root, "", "language") {
        c.language = v;
    }
    if let Some(v) = t.number(root, "", "defaultPlayerVolume") {
        c.default_player_volume = v.clamp(0.0, 100.0) as u8;
    }
    if let Some(v) = t.flag(root, "", "savePlayerVolumes") {
        c.save_player_volumes = v;
    }
    if let Some(v) = t.flag(root, "", "use10OctaveRange") {
        c.use_10_octave_range = v;
    }
    if let Some(v) = t.text(root, "", "config_Playlists") {
        c.playlists_file = v;
    }
    if let Some(v) = t.text(root, "", "config_WorldRadio") {
        c.world_radio_file = v;
    }
    if let Some(v) = t.text(root, "", "config_ServerRadio") {
        c.server_radio_file = v;
    }
    if let Some(v) = t.text(root, "", "config_PositionRadio") {
        c.position_radio_file = v;
    }
    if let Some(v) = t.text(root, "", "config_Holograms") {
        c.holograms_file = v;
    }
    c
}

/// One radio as the file writes it. World and server radios use `enable`, `world`, `repeat`, `random`,
/// `actionbar`, `playlist`, `delay` (seconds) and `volume`; position radios add `x`, `y`, `z`, `radius` and the
/// hologram names (inert here).
#[derive(Clone, Debug, PartialEq)]
pub struct RadioDef {
    pub name: String,
    pub enable: bool,
    pub world: String,
    pub center: [f64; 3],
    pub radius: f64,
    pub playlist: String,
    pub repeat: bool,
    pub random: bool,
    pub actionbar: bool,
    pub delay: f64,
    pub volume: u8,
    pub hologram: String,
    pub hologram_op: String,
}

fn radio_def(t: &mut Tracker, node: &Node, path: &str, name: &str) -> RadioDef {
    RadioDef {
        name: name.to_string(),
        enable: t.flag(node, path, "enable").unwrap_or(false),
        world: t.text(node, path, "world").unwrap_or_default(),
        center: [
            t.number(node, path, "x").unwrap_or(0.0),
            t.number(node, path, "y").unwrap_or(0.0),
            t.number(node, path, "z").unwrap_or(0.0),
        ],
        radius: t.number(node, path, "radius").unwrap_or(0.0),
        playlist: t.text(node, path, "playlist").unwrap_or_default(),
        repeat: t.flag(node, path, "repeat").unwrap_or(false),
        random: t.flag(node, path, "random").unwrap_or(false),
        actionbar: t.flag(node, path, "actionbar").unwrap_or(false),
        delay: t.number(node, path, "delay").unwrap_or(0.0).max(0.0),
        volume: t
            .number(node, path, "volume")
            .unwrap_or(100.0)
            .clamp(0.0, 100.0) as u8,
        hologram: t.text(node, path, "hologram").unwrap_or_default(),
        hologram_op: t.text(node, path, "hologramOP").unwrap_or_default(),
    }
}

fn join_name(section: &str, name: &str) -> String {
    join(section, name)
}

/// The radios of one section (`worldradios` or `serverradios`, or `positionradios`), in file order.
pub fn read_radio_section(root: &Node, t: &mut Tracker, section: &str) -> Vec<RadioDef> {
    let Some(node) = t.child(root, "", section) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, value) in t.entries(node, section) {
        let path = join_name(section, &name);
        out.push(radio_def(t, value, &path, &name));
    }
    out
}

/// `positionRadio.yml`: the stray top-level `radios` key is read and ignored (as NBMP does); `positionradios` is read.
pub fn read_position(root: &Node, t: &mut Tracker) -> Vec<RadioDef> {
    let _ = t.child(root, "", "radios");
    read_radio_section(root, t, "positionradios")
}

/// `playlists.yml`: `playlist:` maps a name to its song names (file names without `.nbs`), in order.
pub fn read_playlists(root: &Node, t: &mut Tracker) -> Vec<(String, Vec<String>)> {
    let Some(node) = t.child(root, "", "playlist") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, list) in t.entries(node, "playlist") {
        t.whole(&join_name("playlist", &name));
        out.push((name, list.str_list()));
    }
    out
}

/// A song's overrides in `songs.yml` (empty means none).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SongMeta {
    pub author: String,
    pub title: String,
}

/// `songs.yml`: `songs:` maps a song name to its `author` and `title` (block scalars such as `|2-` included).
pub fn read_songs(root: &Node, t: &mut Tracker) -> BTreeMap<String, SongMeta> {
    let mut out = BTreeMap::new();
    let Some(node) = t.child(root, "", "songs") else {
        return out;
    };
    for (name, value) in t.entries(node, "songs") {
        let path = join_name("songs", &name);
        let author = t.text(value, &path, "author").unwrap_or_default();
        let title = t.text(value, &path, "title").unwrap_or_default();
        out.insert(name, SongMeta { author, title });
    }
    out
}

/// `holograms.yml`: `hologram:` maps a name to its `lines`. Read, but there is no hologram provider here.
pub fn read_holograms(root: &Node, t: &mut Tracker) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let Some(node) = t.child(root, "", "hologram") else {
        return out;
    };
    for (name, value) in t.entries(node, "hologram") {
        let path = join_name("hologram", &name);
        let mut lines = Vec::new();
        if let Some(l) = t.child(value, &path, "lines") {
            t.whole(&join_name(&path, "lines"));
            lines = l.str_list();
        }
        out.push((name, lines));
    }
    out
}

/// `messages_<language>.yml`: every top-level key with a text.
pub fn read_messages(root: &Node, t: &mut Tracker) -> Messages {
    let mut pairs = Vec::new();
    for (key, value) in t.entries(root, "") {
        if let Some(text) = value.as_str() {
            pairs.push((key, text.to_string()));
        }
    }
    Messages::from_pairs(pairs)
}

/// Everything the engine reads from the plugin folder.
#[derive(Debug)]
pub struct Files {
    pub config: ConfigFile,
    /// False when `config.yml` is missing or does not parse (then nothing is reloaded).
    pub config_ok: bool,
    pub playlists: Vec<(String, Vec<String>)>,
    /// The parsed `playlists.yml`, kept so a save keeps its layout.
    pub playlists_doc: Option<Document>,
    pub world_radios: Vec<RadioDef>,
    pub server_radios: Vec<RadioDef>,
    pub position_radios: Vec<RadioDef>,
    pub songs: BTreeMap<String, SongMeta>,
    pub holograms: Vec<(String, Vec<String>)>,
    pub messages: Messages,
    pub log: Vec<String>,
}

fn read_doc(dir: &Path, name: &str, log: &mut Vec<String>) -> Option<Document> {
    let text = match std::fs::read_to_string(dir.join(name)) {
        Ok(text) => text,
        Err(_) => {
            log.push(format!("{name} is missing, defaults are used"));
            return None;
        }
    };
    match yaml::parse_document(&text) {
        Ok(doc) => Some(doc),
        Err(e) => {
            log.push(format!("{name} does not parse: {e}"));
            None
        }
    }
}

/// Reads the plugin folder the way NBMP's `config.yml` points to its files.
pub fn load_files(dir: &Path) -> Files {
    let mut log = Vec::new();
    let config_doc = read_doc(dir, "config.yml", &mut log);
    let config_ok = config_doc.is_some();
    let config = match &config_doc {
        Some(doc) => read_config(&doc.root, &mut Tracker::default()),
        None => ConfigFile::default(),
    };
    let playlists_doc = read_doc(dir, &config.playlists_file, &mut log);
    let playlists = match &playlists_doc {
        Some(doc) => read_playlists(&doc.root, &mut Tracker::default()),
        None => Vec::new(),
    };
    let world_doc = read_doc(dir, &config.world_radio_file, &mut log);
    let world_radios = match &world_doc {
        Some(doc) => read_radio_section(&doc.root, &mut Tracker::default(), "worldradios"),
        None => Vec::new(),
    };
    let server_doc = read_doc(dir, &config.server_radio_file, &mut log);
    let server_radios = match &server_doc {
        Some(doc) => read_radio_section(&doc.root, &mut Tracker::default(), "serverradios"),
        None => Vec::new(),
    };
    let position_doc = read_doc(dir, &config.position_radio_file, &mut log);
    let position_radios = match &position_doc {
        Some(doc) => read_position(&doc.root, &mut Tracker::default()),
        None => Vec::new(),
    };
    let songs = match read_doc(dir, "songs.yml", &mut log) {
        Some(doc) => read_songs(&doc.root, &mut Tracker::default()),
        None => BTreeMap::new(),
    };
    let holograms = match read_doc(dir, &config.holograms_file, &mut log) {
        Some(doc) => read_holograms(&doc.root, &mut Tracker::default()),
        None => Vec::new(),
    };
    let messages_name = format!("messages_{}.yml", config.language);
    let messages = match read_doc(dir, &messages_name, &mut log) {
        Some(doc) => read_messages(&doc.root, &mut Tracker::default()),
        None => Messages::default(),
    };
    Files {
        config,
        config_ok,
        playlists,
        playlists_doc,
        world_radios,
        server_radios,
        position_radios,
        songs,
        holograms,
        messages,
        log,
    }
}

/// What `check_original` prints for one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileReport {
    pub file: String,
    pub parse_error: Option<String>,
    pub read: Vec<String>,
    pub unread: Vec<String>,
}

/// Reads one original file (by its file name) with its reader and lists the keys read and not read.
pub fn report_for(file: &str, text: &str) -> FileReport {
    let doc = match yaml::parse_document(text) {
        Ok(doc) => doc,
        Err(e) => {
            return FileReport {
                file: file.to_string(),
                parse_error: Some(e.to_string()),
                read: vec![],
                unread: vec![],
            };
        }
    };
    let mut t = Tracker::default();
    let root = &doc.root;
    let known = match file {
        "config.yml" => {
            read_config(root, &mut t);
            true
        }
        "playlists.yml" => {
            read_playlists(root, &mut t);
            true
        }
        "globalRadio.yml" => {
            read_radio_section(root, &mut t, "worldradios");
            read_radio_section(root, &mut t, "serverradios");
            true
        }
        "positionRadio.yml" => {
            read_position(root, &mut t);
            true
        }
        "songs.yml" => {
            read_songs(root, &mut t);
            true
        }
        "holograms.yml" => {
            read_holograms(root, &mut t);
            true
        }
        f if f.starts_with("messages_") => {
            read_messages(root, &mut t);
            true
        }
        _ => false,
    };
    if !known {
        return FileReport {
            file: file.to_string(),
            parse_error: Some("no reader for this file name".to_string()),
            read: vec![],
            unread: vec![],
        };
    }
    let mut all = Vec::new();
    leaves(root, "", &mut all);
    let unread = all.into_iter().filter(|p| !t.covers(p)).collect();
    FileReport {
        file: file.to_string(),
        parse_error: None,
        read: t.read_paths(),
        unread,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADIOS: &str = "worldradios:\n  world_radio:\n    enable: false\n    world: fixture_world\n    repeat: true\n    random: false\n    actionbar: true\n    playlist: fixture\n    delay: 2\n    volume: 100\n    extra: 1\nserverradios:\n  server_radio:\n    enable: false\n    repeat: true\n    random: false\n    actionbar: true\n    playlist: fixture\n    delay: 2\n    volume: 80\n";

    fn doc(text: &str) -> Document {
        yaml::parse_document(text).expect("fixture parses")
    }

    #[test]
    fn radio_sections_read_every_key_the_file_has() {
        let d = doc(RADIOS);
        let mut t = Tracker::default();
        let world = read_radio_section(&d.root, &mut t, "worldradios");
        let server = read_radio_section(&d.root, &mut t, "serverradios");
        assert_eq!(world.len(), 1);
        assert!(!world[0].enable);
        assert_eq!(world[0].world, "fixture_world");
        assert_eq!(world[0].delay, 2.0);
        assert_eq!(world[0].volume, 100);
        assert_eq!(server[0].volume, 80);
        assert_eq!(server[0].name, "server_radio");
    }

    #[test]
    fn report_lists_the_unread_key_of_a_radio_file() {
        let report = report_for("globalRadio.yml", RADIOS);
        assert!(report.parse_error.is_none());
        assert!(
            report
                .unread
                .iter()
                .any(|p| p == "worldradios.world_radio.extra"),
            "{:?}",
            report.unread
        );
        assert!(
            !report
                .unread
                .iter()
                .any(|p| p == "worldradios.world_radio.enable")
        );
    }

    #[test]
    fn position_file_reads_the_stray_key_and_the_radio() {
        let text = "radios: {}\n\n\npositionradios:\n  test_radio:\n    enable: false\n    world: world\n    x: 0\n    y: 100\n    z: 0\n    radius: 16\n    hologram: example\n    hologramOP: example_op\n    repeat: true\n    random: false\n    actionbar: true\n    delay: 0\n    playlist: test\n    volume: 100\n";
        let report = report_for("positionRadio.yml", text);
        assert!(report.unread.is_empty(), "{:?}", report.unread);
        assert!(report.read.iter().any(|p| p == "radios"));
        let d = doc(text);
        let radios = read_position(&d.root, &mut Tracker::default());
        assert_eq!(radios[0].radius, 16.0);
        assert_eq!(radios[0].center, [0.0, 100.0, 0.0]);
        assert_eq!(radios[0].hologram_op, "example_op");
    }

    #[test]
    fn playlists_are_read_in_order_as_whole_values() {
        let text = "playlist:\n  fixture:\n  - first_song\n  - second_song\n  empty: []\n";
        let d = doc(text);
        let lists = read_playlists(&d.root, &mut Tracker::default());
        assert_eq!(
            lists[0],
            (
                "fixture".to_string(),
                vec!["first_song".to_string(), "second_song".to_string()]
            )
        );
        assert_eq!(lists[1], ("empty".to_string(), Vec::<String>::new()));
        let report = report_for("playlists.yml", text);
        assert!(report.unread.is_empty(), "{:?}", report.unread);
    }

    #[test]
    fn songs_block_scalar_title_is_read() {
        let text = "songs:\n  carol:\n    author: ''\n    title: |2-\n\n      Carol of the Bells\n  plain:\n    author: Someone\n    title: Plain\n";
        let d = doc(text);
        let songs = read_songs(&d.root, &mut Tracker::default());
        assert_eq!(songs["carol"].title.trim(), "Carol of the Bells");
        assert_eq!(songs["plain"].author, "Someone");
        assert_eq!(songs["plain"].title, "Plain");
    }

    #[test]
    fn holograms_read_their_lines() {
        let text = "hologram:\n  example:\n    lines:\n    - 'Playing: %song_title%'\n    - '%song_number%/%songs_count%'\n";
        let d = doc(text);
        let holo = read_holograms(&d.root, &mut Tracker::default());
        assert_eq!(holo[0].0, "example");
        assert_eq!(holo[0].1.len(), 2);
    }

    #[test]
    fn config_reads_language_volumes_and_file_names() {
        let text = "language: en\nconfig_Playlists: playlists.yml\nconfig_WorldRadio: globalRadio.yml\nconfig_ServerRadio: globalRadio.yml\nconfig_PositionRadio: positionRadio.yml\nconfig_Holograms: holograms.yml\ndefaultPlayerVolume: 60\nsavePlayerVolumes: false\nuse10OctaveRange: true\n";
        let d = doc(text);
        let c = read_config(&d.root, &mut Tracker::default());
        assert_eq!(c.language, "en");
        assert_eq!(c.default_player_volume, 60);
        assert!(c.use_10_octave_range);
        assert_eq!(c.position_radio_file, "positionRadio.yml");
        let report = report_for("config.yml", text);
        assert!(report.unread.is_empty(), "{:?}", report.unread);
    }

    #[test]
    fn messages_read_every_text_key() {
        let d = doc("prefix: \"&9[X] \"\nactionbar: \"You are now listening %song_title%\"\n");
        let m = read_messages(&d.root, &mut Tracker::default());
        assert_eq!(m.raw("actionbar"), "You are now listening %song_title%");
        assert_eq!(m.raw("prefix"), "&9[X] ");
    }

    #[test]
    fn unknown_file_name_has_no_reader() {
        let report = report_for("other.yml", "a: 1\n");
        assert!(report.parse_error.is_some());
    }

    #[test]
    fn parse_error_is_reported_without_values() {
        let report = report_for("config.yml", "a: [unclosed\n");
        assert!(report.parse_error.is_some());
        assert!(report.read.is_empty());
    }

    #[test]
    fn missing_folder_gives_defaults_and_a_log_line() {
        let dir = std::env::temp_dir().join("horizon-noteblock-missing-folder-test");
        let files = load_files(&dir);
        assert!(!files.config_ok);
        assert_eq!(files.config, ConfigFile::default());
        assert!(files.world_radios.is_empty());
        assert!(files.log.iter().any(|l| l.contains("config.yml")));
    }
}
