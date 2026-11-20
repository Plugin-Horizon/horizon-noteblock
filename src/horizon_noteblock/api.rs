//! NoteBlockAPI's public API as plain Rust functions, for the other code of this binary (the glue, other modules).
//!
//! A radio is named by its kind and name, as in the commands, instead of a `SongPlayer` object. The functions wrap
//! `Engine`; the song events come from `take_song_events` (`SongEvent`).

use crate::horizon_noteblock::engine::{Engine, SongEvent};
use crate::horizon_noteblock::radio::Kind;

/// `NoteBlockAPI.getPlayerVolume`: the player's own volume, 0 to 100.
pub fn player_volume(engine: &Engine, uuid: u128) -> u8 {
    engine.player_volume(uuid)
}

/// `NoteBlockAPI.setPlayerVolume`: the volume is capped at 100.
pub fn set_player_volume(engine: &mut Engine, uuid: u128, volume: u8) {
    engine.set_volume(uuid, volume);
}

/// Whether a radio of this kind and name is configured.
pub fn has_radio(engine: &Engine, kind: Kind, name: &str) -> bool {
    engine.find(kind, name).is_some()
}

/// Switches a radio on or off, as `/worldradio enable|disable` does. True when the radio ends in the asked state (a
/// radio that is not loaded, or has several songs, stays off and gives false).
pub fn set_playing(engine: &mut Engine, kind: Kind, name: &str, on: bool) -> bool {
    let _ = engine.set_state(kind, name, on);
    engine.is_enabled(kind, name) == on
}

/// The song a radio plays now, if any.
pub fn current_song(engine: &Engine, kind: Kind, name: &str) -> Option<String> {
    engine.current_song(kind, name)
}

/// Ends the radio's current song, as `skip` does. Returns the song that was skipped, if one played.
pub fn skip_song(engine: &mut Engine, kind: Kind, name: &str) -> Option<String> {
    let song = engine.current_song(kind, name)?;
    let _ = engine.skip(kind, name);
    Some(song)
}

/// The song events since the last call, in order (`SongStartedEvent`, `SongEndedEvent`, `SongStoppedEvent`).
pub fn take_events(engine: &mut Engine) -> Vec<SongEvent> {
    engine.take_song_events()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn engine() -> Engine {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("noteblock")
            .join("testdata")
            .join("nbmp");
        Engine::load(&dir, vec!["fixture_world".to_string()], 1)
    }

    #[test]
    fn an_unknown_radio_is_not_playing_and_skips_nothing() {
        let mut engine = engine();
        assert!(!has_radio(&engine, Kind::World, "no_such_radio"));
        assert!(!set_playing(&mut engine, Kind::World, "no_such_radio", true));
        assert_eq!(current_song(&engine, Kind::World, "no_such_radio"), None);
        assert_eq!(skip_song(&mut engine, Kind::World, "no_such_radio"), None);
        assert!(take_events(&mut engine).is_empty());
    }

    #[test]
    fn player_volume_is_capped_at_100() {
        let mut engine = engine();
        set_player_volume(&mut engine, 7, 250);
        assert_eq!(player_volume(&engine, 7), 100);
        set_player_volume(&mut engine, 7, 40);
        assert_eq!(player_volume(&engine, 7), 40);
    }
}
