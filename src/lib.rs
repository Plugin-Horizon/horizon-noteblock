//! NoteBlockMusicPlayer 1.10.0 with NoteBlockAPI 1.7.0 as an arismoko native mod (trait mod on `mod_api`). The logic
//! is the unchanged `horizon-noteblock-core` crate (library `horizon_noteblock`); this file only wires it to hooks and
//! host methods.
//!
//! - `server_about_to_start`: reads `plugins/NoteBlockMusicPlayer/` (relative to the server folder, as the Java plugin
//!   does) for the level named in `server.properties`.
//! - `server_tick` (start of a tick): hands the players in, sends the note sounds and action bar lines it answers with.
//! - `register_commands` / `run_mod_command`: one usage per root (`radio`, `wr`, `sr`, `pr`, `nbmp`, ...) taking the
//!   rest of the line; the line goes through the core's command table with the permission check of the original.
use horizon_noteblock::commands;
use horizon_noteblock::engine::{Engine, Out, PlayerView, WorldView};
use mod_api::{
    Command, CommandArgumentValue, CommandSource, Flow, Hooks, RegisterCommandsEvent,
    RunModCommandEvent, ServerAboutToStartEvent, ServerHost, ServerTickEvent, ServerTickPhase,
    SoundSource, Text,
};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// The plugin folder, relative to the server folder (the game's working directory).
const FOLDER: &str = "plugins/NoteBlockMusicPlayer";

struct Loaded {
    /// The level's folder name, which the radios' worlds are compared with.
    level: String,
    engine: Engine,
}

#[derive(Default)]
struct Mod {
    loaded: Mutex<Option<Loaded>>,
}

fn log(line: &str) {
    eprintln!("[noteblock] {line}");
}

fn seed() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
}

/// `level-name` of `server.properties`, `world` when it is unset (Paper's default).
fn level_name() -> String {
    std::fs::read_to_string("server.properties")
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("level-name="))
                .map(|name| name.trim().to_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "world".to_owned())
}

/// The first word of every usage (`radio`, `wr`, `nbmp`, ...), each once.
/// Roots this mod leaves to another: `/sr` is SkinsRestorer's on the network (the proxy and every lobby
/// carry it, and on Paper the proxy's `/sr` answers first); `/serverradio` stays for the server radio.
const GIVEN_AWAY: [&str; 1] = ["sr"];

fn roots() -> Vec<String> {
    let mut roots: Vec<String> = commands::usages()
        .into_iter()
        .filter_map(|usage| usage.usage.split_whitespace().next().map(str::to_owned))
        .filter(|root| !GIVEN_AWAY.contains(&root.as_str()))
        .collect();
    roots.sort();
    roots.dedup();
    roots
}

/// Minecraft's colour code for `code`, if it names a colour.
fn colour(code: char) -> Option<&'static str> {
    Some(match code {
        '0' => "black",
        '1' => "dark_blue",
        '2' => "dark_green",
        '3' => "dark_aqua",
        '4' => "dark_red",
        '5' => "dark_purple",
        '6' => "gold",
        '7' => "gray",
        '8' => "dark_gray",
        '9' => "blue",
        'a' => "green",
        'b' => "aqua",
        'c' => "red",
        'd' => "light_purple",
        'e' => "yellow",
        'f' => "white",
        _ => return None,
    })
}

/// The colour and style a run of text has.
#[derive(Clone, Copy, Default)]
struct Run {
    color: Option<&'static str>,
    bold: bool,
    italic: bool,
    underlined: bool,
    strike: bool,
}

impl Run {
    fn piece(self, text: &str) -> Text {
        let mut piece = Text::literal(text);
        piece.style.color = self.color.map(str::to_owned);
        piece.style.bold = self.bold.then_some(true);
        piece.style.italic = self.italic.then_some(true);
        piece.style.underlined = self.underlined.then_some(true);
        piece.style.strikethrough = self.strike.then_some(true);
        piece
    }
}

