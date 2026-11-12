//! Note Block Studio song files (`.nbs`): the legacy format (no leading `0x0000`, format 0) and formats 1 to 5,
//! read the way NoteBlockAPI 1.7.0 reads them (`utils/NBSDecoder`). Trailing bytes after the custom instrument
//! list are ignored, so files with zero padding read the same.

use crate::instrument::FIRST_CUSTOM;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// One note: the instrument id (`>= 20` is a custom instrument), the key (33 to 57 on this server), the velocity
/// (0 to 100; 100 before format 4), the panning as NBS writes it after `200 -` (100 = centre) and the fine pitch
/// in 1/100 of a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Note {
    pub instrument: i8,
    pub key: i8,
    pub velocity: u8,
    pub panning: i32,
    pub pitch: i16,
}

/// A layer: its name, its volume (0 to 100, 100 when the header gives no record) and its panning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub name: String,
    pub volume: u8,
    pub panning: i32,
}

impl Default for Layer {
    fn default() -> Self {
        Layer {
            name: String::new(),
            volume: 100,
            panning: 100,
        }
    }
}

/// A custom instrument: its name, its sound file, pitch and key (kept; custom sounds are not played here).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomInstrument {
    pub name: String,
    pub file: String,
    pub pitch: u8,
    pub key: u8,
}

/// A parsed song. `notes` holds every note by tick, as (layer index, note), in file order.
#[derive(Clone, Debug, PartialEq)]
pub struct Song {
    pub version: u8,
    /// The length as the header writes it (format 0 and 1-2 keep it in the first short; format 3+ after the marker).
    pub header_length: u16,
    /// The length the player uses: the header value, or the last tick for formats 1 and 2 (NoteBlockAPI's rule).
    pub length: i32,
    /// The layer count of the header (may be 0 in a file that has layers).
    pub height: u16,
    /// Ticks per second (the header's tempo divided by 100).
    pub tempo: f32,
    pub title: String,
    pub author: String,
    pub original_author: String,
    pub description: String,
    pub autosave: bool,
    pub autosave_duration: u8,
    pub time_signature: u8,
    pub minutes_spent: i32,
    pub left_clicks: i32,
    pub right_clicks: i32,
    pub blocks_added: i32,
    pub blocks_removed: i32,
    pub midi_file_name: String,
    pub loop_enabled: bool,
    pub loop_max_count: u8,
    pub loop_start_tick: u16,
    /// The first custom instrument id the file was written with (10 for format 0).
    pub first_custom_instrument: u8,
    pub custom_instruments: Vec<CustomInstrument>,
    pub layers: Vec<Layer>,
    pub notes: BTreeMap<i32, Vec<(usize, Note)>>,
    pub note_count: usize,
}

impl Song {
    /// The notes of one tick (empty when none).
    pub fn notes_at(&self, tick: i32) -> &[(usize, Note)] {
        match self.notes.get(&tick) {
            Some(v) => v.as_slice(),
            None => &[],
        }
    }
}

/// What `examples/check_original` prints for a song.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub version: u8,
    pub length: i32,
    pub tempo: f32,
    pub header_layers: u16,
    pub layers_with_notes: usize,
    pub notes: usize,
    pub custom: usize,
}

pub fn summarize(song: &Song) -> Summary {
    let used: BTreeSet<usize> = song
        .notes
        .values()
        .flatten()
        .map(|(layer, _)| *layer)
        .collect();
    Summary {
        version: song.version,
        length: song.length,
        tempo: song.tempo,
        header_layers: song.height,
        layers_with_notes: used.len(),
        notes: song.note_count,
        custom: song.custom_instruments.len(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NbsError {
    /// The file ends inside a field (at this byte offset).
    Truncated { at: usize },
    /// A string with a negative length (at this byte offset).
    BadString { at: usize },
}

impl fmt::Display for NbsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NbsError::Truncated { at } => write!(f, "file ends early at byte {at}"),
            NbsError::BadString { at } => write!(f, "bad string length at byte {at}"),
        }
    }
}

impl std::error::Error for NbsError {}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], NbsError> {
        let data: &'a [u8] = self.data;
        let end = self
            .pos
            .checked_add(n)
            .ok_or(NbsError::Truncated { at: self.pos })?;
        if end > data.len() {
            return Err(NbsError::Truncated { at: self.pos });
        }
        let slice = &data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, NbsError> {
        Ok(self.take(1)?[0])
    }

