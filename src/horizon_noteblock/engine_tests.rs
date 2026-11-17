//! Engine tests: the fixture folder (every radio off, nothing plays), radios built in code, sounds and action bar,
//! the switches and checks of the commands, and playlists saved to a copy of the folder.

use super::*;
use crate::horizon_noteblock::nbs::testutil::{FileSpec, note};
use crate::horizon_noteblock::radio::{Kind, RadioSpec};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data").join("noteblock").join("testdata")
        .join("nbmp")
}

fn world_view(players: &[(u128, &str, [f64; 3])]) -> WorldView {
    WorldView {
        level: "fixture_world".to_string(),
        players: players
            .iter()
            .map(|(uuid, dimension, pos)| PlayerView {
                uuid: *uuid,
                dimension: dimension.to_string(),
                pos: *pos,
            })
            .collect(),
    }
}

/// A song of `length` ticks, one note per tick (layer 0, instrument 0, key 45 = base pitch), 10 ticks per second.
fn tone(length: i32) -> crate::horizon_noteblock::nbs::Song {
    let notes: Vec<_> = (0..=length).map(|t| note(t, 0, 0, 45)).collect();
    let spec = FileSpec::new(0, length as u16, 1, 1000, notes);
    crate::horizon_noteblock::nbs::parse(&spec.bytes()).expect("synthetic song parses")
}

fn radio(kind: Kind, name: &str, songs: &[&str], volume: u8) -> RadioSpec {
    RadioSpec {
        kind,
        name: name.to_string(),
        world: "fixture_world".to_string(),
        center: [0.0, 100.0, 0.0],
        radius: 5.0,
        playlist: "fixture_playlist".to_string(),
        songs: songs.iter().map(|s| s.to_string()).collect(),
        repeat: true,
        random: false,
        actionbar: true,
        delay_secs: 0.0,
        volume,
    }
}

fn sounds_to(outs: &[Out], uuid: u128) -> Vec<(f32, f32, &'static str)> {
    outs.iter()
        .filter_map(|o| match o {
            Out::Sound {
                uuid: u,
                sound,
                volume,
                pitch,
            } if *u == uuid => Some((*volume, *pitch, *sound)),
            _ => None,
        })
        .collect()
}

fn copy_fixture_to(dir: &Path) {
    std::fs::create_dir_all(dir.join("songs")).expect("temp folder");
    for entry in std::fs::read_dir(fixture_dir()).expect("fixture folder") {
        let entry = entry.expect("entry");
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            std::fs::copy(entry.path(), dir.join(entry.file_name())).expect("copy fixture file");
        }
    }
}

#[test]
fn fixture_flags_start_nothing() {
    let mut engine = Engine::load(&fixture_dir(), vec!["fixture_world".to_string()], 1);
    let states = engine.radio_states();
    assert_eq!(states.len(), 3, "{states:?}");
    assert!(states.values().all(|on| !*on), "{states:?}");
    let view = world_view(&[(1, "minecraft:overworld", [0.0, 100.0, 0.0])]);
    let mut total = 0;
    for _ in 0..400 {
        total += engine.tick(&view).len();
    }
    assert_eq!(total, 0, "a radio that is off sends nothing");
}

#[test]
fn position_radio_of_the_fixture_is_refused_when_enabled() {
    let mut engine = Engine::load(&fixture_dir(), vec!["fixture_world".to_string()], 1);
    let reply = engine.command(Some(1), "positionradio enable test_radio");
    assert!(reply.ran);
    assert!(
        reply.lines[0].contains("could not be loaded"),
        "{:?}",
        reply.lines
    );
    assert!(!engine.is_enabled(Kind::Position, "test_radio"));
}

#[test]
fn file_radios_with_missing_songs_are_not_loaded() {
    let engine = Engine::load(&fixture_dir(), vec!["fixture_world".to_string()], 1);
    // The fixture's playlist names songs that have no file: the radio has no songs.
    let problem = engine.problem(0);
    assert!(matches!(problem, Some(("radio_nosongs", _))), "{problem:?}");
    assert!(!engine.radios[0].loaded);
}