/// A line with `§` codes as a component tree. A colour code clears the styles before it, as Minecraft does;
/// `§k` (obfuscated) is dropped.
fn styled(line: &str) -> Text {
    let mut root = Text::literal("");
    let mut run = Run::default();
    let mut text = String::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '§' {
            text.push(c);
            continue;
        }
        let Some(code) = chars.next() else { break };
        if !text.is_empty() {
            root = root.append(run.piece(&text));
            text.clear();
        }
        run = match code.to_ascii_lowercase() {
            'r' => Run::default(),
            'l' => Run { bold: true, ..run },
            'o' => Run { italic: true, ..run },
            'n' => Run { underlined: true, ..run },
            'm' => Run { strike: true, ..run },
            'k' => run,
            other => match colour(other) {
                Some(name) => Run { color: Some(name), ..Run::default() },
                None => run,
            },
        };
    }
    if !text.is_empty() {
        root = root.append(run.piece(&text));
    }
    root
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn server_about_to_start(&self, _: &mut dyn ServerHost, _: &mut ServerAboutToStartEvent) {
        let level = level_name();
        let mut engine = Engine::load(Path::new(FOLDER), vec![level.clone()], seed());
        for line in engine.take_log() {
            log(&line);
        }
        log(&format!("{FOLDER} read for level {level}"));
        *self.loaded.lock().unwrap_or_else(|e| e.into_inner()) = Some(Loaded { level, engine });
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        // Every root takes the rest of the line; the original's usages and levels are checked in `run_mod_command`.
        // This loses tab completion of the sub-commands (the server only sees `[args:text]`).
        for root in roots() {
            event.commands.push(Command::new(&format!("{root} [args:text]")));
        }
    }

    fn run_mod_command(&self, host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
        let words = &event.command.literals;
        let Some(root) = words.first() else {
            return Flow::Continue;
        };
        if !roots().contains(root) {
            return Flow::Continue;
        }
        let mut line = words.join(" ");
        if let Some(CommandArgumentValue::String(args)) = event.command.argument("args")
            && !args.is_empty()
        {
            line.push(' ');
            line.push_str(args);
        }
        let player = match event.source {
            CommandSource::Player { player, .. } => player,
            _ => {
                // Console use was never ported (the Java plugin's console path is not in the perform hook).
                let _ = host.commands().feedback(Text::literal(
                    "NoteBlockMusicPlayer commands are for players in this port.",
                ));
                return Flow::Cancel;
            }
        };
        let uuid = player.uuid();
        // Operator level of the player, for the permission nodes below.
        let op_level = player.permission_level(&*host).unwrap_or(0);
        let answer = {
            let mut slot = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
            let Some(loaded) = slot.as_mut() else {
                return Flow::Continue;
            };
            // The original asks each node (`radio.toggle`, `nbmp.admin`, ...) with `hasPermission`: a
            // permissions provider's answer decides; unset, the plugin.yml default does (everyone pass
            // for default 0, the others need permission level 2, the game master).
            let answer = loaded.engine.command_checked(Some(uuid), &line, |node, default_level| {
                player
                    .permission_value(&*host, node)
                    .unwrap_or(default_level == 0 || op_level >= 2)
            });
            for log_line in loaded.engine.take_log() {
                log(&log_line);
            }
            answer
        };
        if !answer.ran {
            return Flow::Continue;
        }
        for reply in answer.lines {
            let _ = host.commands().feedback(styled(&reply));
        }
        Flow::Cancel
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if !matches!(event.phase, ServerTickPhase::ServerTickStart { world_running: true, .. }) {
            return;
        }
        let players = host.players().server_players();
        let outs: Vec<Out> = {
            let mut slot = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
            let Some(loaded) = slot.as_mut() else {
                return;
            };
            let view = WorldView {
                level: loaded.level.clone(),
                players: players
                    .iter()
                    .filter_map(|p| {
                        let dimension = p.level(&*host)?.dimension_id().to_owned();
                        let pos = p.entity().position(&*host)?;
                        Some(PlayerView { uuid: p.uuid(), dimension, pos })
                    })
                    .collect(),
            };
            let outs = loaded.engine.tick(&view);
            for line in loaded.engine.take_log() {
                log(&line);
            }
            outs
        };
        for out in outs {
            match out {
                // NoteBlockAPI's MonoMode: the note is played where the listener stands, for them only.
                Out::Sound { uuid, sound, volume, pitch } => {
                    if let Some(p) = players.iter().find(|p| p.uuid() == uuid) {
                        p.play_sound(host, sound, SoundSource::Master, volume, pitch);
                    }
                }
                Out::ActionBar { uuid, line } => {
                    if let Some(p) = players.iter().find(|p| p.uuid() == uuid) {
                        let _ = p.action_bar(host, styled(&line));
                    }
                }
            }
        }
    }
}