    fn i8(&mut self) -> Result<i8, NbsError> {
        Ok(self.u8()? as i8)
    }

    fn u16(&mut self) -> Result<u16, NbsError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn i16(&mut self) -> Result<i16, NbsError> {
        let b = self.take(2)?;
        Ok(i16::from_le_bytes([b[0], b[1]]))
    }

    fn i32(&mut self) -> Result<i32, NbsError> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// An int length, then the bytes; a carriage return becomes a space (NBS editors write them).
    fn string(&mut self) -> Result<String, NbsError> {
        let at = self.pos;
        let len = self.i32()?;
        if len < 0 {
            return Err(NbsError::BadString { at });
        }
        let bytes = self.take(len as usize)?;
        Ok(bytes
            .iter()
            .map(|&b| if b == 0x0D { ' ' } else { b as char })
            .collect())
    }
}

/// Parses an `.nbs` file. Format 0 (no marker) and 1 to 5 are read; anything after the custom instruments is ignored.
pub fn parse(data: &[u8]) -> Result<Song, NbsError> {
    let mut r = Reader { data, pos: 0 };
    let first = r.u16()?;
    let mut length: i32 = first as i32;
    let mut version: u8 = 0;
    let mut first_custom: u8 = 10;
    if first == 0 {
        version = r.u8()?;
        first_custom = r.u8()?;
        if version >= 3 {
            length = r.u16()? as i32;
        }
    }
    let header_length = length as u16;
    let height = r.u16()?;
    let title = r.string()?;
    let author = r.string()?;
    let original_author = r.string()?;
    let description = r.string()?;
    let tempo = r.u16()? as f32 / 100.0;
    let autosave = r.u8()? != 0;
    let autosave_duration = r.u8()?;
    let time_signature = r.u8()?;
    let minutes_spent = r.i32()?;
    let left_clicks = r.i32()?;
    let right_clicks = r.i32()?;
    let blocks_added = r.i32()?;
    let blocks_removed = r.i32()?;
    let midi_file_name = r.string()?;
    let mut loop_enabled = false;
    let mut loop_max_count = 0u8;
    let mut loop_start_tick = 0u16;
    if version >= 4 {
        loop_enabled = r.u8()? != 0;
        loop_max_count = r.u8()?;
        loop_start_tick = r.u16()?;
    }

    // Notes: a tick jump (0 ends the ticks), then per layer jump (0 ends the layers) the note.
    let diff: i32 = FIRST_CUSTOM as i32 - first_custom as i32;
    let mut layers: Vec<Layer> = vec![Layer::default(); height as usize];
    let mut notes: BTreeMap<i32, Vec<(usize, Note)>> = BTreeMap::new();
    let mut note_count = 0usize;
    let mut tick: i32 = -1;
    loop {
        let jump = r.u16()?;
        if jump == 0 {
            break;
        }
        tick += jump as i32;
        let mut layer: i32 = -1;
        loop {
            let layer_jump = r.u16()?;
            if layer_jump == 0 {
                break;
            }
            layer += layer_jump as i32;
            let mut instrument = r.i8()?;
            let key = r.i8()?;
            let (velocity, panning, pitch) = if version >= 4 {
                let velocity = r.u8()?;
                let panning = 200 - r.u8()? as i32;
                let pitch = r.i16()?;
                (velocity, panning, pitch)
            } else {
                (100u8, 100i32, 0i16)
            };
            if diff > 0 && instrument as i32 >= first_custom as i32 {
                instrument = instrument.wrapping_add(diff as i8);
            }
            if tick >= 0 && layer >= 0 {
                let index = layer as usize;
                if index >= layers.len() {
                    layers.resize(index + 1, Layer::default());
                }
                let note = Note {
                    instrument,
                    key,
                    velocity,
                    panning,
                    pitch,
                };
                notes.entry(tick).or_default().push((index, note));
                note_count += 1;
            }
        }
    }
    if version > 0 && version < 3 {
        length = tick;
    }

    // Layers: name, lock (format 4+), volume, panning (format 2+), one record per header layer.
    for index in 0..height as usize {
        let name = r.string()?;
        if version >= 4 {
            r.u8()?;
        }
        let volume = r.u8()?;
        let panning = if version >= 2 {
            200 - r.u8()? as i32
        } else {
            100
        };
        layers[index] = Layer {
            name,
            volume,
            panning,
        };
    }

    // Custom instruments (the count is a byte; ids from FIRST_CUSTOM are these).
    let custom_count = r.u8()?;
    let mut custom_instruments = Vec::with_capacity(custom_count as usize);
    for _ in 0..custom_count {
        let name = r.string()?;
        let file = r.string()?;
        let pitch = r.u8()?;
        let key = r.u8()?;
        custom_instruments.push(CustomInstrument {
            name,
            file,
            pitch,
            key,
        });
    }

    Ok(Song {
        version,
        header_length,
        length,
        height,
        tempo,
        title,
        author,
        original_author,
        description,
        autosave,
        autosave_duration,
        time_signature,
        minutes_spent,
        left_clicks,
        right_clicks,
        blocks_added,
        blocks_removed,
        midi_file_name,
        loop_enabled,
        loop_max_count,
        loop_start_tick,
        first_custom_instrument: first_custom,
        custom_instruments,
        layers,
        notes,
        note_count,
    })
}

