//! The radios of one server: loading the plugin folder, the radios' ticks, the sounds and action bar lines they
//! answer with, the switches and playlists the commands change, and the player state (mute, volume).
//!
//! The engine knows players only by uuid, dimension and position (the glue passes them in each tick). World
//! names follow Bukkit's: the level's name in the overworld, `<level>_nether` and `<level>_the_end` elsewhere.

use crate::horizon_noteblock::commands::Answer;
use crate::horizon_noteblock::config::{self, ConfigFile, Files, RadioDef};
use crate::horizon_noteblock::instrument;
use crate::horizon_noteblock::library::Library;
use crate::horizon_noteblock::messages::{self, Messages};
use crate::horizon_noteblock::pitch;
use crate::horizon_noteblock::radio::{Event, Kind, Radio, RadioSpec, Rng, MS_PER_SERVER_TICK};
use crate::horizon_noteblock::yaml::{self, Document, Node};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// NBMP's per-player volume file, in the plugin folder (kept only with `savePlayerVolumes: true`).
const PLAYER_VOLUMES_FILE: &str = "playerVolumes.yml";

/// A player's UUID as Bukkit writes it in files: 8-4-4-4-12 lowercase hex digits.
pub fn uuid_text(uuid: u128) -> String {
    let hex = format!("{uuid:032x}");
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

/// The UUID a file key names (dashed or not); `None` for anything that is not 32 hex digits.
pub fn parse_uuid(text: &str) -> Option<u128> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u128::from_str_radix(&hex, 16).ok()
}

/// A player as the radios see them.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerView {
    pub uuid: u128,
    /// `minecraft:overworld`, `minecraft:the_nether` or `minecraft:the_end`.
    pub dimension: String,
    pub pos: [f64; 3],
}

/// The players of the server this tick, and the name of its level (the world folder's name).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldView {
    pub level: String,
    pub players: Vec<PlayerView>,
}

/// What a tick sends: a note's sound to one player (as `Player.playSound`, pitch and volume as Minecraft takes
/// them), or an action bar line (`&` colour codes already turned into `§`).
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    Sound {
        uuid: u128,
        sound: String,
        volume: f32,
        pitch: f32,
    },
    ActionBar {
        uuid: u128,
        line: String,
    },
}

/// NoteBlockAPI's song player events, for code in this binary (see `api`). `radio` is the radio's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SongEvent {
    /// A song starts (`SongStartedEvent`).
    Started { radio: String, song: String },
    /// A song ends, by running out or by `skip` (`SongEndedEvent`).
    Ended { radio: String, song: String },
    /// A radio is switched off while it may play (`SongStoppedEvent`).
    Stopped { radio: String },
}

/// Bukkit's world name for a player in `level` in this dimension.
pub fn world_name(level: &str, dimension: &str) -> String {
    match dimension {
        "minecraft:the_nether" => format!("{level}_nether"),
        "minecraft:the_end" => format!("{level}_the_end"),
        _ => level.to_string(),
    }
}

fn radio_message_vars(radio: &Radio) -> messages::Vars {
    vec![
        ("radio_type", radio.kind.word().to_string()),
        ("radio", radio.name.clone()),
        ("playlist", radio.playlist.clone()),
        ("world", radio.world.clone()),
    ]
}

fn radio_vars(kind: Kind, name: &str) -> messages::Vars {
    vec![
        ("radio_type", kind.word().to_string()),
        ("radio", name.to_string()),
    ]
}

pub struct Engine {
    dir: PathBuf,
    worlds: Vec<String>,
    cfg: ConfigFile,
    messages: Messages,
    library: Library,
    playlists: Vec<(String, Vec<String>)>,
    playlists_doc: Option<Document>,
    holograms: Vec<(String, Vec<String>)>,
    radios: Vec<Radio>,
    rng: Rng,
    volumes: HashMap<u128, u8>,
    muted: HashSet<u128>,
    present: HashSet<u128>,
    log: Vec<String>,
    events: Vec<SongEvent>,
}

