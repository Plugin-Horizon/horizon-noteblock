//! The commands of NoteBlockMusicPlayer 1.10.0: their usages (the words, the arguments, the permission nodes of
//! `plugin.yml`) and what they answer.
//!
//! Roots: `radio` (player controls), `worldradio` (`wr`), `serverradio` (`sr`), `positionradio` (`pr`) and
//! `noteblockmusicplayer` (`nbmp`). Bukkit gives `sr` to whichever command it registers first; `serverradio` is
//! first in `plugin.yml`, so `sr` is `serverradio` here. `radio` has no alias.
//! Permission nodes come from `plugin.yml`; the gate of a sub-command with no node of its own (playlist `songs`,
//! `remove`, `hologram`) is inferred and listed in STATUS.md.

use crate::horizon_noteblock::engine::Engine;
use crate::horizon_noteblock::radio::Kind;

/// What a command answered: whether it ran (so the server does not run it as vanilla), and its lines (`§` codes).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Answer {
    pub ran: bool,
    pub lines: Vec<String>,
}

impl Answer {
    pub fn none() -> Answer {
        Answer {
            ran: false,
            lines: Vec::new(),
        }
    }

    pub fn done(lines: Vec<String>) -> Answer {
        Answer { ran: true, lines }
    }
}

/// One usage a command is registered with: the words and arguments (`<name:kind>`, as the server reads them), the
/// operator level and the permission node that decides instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Usage {
    pub usage: String,
    pub level: u8,
    pub permission: Option<String>,
}

fn usage(text: String, level: u8, permission: Option<String>) -> Usage {
    Usage {
        usage: text,
        level,
        permission,
    }
}

/// The usages to register: every root and alias, with their sub-commands. Operators (level 1) by default where
/// `plugin.yml` says `op`; players where it says `true`.
pub fn usages() -> Vec<Usage> {
    let mut out = Vec::new();
    for root in ["radio"] {
        out.push(usage(root.to_string(), 0, None));
        out.push(usage(format!("{root} help"), 0, None));
        out.push(usage(
            format!("{root} toggle"),
            0,
            Some("radio.toggle".to_string()),
        ));
        out.push(usage(
            format!("{root} volume [level:integer]"),
            0,
            Some("radio.volume".to_string()),
        ));
    }
    for (root, kind) in [
        ("worldradio", Kind::World),
        ("wr", Kind::World),
        ("serverradio", Kind::Server),
        ("sr", Kind::Server),
        ("positionradio", Kind::Position),
        ("pr", Kind::Position),
    ] {
        let node = kind.command();
        out.push(usage(root.to_string(), 1, Some(format!("{node}.admin"))));
        out.push(usage(
            format!("{root} help"),
            1,
            Some(format!("{node}.admin")),
        ));
        out.push(usage(
            format!("{root} enable <name:word>"),
            1,
            Some(format!("{node}.enable")),
        ));
        out.push(usage(
            format!("{root} disable <name:word>"),
            1,
            Some(format!("{node}.enable")),
        ));
        out.push(usage(
            format!("{root} skip <name:word>"),
            1,
            Some(format!("{node}.skip")),
        ));
        out.push(usage(
            format!("{root} reload"),
            1,
            Some(format!("{node}.admin")),
        ));
    }
    for root in ["noteblockmusicplayer", "nbmp"] {
        out.push(usage(root.to_string(), 1, Some("nbmp.admin".to_string())));
        out.push(usage(
            format!("{root} help"),
            1,
            Some("nbmp.admin".to_string()),
        ));
        out.push(usage(
            format!("{root} reload"),
            1,
            Some("nbmp.admin".to_string()),
        ));
        out.push(usage(
            format!("{root} playlist help"),
            1,
            Some("nbmp.playlist.admin".to_string()),
        ));
        out.push(usage(
            format!("{root} playlist list"),
            1,
            Some("nbmp.playlist.list".to_string()),
        ));
        out.push(usage(
            format!("{root} playlist songs <playlist:word>"),
            1,
            Some("nbmp.playlist.list".to_string()),
        ));
        out.push(usage(
            format!("{root} playlist create <playlist:word>"),
            1,
            Some("nbmp.playlist.create".to_string()),
        ));
        out.push(usage(
            format!("{root} playlist add <playlist:word> <song:word>"),
            1,
            Some("nbmp.playlist.add".to_string()),
        ));
        out.push(usage(
            format!("{root} playlist remove <playlist:word> <song:word>"),
            1,
            Some("nbmp.playlist.admin".to_string()),
        ));
        out.push(usage(
            format!("{root} playlist reload"),
            1,
            Some("nbmp.playlist.reload".to_string()),
        ));
        out.push(usage(
            format!("{root} hologram reload"),
            1,
            Some("nbmp.hologram.admin".to_string()),
        ));
        out.push(usage(
            format!("{root} hologram list"),
            1,
            Some("nbmp.hologram.admin".to_string()),
        ));
    }
    out
}

