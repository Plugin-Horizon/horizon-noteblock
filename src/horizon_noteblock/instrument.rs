//! NoteBlockAPI's instrument ids (`utils/InstrumentUtils`) as the vanilla note block sound events.
//!
//! On Minecraft 26.x NoteBlockAPI 1.7.0 takes ids 0 to 19 as vanilla instruments (16 to 19 are the trumpets) and
//! starts custom instruments at 20. Custom instruments are read by the NBS reader but not played (no sound file).

/// The first custom instrument id on Minecraft 26.x (`getCustomInstrumentFirstIndex`).
pub const FIRST_CUSTOM: u8 = 20;

const SOUNDS: [&str; 20] = [
    "minecraft:block.note_block.harp",
    "minecraft:block.note_block.bass",
    "minecraft:block.note_block.basedrum",
    "minecraft:block.note_block.snare",
    "minecraft:block.note_block.hat",
    "minecraft:block.note_block.guitar",
    "minecraft:block.note_block.flute",
    "minecraft:block.note_block.bell",
    "minecraft:block.note_block.chime",
    "minecraft:block.note_block.xylophone",
    "minecraft:block.note_block.iron_xylophone",
    "minecraft:block.note_block.cow_bell",
    "minecraft:block.note_block.didgeridoo",
    "minecraft:block.note_block.bit",
    "minecraft:block.note_block.banjo",
    "minecraft:block.note_block.pling",
    "minecraft:block.note_block.trumpet",
    "minecraft:block.note_block.trumpet_exposed",
    "minecraft:block.note_block.trumpet_weathered",
    "minecraft:block.note_block.trumpet_oxidized",
];

/// The sound event of an instrument id. Negative ids play the harp, as NoteBlockAPI's default branch does;
/// `None` for custom instruments (ids from 20).
pub fn sound_name(instrument: i8) -> Option<&'static str> {
    if instrument < 0 {
        return Some(SOUNDS[0]);
    }
    SOUNDS.get(instrument as usize).copied()
}

/// The sound a custom instrument plays: its file name without `.ogg` (NoteBlockAPI's `CustomInstrument`), a bare
/// name in the `minecraft` namespace; `pling` is the vanilla pling.
pub fn custom_sound(file: &str) -> String {
    let name = file.replace(".ogg", "");
    if name.eq_ignore_ascii_case("pling") || name.eq_ignore_ascii_case("block.note_block.pling") {
        return "minecraft:block.note_block.pling".to_owned();
    }
    if name.contains(':') { name } else { format!("minecraft:{name}") }
}

/// `NoteUtils.isOutOfRange`: the key with its pitch applied is outside keys 33 to 56.
pub fn out_of_range(key: i32, pitch: i32) -> bool {
    let key = key + pitch / 100;
    !(33..57).contains(&key)
}

/// `InstrumentUtils.warpNameOutOfRange`: the suffix of the octave the key falls in.
pub fn warp_name(name: &str, key: i32, pitch: i32) -> String {
    let key = key + pitch / 100;
    let suffix = if key < 9 {
        "_-2"
    } else if key < 33 {
        "_-1"
    } else if key < 57 {
        ""
    } else if key < 81 {
        "_1"
    } else if key < 105 {
        "_2"
    } else {
        ""
    };
    format!("{name}{suffix}")
}

/// Whether the id is a custom instrument (NoteBlockAPI's `isCustomInstrument`).
pub fn is_custom(instrument: i8) -> bool {
    instrument >= FIRST_CUSTOM as i8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_ids_map_to_note_block_sounds() {
        assert_eq!(sound_name(0), Some("minecraft:block.note_block.harp"));
        assert_eq!(sound_name(1), Some("minecraft:block.note_block.bass"));
        assert_eq!(sound_name(6), Some("minecraft:block.note_block.flute"));
        assert_eq!(sound_name(15), Some("minecraft:block.note_block.pling"));
        assert_eq!(
            sound_name(19),
            Some("minecraft:block.note_block.trumpet_oxidized")
        );
    }

    #[test]
    fn custom_sounds_and_warped_names() {
        assert_eq!(custom_sound("drum.ogg"), "minecraft:drum");
        assert_eq!(custom_sound("Pling.ogg"), "minecraft:block.note_block.pling");
        assert_eq!(custom_sound("pack:kick"), "pack:kick");
        assert_eq!(warp_name("minecraft:block.note_block.harp", 20, 0), "minecraft:block.note_block.harp_-1");
        assert_eq!(warp_name("x", 45, 0), "x");
        assert_eq!(warp_name("x", 60, 0), "x_1");
        assert_eq!(warp_name("x", 4, 0), "x_-2");
        assert!(out_of_range(32, 0) && !out_of_range(33, 0) && !out_of_range(56, 0) && out_of_range(57, 0));
    }

    #[test]
    fn negative_ids_play_the_harp_and_custom_ids_have_no_sound() {
        assert_eq!(sound_name(-1), Some("minecraft:block.note_block.harp"));
        assert_eq!(sound_name(20), None);
        assert!(is_custom(20));
        assert!(!is_custom(19));
        assert!(!is_custom(-5));
    }
}
