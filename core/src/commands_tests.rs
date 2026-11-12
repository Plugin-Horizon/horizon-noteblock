//! Command tests: the usage tree (roots, aliases, permission nodes) and what the player controls answer.

use super::*;
use std::path::PathBuf;

fn engine() -> Engine {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join("nbmp");
    Engine::load(&dir, vec!["fixture_world".to_string()], 1)
}

#[test]
fn every_root_and_alias_is_registered() {
    let all = usages();
    for root in [
        "radio",
        "worldradio",
        "wr",
        "serverradio",
        "sr",
        "positionradio",
        "pr",
        "noteblockmusicplayer",
        "nbmp",
    ] {
        assert!(
            all.iter().any(|u| u.usage == root),
            "bare {root} is missing"
        );
    }
    assert!(
        all.iter()
            .any(|u| u.usage == "worldradio enable <name:word>")
    );
    assert!(
        all.iter()
            .any(|u| u.usage == "nbmp playlist add <playlist:word> <song:word>")
    );
}

#[test]
fn permission_nodes_follow_plugin_yml() {
    let all = usages();
    let node = |text: &str| {
        all.iter()
            .find(|u| u.usage == text)
            .and_then(|u| u.permission.clone())
    };
    assert_eq!(
        node("worldradio enable <name:word>").as_deref(),
        Some("worldradio.enable")
    );
    assert_eq!(
        node("sr skip <name:word>").as_deref(),
        Some("serverradio.skip")
    );
    assert_eq!(
        node("positionradio reload").as_deref(),
        Some("positionradio.admin")
    );
    assert_eq!(node("radio toggle").as_deref(), Some("radio.toggle"));
    assert_eq!(
        node("radio volume [level:integer]").as_deref(),
        Some("radio.volume")
    );
    assert_eq!(
        node("nbmp playlist create <playlist:word>").as_deref(),
        Some("nbmp.playlist.create")
    );
    // Operators by default (plugin.yml `default: op`), players for the radio controls.
    let op = all
        .iter()
        .find(|u| u.usage == "wr enable <name:word>")
        .expect("usage");
    assert_eq!(op.level, 1);
    let player = all
        .iter()
        .find(|u| u.usage == "radio toggle")
        .expect("usage");
    assert_eq!(player.level, 0);
}

#[test]
fn other_roots_are_not_ours() {
    let mut e = engine();
    assert_eq!(run(&mut e, Some(1), "say hello"), Answer::none());
    assert_eq!(run(&mut e, Some(1), "   "), Answer::none());
}

#[test]
fn volume_needs_a_player_and_a_level_from_0_to_100() {
    let mut e = engine();
    assert!(
        !run(&mut e, None, "radio volume 40").ran,
        "the console has no volume"
    );
    let set = run(&mut e, Some(7), "radio volume 40");
    assert!(set.ran);
    assert!(set.lines[0].contains("set to 40"), "{:?}", set.lines);
    let shown = run(&mut e, Some(7), "radio volume");
    assert!(shown.lines[0].contains("is 40"), "{:?}", shown.lines);
    let out_of_range = run(&mut e, Some(7), "radio volume 150");
    assert!(out_of_range.ran);
    assert_eq!(out_of_range.lines[0], "Commands for player controls:");
    assert_eq!(e.player_volume(7), 40, "a refused level changes nothing");
}

#[test]
fn toggle_mutes_the_player_for_every_radio() {
    let mut e = engine();
    assert!(run(&mut e, Some(9), "radio toggle").ran);
    assert!(e.is_muted(9));
    assert!(run(&mut e, Some(9), "radio toggle").ran);
    assert!(!e.is_muted(9));
    assert!(!run(&mut e, None, "radio toggle").ran);
}

#[test]
fn bare_nbmp_and_help_answer_with_text() {
    let mut e = engine();
    let about = run(&mut e, None, "nbmp");
    assert!(about.ran);
    assert!(
        about.lines[0].contains("/noteblockmusicplayer help"),
        "{:?}",
        about.lines
    );
    let help = run(&mut e, None, "noteblockmusicplayer help");
    assert_eq!(help.lines[0], "Commands for NoteBlockMusicPlayer plugin:");
    let playlist = run(&mut e, None, "nbmp playlist list");
    assert!(playlist.lines[0].contains("Playlists on server"));
    assert!(
        playlist.lines.iter().any(|l| l == "- fixture_playlist"),
        "{:?}",
        playlist.lines
    );
}

#[test]
fn hologram_commands_say_holograms_are_off() {
    let mut e = engine();
    let reply = run(&mut e, None, "nbmp hologram list");
    assert!(
        reply.lines[0].contains("not enabled on this server"),
        "{:?}",
        reply.lines
    );
}

#[test]
fn kind_help_names_the_kind() {
    let mut e = engine();
    let reply = run(&mut e, None, "positionradio");
    assert!(reply.ran);
    assert_eq!(reply.lines[0], "Commands for Position Radios:");
    assert!(
        reply
            .lines
            .iter()
            .any(|l| l == "/positionradio skip <name of radio> - skip actual song in radio")
    );
}

#[test]
fn skip_without_a_running_radio_says_so() {
    let mut e = engine();
    let reply = run(&mut e, None, "worldradio skip world_radio");
    assert!(
        reply.lines[0].contains("is not running"),
        "{:?}",
        reply.lines
    );
}

/// On a lobby server: Bukkit shows NoteBlockMusicPlayer's commands to everyone; a player without the node gets
/// `command_permission` (messages_en.yml), `/radio` stays open (radio.toggle/radio.volume default true).
#[test]
fn a_player_without_the_node_is_told_so() {
    let mut e = engine();
    let deny = |_: &str, level: u8| level == 0;
    let a = e.command_checked(Some(7), "nbmp", deny);
    assert!(a.ran);
    assert_eq!(a.lines.len(), 1);
    assert!(a.lines[0].contains("You are not allowed to use this command!"), "{:?}", a.lines);
    let a = e.command_checked(Some(7), "wr enable world", deny);
    assert!(a.lines[0].contains("You are not allowed"), "{:?}", a.lines);
    let a = e.command_checked(Some(7), "radio", deny);
    assert!(a.lines.iter().any(|l| l.contains("/radio toggle")), "{:?}", a.lines);
    assert_eq!(crate::commands::needs("wr enable world"), Some(("worldradio.enable".into(), 1)));
    assert_eq!(crate::commands::needs("nbmp playlist list"), Some(("nbmp.playlist.list".into(), 1)));
    assert_eq!(crate::commands::needs("radio toggle"), Some(("radio.toggle".into(), 0)));
    let a = e.command_checked(Some(7), "nbmp", |_, _| true);
    assert!(a.lines[0].contains("/noteblockmusicplayer help"), "{:?}", a.lines);
}