/// The permission node and default operator level (0 everyone, 1 operators, plugin.yml's `default`) that a
/// command line needs: the node of the most specific usage whose literal words the line starts with.
/// NoteBlockMusicPlayer declares its commands without a `permission:`, so Bukkit shows them to everyone and
/// its executors check `hasPermission` when they run (`COMMAND_PERMISSIONS` when it is missing).
pub fn needs(line: &str) -> Option<(String, u8)> {
    let words: Vec<String> = line.split_whitespace().map(str::to_ascii_lowercase).collect();
    let mut best: Option<(usize, &Usage)> = None;
    let all = usages();
    for u in &all {
        let literal: Vec<&str> = u
            .usage
            .split_whitespace()
            .take_while(|w| !w.starts_with('<') && !w.starts_with('['))
            .collect();
        let fits = literal.len() <= words.len() && literal.iter().zip(&words).all(|(a, b)| *a == b);
        if fits && best.is_none_or(|(n, _)| literal.len() > n) {
            best = Some((literal.len(), u));
        }
    }
    let (_, u) = best?;
    Some((u.permission.clone()?, u.level))
}

/// Runs a player's command line after the permission check of [`needs`]; `allowed(node, default_level)`
/// answers for the player.
pub fn run_checked(
    engine: &mut Engine,
    uuid: Option<u128>,
    line: &str,
    allowed: impl Fn(&str, u8) -> bool,
) -> Answer {
    if let Some((node, level)) = needs(line)
        && !allowed(&node, level)
    {
        return Answer::done(vec![engine.prefixed("command_permission", &[])]);
    }
    run(engine, uuid, line)
}

/// Runs a command line (without its slash). `uuid` is the player who ran it (None for the console); player
/// controls need one.
pub fn run(engine: &mut Engine, uuid: Option<u128>, line: &str) -> Answer {
    let words: Vec<&str> = line.split_whitespace().collect();
    let Some(first) = words.first() else {
        return Answer::none();
    };
    let args = &words[1..];
    match first.to_ascii_lowercase().as_str() {
        "radio" => radio_cmd(engine, uuid, args),
        "worldradio" | "wr" => kind_cmd(engine, Kind::World, args),
        "serverradio" | "sr" => kind_cmd(engine, Kind::Server, args),
        "positionradio" | "pr" => kind_cmd(engine, Kind::Position, args),
        "noteblockmusicplayer" | "nbmp" => nbmp_cmd(engine, args),
        _ => Answer::none(),
    }
}