impl Engine {
    /// Reads the plugin folder (`plugins/NoteBlockMusicPlayer/`), loads the playlists' songs and switches on the
    /// radios its files enable that pass the checks. `worlds` are the level's world names.
    pub fn load(dir: &Path, worlds: Vec<String>, seed: u64) -> Engine {
        let mut engine = Engine {
            dir: dir.to_path_buf(),
            worlds,
            cfg: ConfigFile::default(),
            messages: Messages::default(),
            library: Library::new(dir.join("songs"), BTreeMap::new()),
            playlists: Vec::new(),
            playlists_doc: None,
            holograms: Vec::new(),
            radios: Vec::new(),
            rng: Rng::new(seed),
            volumes: HashMap::new(),
            muted: HashSet::new(),
            present: HashSet::new(),
            log: Vec::new(),
            events: Vec::new(),
        };
        let files = config::load_files(dir);
        engine.apply(files);
        if engine.cfg.save_player_volumes {
            engine.read_player_volumes();
        }
        engine
    }

    /// Lines for the console (what the loading and the checks said), taken once.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// The song events since the last call, in order.
    pub fn take_song_events(&mut self) -> Vec<SongEvent> {
        std::mem::take(&mut self.events)
    }

    /// Queues a song event. Without a taker the oldest ones go after 1024, so an idle queue cannot grow forever.
    fn push_event(&mut self, event: SongEvent) {
        const MAX_QUEUED: usize = 1024;
        if self.events.len() >= MAX_QUEUED {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    fn note(&mut self, text: String) {
        self.log.push(messages::plain(&text));
    }

    /// Installs what was read from the folder: radios are rebuilt, and those the files enable are switched on
    /// when they pass the checks.
    fn apply(&mut self, files: Files) {
        let Files {
            config,
            config_ok: _,
            playlists,
            playlists_doc,
            world_radios,
            server_radios,
            position_radios,
            songs,
            holograms,
            messages,
            log,
        } = files;
        self.log.extend(log);
        self.cfg = config;
        self.messages = messages;
        self.playlists = playlists;
        self.playlists_doc = playlists_doc;
        self.holograms = holograms;
        self.library = Library::new(self.dir.join("songs"), songs);
        if self.cfg.use_10_octave_range {
            self.log.push("use10OctaveRange is set: notes outside the two octaves use the suffixed sounds (`_-2`, `_-1`, `_1`, `_2`) of the resource pack".to_string());
        }
        let mut names: Vec<String> = Vec::new();
        for (_, list) in &self.playlists {
            for song in list {
                if !names.contains(song) {
                    names.push(song.clone());
                }
            }
        }
        for name in names {
            let mut log = Vec::new();
            self.library.ensure(&name, &mut log);
            self.log.extend(log);
        }

        let mut defs: Vec<(Kind, RadioDef)> = Vec::new();
        defs.extend(world_radios.into_iter().map(|d| (Kind::World, d)));
        defs.extend(server_radios.into_iter().map(|d| (Kind::Server, d)));
        defs.extend(position_radios.into_iter().map(|d| (Kind::Position, d)));
        let mut radios = Vec::new();
        let mut enable_flags = Vec::new();
        for (kind, def) in defs {
            enable_flags.push(def.enable);
            let spec = RadioSpec {
                kind,
                name: def.name.clone(),
                world: def.world.clone(),
                center: def.center,
                radius: def.radius,
                playlist: def.playlist.clone(),
                songs: self.playlist_songs(&def.playlist),
                repeat: def.repeat,
                random: def.random,
                actionbar: def.actionbar,
                delay_secs: def.delay,
                volume: def.volume,
            };
            radios.push(Radio::new(spec));
        }
        self.radios = radios;
        for idx in 0..self.radios.len() {
            self.radios[idx].loaded = self.problem(idx).is_none();
        }
        for (idx, enable) in enable_flags.into_iter().enumerate() {
            if !enable {
                continue;
            }
            if let Some((key, vars)) = self.problem(idx) {
                let text = self.messages.prefixed(key, &vars);
                self.note(text);
            } else if let Some((key, vars)) = self.multiple(idx) {
                let text = self.messages.prefixed(key, &vars);
                self.note(text);
            } else {
                let rng = &mut self.rng;
                self.radios[idx].start(rng);
            }
        }
    }

    fn playlist_songs(&self, playlist: &str) -> Vec<String> {
        match self
            .playlists
            .iter()
            .find(|(name, _)| name.as_str() == playlist)
        {
            Some((_, list)) => list
                .iter()
                .filter(|s| self.library.contains(s))
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }

    /// Why a radio cannot be switched on at all (its playlist, its songs, its world), with the message's key.
    fn problem(&self, idx: usize) -> Option<(&'static str, messages::Vars)> {
        let radio = &self.radios[idx];
        if !self
            .playlists
            .iter()
            .any(|(name, _)| name.as_str() == radio.playlist.as_str())
        {
            return Some(("radio_noplaylist", radio_message_vars(radio)));
        }
        if radio.songs.is_empty() {
            return Some(("radio_nosongs", radio_message_vars(radio)));
        }
        if radio.kind != Kind::Server
            && !self
                .worlds
                .iter()
                .any(|w| w.as_str() == radio.world.as_str())
        {
            return Some(("world_radio_badworld", radio_message_vars(radio)));
        }
        None
    }

    /// Why another radio of the same kind already plays (one world radio per world, one server radio).
    fn multiple(&self, idx: usize) -> Option<(&'static str, messages::Vars)> {
        let radio = &self.radios[idx];
        let others = self
            .radios
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != idx)
            .map(|(_, r)| r);
        match radio.kind {
            Kind::World => {
                let taken = others
                    .into_iter()
                    .any(|o| o.enabled && o.kind == Kind::World && o.world == radio.world);
                if taken {
                    Some(("world_radio_multiple", radio_message_vars(radio)))
                } else {
                    None
                }
            }
            Kind::Server => {
                let taken = others
                    .into_iter()
                    .any(|o| o.enabled && o.kind == Kind::Server);
                if taken {
                    Some(("server_radio_multiple", radio_message_vars(radio)))
                } else {
                    None
                }
            }
            Kind::Position => None,
        }
    }

    pub(crate) fn find(&self, kind: Kind, name: &str) -> Option<usize> {
        self.radios
            .iter()
            .position(|r| r.kind == kind && r.name == name)
    }

    /// The song a radio plays now, if it plays one.
    pub fn current_song(&self, kind: Kind, name: &str) -> Option<String> {
        let idx = self.find(kind, name)?;
        self.radios[idx].current_song().map(str::to_owned)
    }

    /// The radios: the sounds and action bar lines for one server tick (50 ms of time, see `tick_for`).
    pub fn tick(&mut self, view: &WorldView) -> Vec<Out> {
        self.tick_for(view, MS_PER_SERVER_TICK)
    }

    /// The radios: the sounds and action bar lines for `elapsed_ms` of real time since the last call.
    pub fn tick_for(&mut self, view: &WorldView, elapsed_ms: f32) -> Vec<Out> {
        let mut out = Vec::new();
        // NBMP's `onPlayerJoin`: a player who was not here at the last tick gets the action bar of the first radio
        // that plays to them. Volume and mute are not dropped when a player leaves: NBMP's `PlayerVolume` map is
        // never pruned, and the file keeps them only when `savePlayerVolumes` is on.
        let joined: Vec<u128> = view
            .players
            .iter()
            .map(|p| p.uuid)
            .filter(|uuid| !self.present.contains(uuid))
            .collect();
        self.present = view.players.iter().map(|p| p.uuid).collect();
        for uuid in joined {
            if let Some(line) = self.join_line(view, uuid) {
                out.push(Out::ActionBar { uuid, line });
            }
        }
        for idx in 0..self.radios.len() {
            if !self.radios[idx].enabled {
                continue;
            }
            let events = {
                let radio = &mut self.radios[idx];
                radio.step_for(&self.library, &mut self.rng, elapsed_ms)
            };
            for event in events {
                match event {
                    Event::End { song } => {
                        let radio = self.radios[idx].name.clone();
                        self.push_event(SongEvent::Ended { radio, song });
                    }
                    Event::Start { song } => {
                        let radio = self.radios[idx].name.clone();
                        self.push_event(SongEvent::Started { radio, song: song.clone() });
                        if !self.radios[idx].actionbar {
                            continue;
                        }
                        let title = self
                            .library
                            .get(&song)
                            .map(|e| e.title.clone())
                            .unwrap_or_else(|| song.clone());
                        let line = self.messages.get("actionbar", &[("song_title", title)]);
                        for uuid in self.audience(idx, view) {
                            out.push(Out::ActionBar {
                                uuid,
                                line: line.clone(),
                            });
                        }
                    }
                    Event::Tick { song, tick } => {
                        let Some(entry) = self.library.get(&song) else {
                            continue;
                        };
                        let notes = entry.song.notes_at(tick);
                        if notes.is_empty() {
                            continue;
                        }
                        let radio_volume = self.radios[idx].volume;
                        for uuid in self.audience(idx, view) {
                            let player_volume = self.player_volume(uuid);
                            for (layer, note) in notes {
                                let ten = self.cfg.use_10_octave_range;
                                let (key, fine) = (note.key as i32, note.pitch as i32);
                                // A custom instrument plays its sound file's name (NoteBlockAPI's
                                // `CustomInstrument`); a vanilla one its note block sound.
                                let base: String = if instrument::is_custom(note.instrument) {
                                    let at = (note.instrument as usize).wrapping_sub(instrument::FIRST_CUSTOM as usize);
                                    let Some(custom) = entry.song.custom_instruments.get(at) else {
                                        continue;
                                    };
                                    instrument::custom_sound(&custom.file)
                                } else {
                                    let Some(sound) = instrument::sound_name(note.instrument) else {
                                        continue;
                                    };
                                    sound.to_owned()
                                };
                                // With `use10OctaveRange` a note outside the two octaves uses the suffixed sound.
                                let sound = if ten && (instrument::is_custom(note.instrument) || instrument::out_of_range(key, fine)) {
                                    instrument::warp_name(&base, key, fine)
                                } else {
                                    base
                                };
                                let layer_volume = entry
                                    .song
                                    .layers
                                    .get(*layer)
                                    .map(|l| l.volume)
                                    .unwrap_or(100);
                                let volume = pitch::volume(
                                    layer_volume,
                                    radio_volume,
                                    player_volume,
                                    note.velocity,
                                );
                                let note_pitch = if self.cfg.use_10_octave_range {
                                    pitch::in_octave(note.key as i32, note.pitch as i32)
                                } else {
                                    pitch::transposed(note.key as i32, note.pitch as i32)
                                };
                                out.push(Out::Sound {
                                    uuid,
                                    sound,
                                    volume,
                                    pitch: note_pitch,
                                });
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// The action bar line a player gets when they join: the song of the first enabled radio that has an action bar,
    /// plays a song now and plays to them (NBMP shows its top-priority radio; the priorities are approximated by the
    /// order of the radios).
    fn join_line(&self, view: &WorldView, uuid: u128) -> Option<String> {
        for idx in 0..self.radios.len() {
            let radio = &self.radios[idx];
            if !radio.enabled || !radio.actionbar {
                continue;
            }
            let Some(song) = radio.current_song() else {
                continue;
            };
            if !self.audience(idx, view).contains(&uuid) {
                continue;
            }
            let title = self
                .library
                .get(song)
                .map(|e| e.title.clone())
                .unwrap_or_else(|| song.to_string());
            return Some(self.messages.get("actionbar", &[("song_title", title)]));
        }
        None
    }

    /// The players a radio plays to this tick: not muted, and in its world (and within its radius for a position
    /// radio).
    fn audience(&self, idx: usize, view: &WorldView) -> Vec<u128> {
        let radio = &self.radios[idx];
        view.players
            .iter()
            .filter(|p| !self.muted.contains(&p.uuid))
            .filter(|p| match radio.kind {
                Kind::Server => true,
                Kind::World => world_name(&view.level, &p.dimension) == radio.world,
                Kind::Position => {
                    world_name(&view.level, &p.dimension) == radio.world
                        && distance(p.pos, radio.center) <= radio.radius
                }
            })
            .map(|p| p.uuid)
            .collect()
    }

    pub fn player_volume(&self, uuid: u128) -> u8 {
        self.volumes
            .get(&uuid)
            .copied()
            .unwrap_or(self.cfg.default_player_volume)
    }

    pub fn set_volume(&mut self, uuid: u128, volume: u8) {
        self.volumes.insert(uuid, volume.min(100));
        self.save_player_volumes();
    }

    pub fn is_muted(&self, uuid: u128) -> bool {
        self.muted.contains(&uuid)
    }

    /// Mutes or unmutes every radio for one player (`/radio toggle`); true when now muted.
    pub fn toggle_mute(&mut self, uuid: u128) -> bool {
        let now = if self.muted.remove(&uuid) {
            false
        } else {
            self.muted.insert(uuid);
            true
        };
        self.save_player_volumes();
        now
    }

    /// Reads `playerVolumes.yml` when `savePlayerVolumes` is on: per player a `volume` and a `mute`, under
    /// `playerVolumes` (NBMP's `PlayerVolume` file, keys as the UUID's text).
    fn read_player_volumes(&mut self) {
        let text = match std::fs::read_to_string(self.dir.join(PLAYER_VOLUMES_FILE)) {
            Ok(text) => text,
            Err(_) => return,
        };
        let doc = match yaml::parse_document(&text) {
            Ok(doc) => doc,
            Err(e) => {
                self.log.push(format!("{PLAYER_VOLUMES_FILE} does not parse: {e}"));
                return;
            }
        };
        let Some(section) = doc.root.get("playerVolumes") else {
            return;
        };
        let keys: Vec<String> = section.keys().map(str::to_owned).collect();
        for key in keys {
            let (Some(uuid), Some(entry)) = (parse_uuid(&key), section.get(&key)) else {
                continue;
            };
            if let Some(volume) = entry.get("volume").and_then(|v| v.as_i64()) {
                self.volumes.insert(uuid, volume.clamp(0, 100) as u8);
            }
            if entry.get("mute").and_then(|v| v.as_bool()).unwrap_or(false) {
                self.muted.insert(uuid);
            }
        }
    }

    /// Writes `playerVolumes.yml` after a change, when `savePlayerVolumes` is on. Failures go to the log.
    fn save_player_volumes(&mut self) {
        if !self.cfg.save_player_volumes {
            return;
        }
        let mut uuids: Vec<u128> = self.volumes.keys().chain(self.muted.iter()).copied().collect();
        uuids.sort();
        uuids.dedup();
        let mut section = Node::map();
        for uuid in uuids {
            let mut entry = Node::map();
            entry.set("volume", Node::int(i64::from(self.player_volume(uuid))));
            entry.set("mute", Node::bool(self.muted.contains(&uuid)));
            section.set(&uuid_text(uuid), entry);
        }
        let mut root = Node::map();
        root.set("playerVolumes", section);
        if let Err(e) = std::fs::write(self.dir.join(PLAYER_VOLUMES_FILE), yaml::write(&root)) {
            self.log.push(format!("{PLAYER_VOLUMES_FILE} could not be saved: {e}"));
        }
    }

    pub fn prefixed(&self, key: &str, vars: &[(&str, String)]) -> String {
        self.messages.prefixed(key, vars)
    }

    pub fn text(&self, key: &str, vars: &[(&str, String)]) -> String {
        self.messages.get(key, vars)
    }

    /// `/<kind>radio enable|disable <name>`: the reply.
    pub fn set_state(&mut self, kind: Kind, name: &str, on: bool) -> String {
        let Some(idx) = self.find(kind, name) else {
            return self.prefixed("command_radio_notfound", &radio_vars(kind, name));
        };
        if !on {
            if self.radios[idx].enabled {
                self.push_event(SongEvent::Stopped { radio: name.to_string() });
            }
            self.radios[idx].stop();
            return self.prefixed("command_state_disable", &radio_vars(kind, name));
        }
        if !self.radios[idx].loaded {
            return self.prefixed("command_state_not_loaded", &radio_vars(kind, name));
        }
        if let Some((key, vars)) = self.multiple(idx) {
            return self.prefixed(key, &vars);
        }
        let rng = &mut self.rng;
        self.radios[idx].start(rng);
        self.prefixed("command_state_enable", &radio_vars(kind, name))
    }

    /// `/<kind>radio skip <name>`: the reply.
    pub fn skip(&mut self, kind: Kind, name: &str) -> String {
        let Some(idx) = self.find(kind, name) else {
            return self.prefixed("command_radio_notfound", &radio_vars(kind, name));
        };
        let playing = self.radios[idx].loaded
            && self.radios[idx].enabled
            && self.radios[idx].current_song().is_some();
        if !playing {
            return self.prefixed("command_skip_song_not_loaded", &radio_vars(kind, name));
        }
        let title = match self.radios[idx].current_song() {
            Some(song) => self
                .library
                .get(song)
                .map(|e| e.title.clone())
                .unwrap_or_else(|| song.to_string()),
            None => String::new(),
        };
        let rng = &mut self.rng;
        if let Some(song) = self.radios[idx].skip(rng) {
            self.push_event(SongEvent::Ended { radio: name.to_string(), song });
        }
        let mut vars = radio_vars(kind, name);
        vars.push(("song_title", title));
        self.prefixed("command_skip_song", &vars)
    }

    /// Re-reads the plugin folder (the radios are switched off first, as NBMP's reload does). False when
    /// `config.yml` cannot be read; nothing changes then.
    pub fn reload(&mut self) -> bool {
        let files = config::load_files(&self.dir);
        if !files.config_ok {
            self.log.extend(files.log);
            return false;
        }
        for radio in self.radios.iter_mut() {
            radio.stop();
        }
        self.apply(files);
        true
    }

    /// The reply of `/<kind>radio reload`.
    pub fn reload_kind(&mut self, kind: Kind) -> String {
        let ok = self.reload();
        let vars = vec![("radio_type", kind.word().to_string())];
        if ok {
            self.prefixed("command_radio_reload_success", &vars)
        } else {
            self.prefixed("command_radio_reload_failed", &vars)
        }
    }

    /// The reply of `/nbmp reload`.
    pub fn reload_all(&mut self) -> String {
        if self.reload() {
            self.prefixed("command_global_reload_success", &[])
        } else {
            self.prefixed("command_global_reload_failed", &[])
        }
    }

    /// The reply of `/nbmp playlist reload`.
    pub fn reload_playlists(&mut self) -> String {
        if self.reload() {
            self.prefixed("command_playlist_reload_success", &[])
        } else {
            self.prefixed("command_playlist_reload_failed", &[])
        }
    }

    pub fn playlist_names(&self) -> Vec<String> {
        self.playlists
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub fn playlist_entries(&self, name: &str) -> Option<Vec<String>> {
        self.playlists
            .iter()
            .find(|(n, _)| n.as_str() == name)
            .map(|(_, list)| list.clone())
    }

    fn song_title(&self, name: &str) -> String {
        self.library
            .get(name)
            .map(|e| e.title.clone())
            .unwrap_or_else(|| name.to_string())
    }

    /// `/nbmp playlist create <name>`.
    pub fn playlist_create(&mut self, name: &str) -> String {
        if self.playlists.iter().any(|(n, _)| n.as_str() == name) {
            return self.prefixed("command_playlist_exists", &[("playlist", name.to_string())]);
        }
        self.playlists.push((name.to_string(), Vec::new()));
        self.save_playlists();
        self.prefixed("command_playlist_create", &[("playlist", name.to_string())])
    }

    /// `/nbmp playlist add <playlist> <song>`.
    pub fn playlist_add(&mut self, playlist: &str, song: &str) -> String {
        let Some(idx) = self
            .playlists
            .iter()
            .position(|(n, _)| n.as_str() == playlist)
        else {
            return self.prefixed(
                "command_playlist_notfound",
                &[("playlist", playlist.to_string())],
            );
        };
        let mut log = Vec::new();
        let loaded = self.library.ensure(song, &mut log);
        self.log.extend(log);
        if !loaded {
            return self.prefixed("command_playlist_add_hint", &[]);
        }
        if !self.playlists[idx].1.iter().any(|s| s.as_str() == song) {
            self.playlists[idx].1.push(song.to_string());
        }
        self.save_playlists();
        let vars = vec![
            ("song_title", self.song_title(song)),
            ("playlist", playlist.to_string()),
        ];
        self.prefixed("command_playlist_add", &vars)
    }

    /// `/nbmp playlist remove <playlist> <song>`.
    pub fn playlist_remove(&mut self, playlist: &str, song: &str) -> String {
        let Some(idx) = self
            .playlists
            .iter()
            .position(|(n, _)| n.as_str() == playlist)
        else {
            return self.prefixed(
                "command_playlist_notfound",
                &[("playlist", playlist.to_string())],
            );
        };
        let before = self.playlists[idx].1.len();
        self.playlists[idx].1.retain(|s| s.as_str() != song);
        if self.playlists[idx].1.len() == before {
            return self.prefixed("command_playlist_remove_hint", &[]);
        }
        self.save_playlists();
        let vars = vec![
            ("song_title", self.song_title(song)),
            ("playlist", playlist.to_string()),
        ];
        self.prefixed("command_playlist_remove", &vars)
    }

    /// Writes `playlists.yml` the way NBMP keeps it (the parsed file, changed in place, so comments and order
    /// stay). Failures go to the log.
    fn save_playlists(&mut self) {
        let file = self.cfg.playlists_file.clone();
        let doc = self.playlists_doc.get_or_insert_with(|| Document {
            leading: Vec::new(),
            root: Node::map(),
            trailing: Vec::new(),
        });
        if doc.root.get("playlist").is_none() {
            doc.root.set("playlist", Node::map());
        }
        let playlists = &self.playlists;
        if let Some(map) = doc.root.get_mut("playlist") {
            let stale: Vec<String> = map
                .keys()
                .filter(|k| !playlists.iter().any(|(n, _)| n.as_str() == *k))
                .map(|k| k.to_string())
                .collect();
            for key in stale {
                map.remove(&key);
            }
            for (name, songs) in playlists {
                let items: Vec<Node> = songs.iter().map(|s| Node::str(s)).collect();
                map.set(name, Node::seq(items));
            }
        }
        let text = yaml::write_document(doc);
        if let Err(e) = std::fs::write(self.dir.join(&file), text) {
            self.log.push(format!("{file} could not be saved: {e}"));
        }
    }

    /// The holograms (inert: no hologram provider on the server).
    pub fn hologram_names(&self) -> Vec<String> {
        self.holograms
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Whether a radio of this kind and name is switched on.
    pub fn is_enabled(&self, kind: Kind, name: &str) -> bool {
        self.find(kind, name)
            .map(|i| self.radios[i].enabled)
            .unwrap_or(false)
    }

    /// The loaded song library (for the glue's tests and the console).
    pub fn library_mut(&mut self) -> &mut Library {
        &mut self.library
    }

    /// Adds a radio built in code (a test or another mod), with the same checks as a file radio; it is not
    /// switched on. Returns its index.
    pub fn add_radio(&mut self, spec: RadioSpec) -> usize {
        self.radios.push(Radio::new(spec));
        let idx = self.radios.len() - 1;
        self.radios[idx].loaded = self.problem(idx).is_none();
        idx
    }

    /// Runs a command line (without its slash) for a player (`None` for the console). See `commands`.
    pub fn command(&mut self, uuid: Option<u128>, line: &str) -> Answer {
        crate::horizon_noteblock::commands::run(self, uuid, line)
    }

    /// A player's command, with the permission check NoteBlockMusicPlayer's executors make.
    pub fn command_checked(
        &mut self,
        uuid: Option<u128>,
        line: &str,
        allowed: impl Fn(&str, u8) -> bool,
    ) -> Answer {
        crate::horizon_noteblock::commands::run_checked(self, uuid, line, allowed)
    }

    /// Names of the radios and their state, for the console and the tests.
    pub fn radio_states(&self) -> BTreeMap<String, bool> {
        self.radios
            .iter()
            .map(|r| (format!("{}:{}", r.kind.word(), r.name), r.enabled))
            .collect()
    }

    /// Names of the radios of one kind (for tab completion).
    pub fn radio_names(&self, kind: Kind) -> Vec<String> {
        self.radios
            .iter()
            .filter(|r| r.kind == kind)
            .map(|r| r.name.clone())
            .collect()
    }

    /// Names of the `songs/*.nbs` files without extension (for tab completion).
    pub fn song_files(&self) -> Vec<String> {
        self.library.file_names()
    }

    /// The raw playlist list (names and songs), for tests.
    pub fn playlists(&self) -> &[(String, Vec<String>)] {
        &self.playlists
    }

    /// The loaded config (for tests).
    pub fn config(&self) -> &ConfigFile {
        &self.cfg
    }

    /// The song count of a loaded entry, for the library test helper.
    pub fn song_loaded(&self, name: &str) -> bool {
        self.library.get(name).is_some()
    }
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
