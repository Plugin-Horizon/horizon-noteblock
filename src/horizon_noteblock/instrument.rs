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
    fn negative_ids_play_the_harp_and_custom_ids_have_no_sound() {
        assert_eq!(sound_name(-1), Some("minecraft:block.note_block.harp"));
        assert_eq!(sound_name(20), None);
        assert!(is_custom(20));
        assert!(!is_custom(19));
        assert!(!is_custom(-5));
    }
}