/// Tab completion for a command line (without its slash; the last word is the one being typed, possibly empty):
/// the sub-commands, the radio names, the playlist names and the song files, filtered by the typed prefix.
pub fn complete(engine: &Engine, line: &str) -> Vec<String> {
    let words: Vec<&str> = line.split(' ').collect();
    let Some(current) = words.last().map(|w| w.to_ascii_lowercase()) else {
        return Vec::new();
    };
    let index = words.len() - 1;
    let root = words[0].to_ascii_lowercase();
    let sub = |n: usize| words.get(n).map(|w| w.to_ascii_lowercase()).unwrap_or_default();
    let candidates: Vec<String> = match (root.as_str(), index) {
        ("radio", 1) => ["help", "toggle", "volume"].map(String::from).to_vec(),
        ("worldradio" | "wr" | "serverradio" | "sr" | "positionradio" | "pr", 1) => {
            ["help", "enable", "disable", "skip", "reload"].map(String::from).to_vec()
        }
        ("worldradio" | "wr", 2) if ["enable", "disable", "skip"].contains(&sub(1).as_str()) => {
            engine.radio_names(Kind::World)
        }
        ("serverradio" | "sr", 2) if ["enable", "disable", "skip"].contains(&sub(1).as_str()) => {
            engine.radio_names(Kind::Server)
        }
        ("positionradio" | "pr", 2) if ["enable", "disable", "skip"].contains(&sub(1).as_str()) => {
            engine.radio_names(Kind::Position)
        }
        ("noteblockmusicplayer" | "nbmp", 1) => {
            ["help", "reload", "playlist", "hologram"].map(String::from).to_vec()
        }
        ("noteblockmusicplayer" | "nbmp", 2) if sub(1) == "playlist" => {
            ["help", "list", "songs", "create", "add", "remove", "reload"].map(String::from).to_vec()
        }
        ("noteblockmusicplayer" | "nbmp", 2) if sub(1) == "hologram" => {
            ["reload", "list"].map(String::from).to_vec()
        }
        ("noteblockmusicplayer" | "nbmp", 3)
            if sub(1) == "playlist" && ["songs", "add", "remove"].contains(&sub(2).as_str()) =>
        {
            engine.playlist_names()
        }
        ("noteblockmusicplayer" | "nbmp", 4) if sub(1) == "playlist" && sub(2) == "add" => {
            engine.song_files()
        }
        ("noteblockmusicplayer" | "nbmp", 4) if sub(1) == "playlist" && sub(2) == "remove" => {
            engine.playlist_entries(words[3]).unwrap_or_default()
        }
        _ => Vec::new(),
    };
    let mut out: Vec<String> = candidates
        .into_iter()
        .filter(|c| c.to_ascii_lowercase().starts_with(&current))
        .collect();
    out.sort();
    out.dedup();
    out
}

fn radio_cmd(engine: &mut Engine, uuid: Option<u128>, args: &[&str]) -> Answer {
    match args.first().copied() {
        Some("toggle") => match uuid {
            Some(player) => {
                engine.toggle_mute(player);
                Answer::done(Vec::new())
            }
            None => Answer::none(),
        },
        Some("volume") => {
            let Some(player) = uuid else {
                return Answer::none();
            };
            match args.get(1).copied() {
                None => {
                    let volume = engine.player_volume(player).to_string();
                    Answer::done(vec![
                        engine.prefixed("command_radio_volume_actual", &[("volume", volume)]),
                    ])
                }
                Some(text) => match text.parse::<u8>() {
                    Ok(level) if level <= 100 => {
                        engine.set_volume(player, level);
                        Answer::done(vec![
                            engine
                                .prefixed("command_radio_volume", &[("volume", level.to_string())]),
                        ])
                    }
                    _ => Answer::done(radio_help()),
                },
            }
        }
        _ => Answer::done(radio_help()),
    }
}

fn kind_cmd(engine: &mut Engine, kind: Kind, args: &[&str]) -> Answer {
    let word = kind.word().to_string();
    let sub = args.first().copied().unwrap_or("");
    let name = args.get(1).copied();
    let lines = match (sub, name) {
        ("enable", Some(name)) => vec![engine.set_state(kind, name, true)],
        ("disable", Some(name)) => vec![engine.set_state(kind, name, false)],
        ("skip", Some(name)) => vec![engine.skip(kind, name)],
        ("enable", None) | ("disable", None) => {
            vec![engine.prefixed("command_state_hint", &[("radio_type", word)])]
        }
        ("skip", None) => vec![engine.prefixed("command_skip_hint", &[("radio_type", word)])],
        ("reload", _) => vec![engine.reload_kind(kind)],
        _ => kind_help(kind),
    };
    Answer::done(lines)
}