#[test]
fn world_radio_plays_to_players_of_its_world_only() {
    let mut engine = Engine::load(&fixture_dir(), vec!["fixture_world".to_string()], 1);
    engine.library_mut().insert("tone_one", tone(3));
    engine.add_radio(radio(Kind::World, "main", &["tone_one"], 100));
    assert!(engine.command(None, "worldradio enable main").lines[0].contains("was set to enable"));
    assert!(engine.command(Some(3), "radio toggle").ran);

    let view = world_view(&[
        (1, "minecraft:overworld", [0.0, 100.0, 0.0]),
        (2, "minecraft:the_nether", [0.0, 100.0, 0.0]),
        (3, "minecraft:overworld", [0.0, 100.0, 0.0]),
    ]);
    let mut per_tick = Vec::new();
    for _ in 0..8 {
        per_tick.push(engine.tick(&view));
    }
    // The first tick starts the song (its action bar goes to player 1 alone) and plays song tick 0.
    let bars: Vec<_> = per_tick[0]
        .iter()
        .filter(|o| matches!(o, Out::ActionBar { .. }))
        .cloned()
        .collect();
    assert_eq!(
        bars,
        vec![Out::ActionBar {
            uuid: 1,
            line: "You are now listening Test".to_string()
        }]
    );
    // Song ticks at server ticks 0, 2, 4, 6 (10 per second); player 1 hears each, not the nether or the muted player.
    let heard: Vec<bool> = per_tick
        .iter()
        .map(|outs| !sounds_to(outs, 1).is_empty())
        .collect();
    assert_eq!(
        heard,
        vec![true, false, true, false, true, false, true, false]
    );
    assert!(per_tick.iter().all(|outs| sounds_to(outs, 2).is_empty()));
    assert!(per_tick.iter().all(|outs| sounds_to(outs, 3).is_empty()));
    let (volume, pitch, sound) = sounds_to(&per_tick[0], 1)[0];
    assert!((volume - 1.0).abs() < 1e-6);
    assert!((pitch - 1.0).abs() < 1e-6, "key 45 is the base pitch");
    assert_eq!(sound, "minecraft:block.note_block.harp");
}

#[test]
fn player_volume_and_radio_volume_scale_the_sound() {
    let mut engine = Engine::load(&fixture_dir(), vec!["fixture_world".to_string()], 1);
    engine.library_mut().insert("tone_two", tone(1));
    engine.add_radio(radio(Kind::Server, "everyone", &["tone_two"], 80));
    assert!(
        engine.command(None, "serverradio enable everyone").lines[0].contains("was set to enable")
    );
    let answer = engine.command(Some(1), "radio volume 50");
    assert!(answer.ran);
    assert!(answer.lines[0].contains("50"));
    let view = world_view(&[(1, "minecraft:overworld", [0.0, 0.0, 0.0])]);
    let outs = engine.tick(&view);
    let (volume, _, _) = sounds_to(&outs, 1)[0];
    // layer 100 * radio 80 * player 50 * velocity 100 / 1e8 = 0.4
    assert!((volume - 0.4).abs() < 1e-6, "{volume}");
}

#[test]
fn position_radio_reaches_players_within_its_radius() {
    let mut engine = Engine::load(&fixture_dir(), vec!["fixture_world".to_string()], 1);
    engine.library_mut().insert("tone_three", tone(1));
    engine.add_radio(radio(Kind::Position, "spot", &["tone_three"], 100));
    assert!(
        engine.command(None, "positionradio enable spot").lines[0].contains("was set to enable")
    );
    let view = world_view(&[
        (1, "minecraft:overworld", [1.0, 100.0, 1.0]),
        (2, "minecraft:overworld", [9.0, 100.0, 0.0]),
    ]);
    let outs = engine.tick(&view);
    assert_eq!(sounds_to(&outs, 1).len(), 1, "inside the radius of 5");
    assert!(sounds_to(&outs, 2).is_empty(), "outside it");
}