/// Test-only writer for `.nbs` bytes, so the tests need no real file.
#[cfg(test)]
pub(crate) mod testutil {
    use std::collections::BTreeMap;

    #[derive(Clone, Debug)]
    pub(crate) struct NoteSpec {
        pub tick: i32,
        pub layer: i32,
        pub instrument: i8,
        pub key: i8,
        pub velocity: u8,
        pub pan_raw: u8,
        pub pitch: i16,
    }

    pub(crate) fn note(tick: i32, layer: i32, instrument: i8, key: i8) -> NoteSpec {
        NoteSpec {
            tick,
            layer,
            instrument,
            key,
            velocity: 100,
            pan_raw: 100,
            pitch: 0,
        }
    }

    #[derive(Clone, Debug)]
    pub(crate) struct FileSpec {
        pub version: u8,
        pub first_custom: u8,
        pub length: u16,
        pub height: u16,
        pub tempo: u16,
        pub title: String,
        pub notes: Vec<NoteSpec>,
        /// Layer records as (name, volume, panning as written, i.e. 200 - panning).
        pub layers: Vec<(String, u8, u8)>,
        pub customs: Vec<(String, String, u8, u8)>,
        pub trailing: usize,
    }

    impl FileSpec {
        pub(crate) fn new(
            version: u8,
            length: u16,
            height: u16,
            tempo: u16,
            notes: Vec<NoteSpec>,
        ) -> FileSpec {
            FileSpec {
                version,
                first_custom: 10,
                length,
                height,
                tempo,
                title: "Test".to_string(),
                notes,
                layers: Vec::new(),
                customs: Vec::new(),
                trailing: 0,
            }
        }

        pub(crate) fn bytes(&self) -> Vec<u8> {
            let mut b = Vec::new();
            if self.version == 0 {
                put_u16(&mut b, self.length);
            } else {
                put_u16(&mut b, 0);
                b.push(self.version);
                b.push(self.first_custom);
                if self.version >= 3 {
                    put_u16(&mut b, self.length);
                }
            }
            put_u16(&mut b, self.height);
            put_str(&mut b, &self.title);
            put_str(&mut b, "Author");
            put_str(&mut b, "Original");
            put_str(&mut b, "Description");
            put_u16(&mut b, self.tempo);
            b.push(1); // autosave
            b.push(10); // autosave duration
            b.push(4); // time signature
            for v in [1i32, 2, 3, 4, 5] {
                put_i32(&mut b, v);
            }
            put_str(&mut b, "song.mid");
            if self.version >= 4 {
                b.push(1); // loop on
                b.push(2); // loop max count
                put_u16(&mut b, 7); // loop start
            }
            let mut sorted = self.notes.clone();
            sorted.sort_by_key(|n| (n.tick, n.layer));
            let mut i = 0usize;
            let mut prev_tick = -1i32;
            while i < sorted.len() {
                let tick = sorted[i].tick;
                put_u16(&mut b, (tick - prev_tick) as u16);
                prev_tick = tick;
                let mut prev_layer = -1i32;
                while i < sorted.len() && sorted[i].tick == tick {
                    let n = &sorted[i];
                    put_u16(&mut b, (n.layer - prev_layer) as u16);
                    prev_layer = n.layer;
                    b.push(n.instrument as u8);
                    b.push(n.key as u8);
                    if self.version >= 4 {
                        b.push(n.velocity);
                        b.push(n.pan_raw);
                        b.extend_from_slice(&n.pitch.to_le_bytes());
                    }
                    i += 1;
                }
                put_u16(&mut b, 0);
            }
            put_u16(&mut b, 0);
            let mut records: BTreeMap<usize, (String, u8, u8)> = BTreeMap::new();
            for (k, rec) in self.layers.iter().enumerate() {
                records.insert(k, rec.clone());
            }
            for k in 0..self.height as usize {
                let (name, volume, pan) =
                    records
                        .get(&k)
                        .cloned()
                        .unwrap_or((String::new(), 100, 100));
                put_str(&mut b, &name);
                if self.version >= 4 {
                    b.push(0); // lock
                }
                b.push(volume);
                if self.version >= 2 {
                    b.push(pan);
                }
            }
            b.push(self.customs.len() as u8);
            for (name, file, pitch, key) in &self.customs {
                put_str(&mut b, name);
                put_str(&mut b, file);
                b.push(*pitch);
                b.push(*key);
            }
            b.extend(std::iter::repeat_n(0u8, self.trailing));
            b
        }
    }