fn nbmp_cmd(engine: &mut Engine, args: &[&str]) -> Answer {
    let lines = match args.first().copied() {
        None => vec![engine.prefixed("command_about", &[])],
        Some("help") => nbmp_help(),
        Some("reload") => vec![engine.reload_all()],
        Some("playlist") => playlist_cmd(engine, &args[1..]),
        Some("hologram") => match args.get(1).copied() {
            Some("reload") | Some("list") => vec![engine.prefixed("command_hologram_disable", &[])],
            _ => nbmp_help(),
        },
        Some(_) => nbmp_help(),
    };
    Answer::done(lines)
}

fn playlist_cmd(engine: &mut Engine, args: &[&str]) -> Vec<String> {
    let first = args.get(1).copied();
    let second = args.get(2).copied();
    match args.first().copied() {
        None | Some("help") => playlist_help(),
        Some("list") => {
            let mut lines = vec![engine.prefixed("command_playlist_list", &[])];
            lines.extend(
                engine
                    .playlist_names()
                    .into_iter()
                    .map(|name| format!("- {name}")),
            );
            lines
        }
        Some("songs") => match first {
            None => vec![engine.prefixed("command_playlist_songs_hint", &[])],
            Some(name) => match engine.playlist_entries(name) {
                None => vec![engine.prefixed(
                    "command_playlist_notfound",
                    &[("playlist", name.to_string())],
                )],
                Some(songs) => {
                    let mut lines = vec![
                        engine
                            .prefixed("command_playlist_songs", &[("playlist", name.to_string())]),
                    ];
                    lines.extend(songs.into_iter().map(|song| format!("- {song}")));
                    lines
                }
            },
        },
        Some("create") => match first {
            None => vec![engine.prefixed("command_playlist_create_hint", &[])],
            Some(name) => vec![engine.playlist_create(name)],
        },
        Some("add") => match (first, second) {
            (Some(playlist), Some(song)) => vec![engine.playlist_add(playlist, song)],
            _ => vec![engine.prefixed("command_playlist_add_hint", &[])],
        },
        Some("remove") => match (first, second) {
            (Some(playlist), Some(song)) => vec![engine.playlist_remove(playlist, song)],
            _ => vec![engine.prefixed("command_playlist_remove_hint", &[])],
        },
        Some("reload") => vec![engine.reload_playlists()],
        Some(_) => playlist_help(),
    }
}

fn radio_help() -> Vec<String> {
    vec![
        "Commands for player controls:".to_string(),
        "/radio help - show help for player controls".to_string(),
        "/radio toggle - mute or unmute all radios for you".to_string(),
        "/radio volume <0-100> - set volume of all radios for you".to_string(),
    ]
}

fn kind_help(kind: Kind) -> Vec<String> {
    let cmd = format!("/{}", kind.command());
    let t = kind.word();
    let heading = match kind {
        Kind::World => "World Radios",
        Kind::Server => "Server Radios",
        Kind::Position => "Position Radios",
    };
    vec![
        format!("Commands for {heading}:"),
        format!("{cmd} help - show help for {t} radios"),
        format!("{cmd} <enable/disable> <name of radio> - enable/disable given radio"),
        format!("{cmd} skip <name of radio> - skip actual song in radio"),
        format!("{cmd} reload - turn off and reloads from config all {t} radios"),
    ]
}

fn nbmp_help() -> Vec<String> {
    vec![
        "Commands for NoteBlockMusicPlayer plugin:".to_string(),
        "/nbmp help - shows this help".to_string(),
        "/nbmp reload - reloads the configuration".to_string(),
        "/nbmp playlist help - show help for playlists".to_string(),
        "/nbmp hologram reload - reloads the holograms".to_string(),
    ]
}

fn playlist_help() -> Vec<String> {
    vec![
        "Commands for Playlists:".to_string(),
        "/nbmp playlist help - show help for playlists".to_string(),
        "/nbmp playlist list - shows list of playlists".to_string(),
        "/nbmp playlist songs <name of playlist> - shows list of songs in playlist".to_string(),
        "/nbmp playlist create <name of playlist> - creates new playlist".to_string(),
        "/nbmp playlist add <name of playlist> <name of song> - add song to playlist".to_string(),
        "/nbmp playlist remove <name of playlist> <name of song> - remove song from playlist"
            .to_string(),
        "/nbmp playlist reload - reload all playlists from config".to_string(),
    ]
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