#[test]
fn a_second_world_radio_in_the_same_world_is_refused() {
    let mut engine = Engine::load(&fixture_dir(), vec!["fixture_world".to_string()], 1);
    engine.library_mut().insert("tone_four", tone(1));
    engine.add_radio(radio(Kind::World, "first", &["tone_four"], 100));
    engine.add_radio(radio(Kind::World, "second", &["tone_four"], 100));
    assert!(engine.command(None, "worldradio enable first").lines[0].contains("was set to enable"));
    let refused = engine.command(None, "worldradio enable second");
    assert!(
        refused.lines[0].contains("already one world radio"),
        "{:?}",
        refused.lines
    );
    assert!(!engine.is_enabled(Kind::World, "second"));
}

#[test]
fn unknown_radio_and_bad_arguments_get_their_replies() {
    let mut engine = Engine::load(&fixture_dir(), vec!["fixture_world".to_string()], 1);
    assert!(engine.command(None, "worldradio enable nobody").lines[0].contains("was not found"));
    assert!(engine.command(None, "worldradio enable").lines[0].contains("Use /worldradio"));
    assert!(engine.command(None, "nbmp playlist songs nothing").lines[0].contains("was not found"));
}

#[test]
fn skip_ends_the_song_of_a_running_radio() {
    let mut engine = Engine::load(&fixture_dir(), vec!["fixture_world".to_string()], 1);
    engine.library_mut().insert("tone_five", tone(50));
    engine.library_mut().insert("tone_six", tone(1));
    engine.add_radio(radio(
        Kind::Server,
        "skippy",
        &["tone_five", "tone_six"],
        100,
    ));
    assert!(engine.command(None, "serverradio enable skippy").ran);
    let view = world_view(&[(1, "minecraft:overworld", [0.0, 0.0, 0.0])]);
    let _ = engine.tick(&view);
    let reply = engine.command(None, "serverradio skip skippy");
    assert!(reply.lines[0].contains("was skipped"), "{:?}", reply.lines);
    let mut bars = 0;
    for _ in 0..4 {
        bars += engine
            .tick(&view)
            .iter()
            .filter(|o| matches!(o, Out::ActionBar { .. }))
            .count();
    }
    assert_eq!(bars, 1, "the next song starts");
}

#[test]
fn playlist_add_and_remove_are_saved_in_playlists_yml() {
    let dir = std::env::temp_dir().join(format!(
        "horizon-noteblock-save-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    copy_fixture_to(&dir);
    let spec = FileSpec::new(0, 1, 1, 1000, vec![note(0, 0, 0, 45)]);
    std::fs::write(dir.join("songs").join("tone_new.nbs"), spec.bytes()).expect("write song");

    let mut engine = Engine::load(&dir, vec!["fixture_world".to_string()], 1);
    let added = engine.command(Some(1), "nbmp playlist add fixture_playlist tone_new");
    assert!(
        added.lines[0].contains("was added to playlist"),
        "{:?}",
        added.lines
    );
    let created = engine.command(Some(1), "nbmp playlist create extra_list");
    assert!(created.lines[0].contains("was created"));

    let saved = std::fs::read_to_string(dir.join("playlists.yml")).expect("playlists.yml written");
    let doc = yaml::parse_document(&saved).expect("saved file parses");
    let list = doc
        .root
        .path("playlist.fixture_playlist")
        .expect("playlist kept")
        .str_list();
    assert!(list.contains(&"tone_new".to_string()), "{list:?}");
    assert!(doc.root.path("playlist.extra_list").is_some());

    let removed = engine.command(Some(1), "nbmp playlist remove fixture_playlist tone_new");
    assert!(removed.lines[0].contains("was removed"));
    let reloaded =
        yaml::parse_document(&std::fs::read_to_string(dir.join("playlists.yml")).expect("file"))
            .expect("parses");
    let list = reloaded
        .root
        .path("playlist.fixture_playlist")
        .expect("kept")
        .str_list();
    assert!(!list.contains(&"tone_new".to_string()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_song_missing_from_the_folder_cannot_be_added() {
    let dir = std::env::temp_dir().join(format!(
        "horizon-noteblock-missing-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    copy_fixture_to(&dir);
    let mut engine = Engine::load(&dir, vec!["fixture_world".to_string()], 1);
    let reply = engine.command(None, "nbmp playlist add fixture_playlist not_here");
    assert!(
        reply.lines[0].contains("Use /nbmp playlist add"),
        "{:?}",
        reply.lines
    );
    let _ = std::fs::remove_dir_all(&dir);
}