    fn put_u16(b: &mut Vec<u8>, v: u16) {
        b.extend_from_slice(&v.to_le_bytes());
    }

    fn put_i32(b: &mut Vec<u8>, v: i32) {
        b.extend_from_slice(&v.to_le_bytes());
    }

    fn put_str(b: &mut Vec<u8>, s: &str) {
        put_i32(b, s.len() as i32);
        b.extend_from_slice(s.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::{FileSpec, note};
    use super::*;

    fn parse_spec(spec: &FileSpec) -> Song {
        parse(&spec.bytes()).expect("synthetic file parses")
    }

    #[test]
    fn legacy_format_reads_header_layers_and_notes() {
        let mut spec = FileSpec::new(0, 100, 2, 1000, vec![note(0, 0, 0, 45), note(5, 1, 1, 33)]);
        spec.layers = vec![("bass line".into(), 80, 100)];
        let song = parse_spec(&spec);
        assert_eq!(song.version, 0);
        assert_eq!(song.header_length, 100);
        assert_eq!(song.length, 100);
        assert_eq!(song.height, 2);
        assert!((song.tempo - 10.0).abs() < f32::EPSILON);
        assert_eq!(song.note_count, 2);
        assert_eq!(song.layers[0].volume, 80);
        assert_eq!(song.layers[0].name, "bass line");
        assert_eq!(
            song.layers[1].volume, 100,
            "a layer without a record keeps the default volume"
        );
        assert_eq!(
            song.notes_at(5),
            &[(
                1usize,
                Note {
                    instrument: 1,
                    key: 33,
                    velocity: 100,
                    panning: 100,
                    pitch: 0
                }
            )]
        );
        assert!(song.notes_at(3).is_empty());
    }

    #[test]
    fn header_fields_are_all_read() {
        let mut spec = FileSpec::new(3, 42, 0, 500, vec![]);
        spec.title = "Title\rLine".into();
        let song = parse_spec(&spec);
        assert_eq!(
            song.title, "Title Line",
            "a carriage return becomes a space"
        );
        assert_eq!(song.author, "Author");
        assert_eq!(song.original_author, "Original");
        assert_eq!(song.description, "Description");
        assert!(song.autosave);
        assert_eq!(song.autosave_duration, 10);
        assert_eq!(song.time_signature, 4);
        assert_eq!(song.minutes_spent, 1);
        assert_eq!(song.left_clicks, 2);
        assert_eq!(song.right_clicks, 3);
        assert_eq!(song.blocks_added, 4);
        assert_eq!(song.blocks_removed, 5);
        assert_eq!(song.midi_file_name, "song.mid");
        assert_eq!(song.first_custom_instrument, 10);
    }

    #[test]
    fn format_one_and_two_take_the_last_tick_as_length() {
        let notes = vec![note(0, 0, 0, 45), note(7, 0, 0, 45)];
        let v1 = parse_spec(&FileSpec::new(1, 999, 1, 1000, notes.clone()));
        assert_eq!(v1.length, 7);
        // Formats 1 and 2 store no length in the header (it starts in format 3).
        assert_eq!(v1.header_length, 0);
        let v2 = parse_spec(&FileSpec::new(2, 999, 1, 1000, notes));
        assert_eq!(v2.length, 7);
    }

    #[test]
    fn format_two_reads_layer_panning() {
        let mut spec = FileSpec::new(2, 10, 1, 1000, vec![note(0, 0, 0, 45)]);
        spec.layers = vec![("left".into(), 90, 50)];
        let song = parse_spec(&spec);
        assert_eq!(song.layers[0].panning, 150, "written as 200 - panning");
        assert_eq!(song.layers[0].volume, 90);
    }

    #[test]
    fn format_three_keeps_the_header_length() {
        let song = parse_spec(&FileSpec::new(
            3,
            300,
            0,
            1000,
            vec![note(0, 0, 0, 45), note(9, 0, 0, 45)],
        ));
        assert_eq!(song.version, 3);
        assert_eq!(song.length, 300);
        assert_eq!(song.note_count, 2);
    }

    #[test]
    fn formats_four_and_five_read_velocity_panning_pitch_and_loop() {
        for version in [4u8, 5] {
            let mut n = note(3, 0, 2, 40);
            n.velocity = 60;
            n.pan_raw = 150;
            n.pitch = -35;
            let song = parse_spec(&FileSpec::new(version, 20, 1, 1000, vec![n]));
            assert_eq!(song.version, version);
            assert!(song.loop_enabled);
            assert_eq!(song.loop_max_count, 2);
            assert_eq!(song.loop_start_tick, 7);
            let (layer, got) = song.notes_at(3)[0];
            assert_eq!(layer, 0);
            assert_eq!(got.velocity, 60);
            assert_eq!(got.panning, 50, "panning is 200 - the written byte");
            assert_eq!(got.pitch, -35);
            assert_eq!(got.instrument, 2);
        }
    }

    #[test]
    fn zero_layers_in_header_and_one_custom_instrument() {
        let mut spec = FileSpec::new(0, 50, 0, 1000, vec![note(0, 0, 1, 45)]);
        spec.customs = vec![("Drum".into(), "drum.ogg".into(), 12, 33)];
        let song = parse_spec(&spec);
        assert_eq!(song.height, 0);
        assert_eq!(song.layers[0].volume, 100, "no header record: volume 100");
        assert_eq!(song.custom_instruments.len(), 1);
        assert_eq!(song.custom_instruments[0].file, "drum.ogg");
        assert_eq!(song.custom_instruments[0].pitch, 12);
        assert_eq!(summarize(&song).custom, 1);
    }

    #[test]
    fn legacy_custom_instrument_ids_are_shifted_like_noteblockapi() {
        // Format 0 with first custom id 10: ids 10 and up move by 20 - 10 = 10.
        let song = parse_spec(&FileSpec::new(
            0,
            10,
            0,
            1000,
            vec![note(0, 0, 12, 45), note(1, 0, 5, 45)],
        ));
        let shifted = song.notes_at(0)[0].1.instrument;
        assert_eq!(shifted, 22);
        assert_eq!(song.notes_at(1)[0].1.instrument, 5);
    }

    #[test]
    fn trailing_zero_padding_is_ignored() {
        let mut spec = FileSpec::new(0, 4, 1, 1000, vec![note(0, 0, 0, 45)]);
        spec.trailing = 4096;
        let song = parse_spec(&spec);
        assert_eq!(song.note_count, 1);
        assert_eq!(song.custom_instruments.len(), 0);
    }

    #[test]
    fn summary_counts_layers_with_notes() {
        let song = parse_spec(&FileSpec::new(
            0,
            9,
            3,
            500,
            vec![note(0, 0, 0, 45), note(1, 2, 0, 45), note(2, 2, 0, 46)],
        ));
        let s = summarize(&song);
        assert_eq!(s.header_layers, 3);
        assert_eq!(s.layers_with_notes, 2);
        assert_eq!(s.notes, 3);
        assert!((s.tempo - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn truncated_file_is_an_error() {
        let bytes = FileSpec::new(0, 10, 1, 1000, vec![note(0, 0, 0, 45)]).bytes();
        let cut = &bytes[..bytes.len() - 3];
        assert!(matches!(parse(cut), Err(NbsError::Truncated { .. })));
        assert!(matches!(parse(&[]), Err(NbsError::Truncated { at: 0 })));
    }
}
