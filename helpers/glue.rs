// Support module `crate::game::noteblock_glue` (horizon-client, beside the game's own modules): keeps the
// NoteBlockMusicPlayer core's state, feeds it the players each tick and sends what it answers. A child of `game`,
// so the game's private fields and its `pub(super)` helpers are in reach.
use super::Game;
use horizon_client::noteblock::{Answer, Engine, Out, PlayerView, WorldView};
use horizon_client::text::Text;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

/// The plugin folder, under the server folder (the engine's working directory).
const FOLDER: &str = "plugins/NoteBlockMusicPlayer";

static STATE: Mutex<Option<Engine>> = Mutex::new(None);

fn seed() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
}

/// The level's name (the world folder's name), which the radios' worlds are compared with.
fn level_name(game: &Game<'_>) -> String {
    game.native_world_dir()
        .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "world".to_owned())
}

/// The engine, loaded on first use (the level's name is known from the game then).
fn guarded(game: &Game<'_>) -> MutexGuard<'static, Option<Engine>> {
    let mut slot = STATE.lock().unwrap_or_else(|p| p.into_inner());
    if slot.is_none() {
        match Game::native_config_dir() {
            Ok(config) => match config.parent() {
                Some(server) => {
                    let dir = server.join(FOLDER);
                    *slot = Some(Engine::load(&dir, vec![level_name(game)], seed()));
                }
                None => eprintln!("[noteblock] no server folder next to the config folder"),
            },
            Err(e) => eprintln!("[noteblock] no config folder: {e}"),
        }
    }
    slot
}

fn flush(engine: &mut Engine) {
    for line in engine.take_log() {
        eprintln!("[noteblock] {line}");
    }
}

/// Every tick: the radios' notes and action bar lines go out to the players they reach.
pub(crate) fn nb_tick(game: &mut Game<'_>) {
    let level = level_name(game);
    let players = game.native_server_players();
    let outs: Vec<Out> = {
        let mut slot = guarded(game);
        let Some(engine) = slot.as_mut() else {
            return;
        };
        let view = WorldView {
            level,
            players: players
                .iter()
                .map(|p| PlayerView { uuid: p.uuid, dimension: p.dimension.clone(), pos: p.position })
                .collect(),
        };
        let outs = engine.tick(&view);
        flush(engine);
        outs
    };
    for out in outs {
        match out {
            Out::Sound { uuid, sound, volume, pitch } => {
                // NoteBlockAPI's MonoMode: the note is played inside the listener's head (`at` None: where they stand).
                let _ = game.native_play_sound_to(uuid, sound, "master", None, volume, pitch);
            }
            Out::ActionBar { uuid, line } => {
                let _ = game.native_action_bar(uuid, Text::legacy(&line));
            }
        }
    }
}

/// The command usages of this mod (every root and alias, with the permission nodes of `plugin.yml`).
pub(crate) fn nb_register(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    // Open to everyone, as Bukkit registers plugin.yml commands without a `permission:`; the
    // permission is checked when a command runs (`nb_perform`).
    for usage in horizon_client::noteblock::usages() {
        commands.push(horizon_client::mod_api::NativeCommand::new(&usage.usage));
    }
}

/// A player's command of this mod: its answer lines go back as the command's feedback.
pub(crate) fn nb_perform(game: &mut Game<'_>, player: u128, command: &str, ran: &mut bool) {
    if *ran {
        return;
    }
    let answer: Answer = {
        let mut slot = guarded(game);
        let Some(engine) = slot.as_mut() else {
            return;
        };
        let answer = engine.command_checked(Some(player), command, |node, level| {
            game.native_has_permission(player, node, level)
        });
        flush(engine);
        answer
    };
    if !answer.ran {
        return;
    }
    *ran = true;
    for line in answer.lines {
        game.native_feedback(Text::legacy(&line));
    }
}
