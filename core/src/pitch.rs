//! NoteUtils' pitch table and formulas, and the volume of a note (NoteBlockMusicPlayer's radio players).
//!
//! The table has 2401 entries, `2^((i - 1200) / 1200)` for i in 0..2401: half to double, so a fine pitch of one
//! hundredth of a key is one step. A note's key times 100 plus its fine pitch is moved into the window 3300..=5700
//! (keys 33 to 57, two octaves) by whole octaves; that index minus 3300 is the table index.

use std::sync::OnceLock;

pub const TABLE_LEN: usize = 2401;

/// The 2401-entry pitch table (computed once).
pub fn table() -> &'static [f32] {
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            (0..TABLE_LEN)
                .map(|i| 2f64.powf((i as f64 - 1200.0) / 1200.0) as f32)
                .collect()
        })
        .as_slice()
}

/// NoteUtils.getPitchTransposed: the note moved into the two-octave window (what `use10OctaveRange: false` plays).
pub fn transposed(key: i32, pitch: i32) -> f32 {
    let mut p = pitch + key * 100;
    while p < 3300 {
        p += 1200;
    }
    while p > 5700 {
        p -= 1200;
    }
    table()[(p - 3300) as usize]
}

/// NoteUtils.getPitchInOctave: the note moved into the base octave (used with `use10OctaveRange: true`).
/// NoteBlockAPI's warped sound names for notes outside the window are not ported.
pub fn in_octave(key: i32, pitch: i32) -> f32 {
    let key = key + pitch / 100;
    let fine = pitch % 100;
    let key = if key < 9 {
        key + 15
    } else if key < 33 {
        key - 9
    } else if key < 57 {
        key - 33
    } else if key < 81 {
        key - 57
    } else if key < 105 {
        key - 81
    } else {
        key
    };
    let index = (key * 100 + fine).clamp(0, (TABLE_LEN - 1) as i32);
    table()[index as usize]
}

/// The volume of one note: layer volume times radio volume times the listener's volume times the note's
/// velocity, each 0 to 100, over 100 000 000 (NoteBlockAPI's formula; 1.0 at full volume).
pub fn volume(layer: u8, radio: u8, player: u8, velocity: u8) -> f32 {
    let product = layer as i32 * radio as i32 * player as i32 * velocity as i32;
    product as f32 / 100_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn table_has_2401_entries_from_half_to_double() {
        let t = table();
        assert_eq!(t.len(), TABLE_LEN);
        assert!(close(t[0], 0.5));
        assert!(close(t[1200], 1.0));
        assert!(close(t[2400], 2.0));
        assert!(close(t[600], 0.70710678));
    }

    #[test]
    fn transposed_maps_the_key_window_to_half_to_double() {
        assert!(close(transposed(33, 0), 0.5), "lowest key of the window");
        assert!(close(transposed(45, 0), 1.0), "key 45 is the base pitch");
        assert!(close(transposed(57, 0), 2.0), "highest key of the window");
    }

    #[test]
    fn transposed_is_octave_invariant_and_uses_fine_pitch() {
        assert!(close(transposed(20, 0), transposed(32, 0)));
        assert!(close(transposed(45, 0), transposed(57 - 12, 0)));
        // A fine pitch of 100 is one key: 44 with +100 is 45.
        assert!(close(transposed(44, 100), transposed(45, 0)));
        // Fine pitch 50 is half a key, i.e. 2^(0.5/12) above key 45.
        assert!(close(transposed(45, 50), 2f32.powf(0.5 / 12.0)));
    }

    #[test]
    fn in_octave_wraps_into_the_base_octave() {
        assert!(close(in_octave(33, 0), 0.5));
        assert!(close(in_octave(45, 0), 1.0));
        assert!(
            close(in_octave(57, 0), 0.5),
            "key 57 starts the next octave: base again"
        );
        assert!(close(in_octave(44, 100), in_octave(45, 0)));
    }

    #[test]
    fn volume_is_layer_times_radio_times_player_times_velocity() {
        assert!(close(volume(100, 100, 100, 100), 1.0));
        assert!(
            close(volume(50, 80, 100, 100), 0.4),
            "world radio 80 on a half layer"
        );
        assert!(close(volume(100, 100, 100, 0), 0.0));
        assert!(close(volume(100, 100, 50, 100), 0.5));
    }
}
