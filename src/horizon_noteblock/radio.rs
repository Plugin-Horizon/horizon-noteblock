//! One radio: its playlist, the playback clock of its current song and the order of its songs.
//!
//! The clock is NoteBlockAPI's and runs on real time: a song tick every `1000 / tempo` milliseconds, ticks `0` to
//! `length` (inclusive) are played, then the song ends. Between songs the radio waits `delay` seconds. The order is
//! the playlist's (repeat starts it again) or random (every song once, then again with repeat, else the radio goes idle).

use crate::horizon_noteblock::library::Library;

/// Which radio: a world radio (every player in the world), a server radio (every player) or a position radio
/// (players within a radius).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    World,
    Server,
    Position,
}

impl Kind {
    /// `%radio_type%` in the messages and the word of the kind's commands' messages.
    pub fn word(self) -> &'static str {
        match self {
            Kind::World => "world",
            Kind::Server => "server",
            Kind::Position => "position",
        }
    }

    /// The command that runs this kind of radio.
    pub fn command(self) -> &'static str {
        match self {
            Kind::World => "worldradio",
            Kind::Server => "serverradio",
            Kind::Position => "positionradio",
        }
    }
}

/// What a radio did on one step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A song starts (its action bar goes out, when the radio has `actionbar`).
    Start { song: String },
    /// The notes of one song tick are played to the listeners.
    Tick { song: String, tick: i32 },
    /// The song ran to its length and ended on its own (a skip is not reported here).
    End { song: String },
}

/// Milliseconds of one server tick (20 per second): what `Radio::step` and `Engine::tick` count when they are not
/// given the real time.
pub const MS_PER_SERVER_TICK: f32 = 50.0;

/// The most real time one step counts. A stalled server then does not play a burst of catch-up notes.
pub const MAX_STEP_MS: f32 = 200.0;

/// The random source of the radios (xorshift64), seeded by the server.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A number in `0..n` (0 when `n` is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

/// What a radio is made of (from the file or a command).
#[derive(Clone, Debug, PartialEq)]
pub struct RadioSpec {
    pub kind: Kind,
    pub name: String,
    pub world: String,
    pub center: [f64; 3],
    pub radius: f64,
    pub playlist: String,
    /// The playlist's songs that have a file, in order.
    pub songs: Vec<String>,
    pub repeat: bool,
    pub random: bool,
    pub actionbar: bool,
    pub delay_secs: f64,
    pub volume: u8,
}

/// One radio with its playback state. `enabled` is the radio's switch; `loaded` is whether its file settings pass
/// the checks (playlist, songs, world), so it may be switched on at all.
#[derive(Clone, Debug)]
pub struct Radio {
    pub kind: Kind,
    pub name: String,
    pub world: String,
    pub center: [f64; 3],
    pub radius: f64,
    pub playlist: String,
    pub songs: Vec<String>,
    pub repeat: bool,
    pub random: bool,
    pub actionbar: bool,
    /// The wait between two songs, in milliseconds.
    pub delay_ms: f32,
    pub volume: u8,
    pub enabled: bool,
    pub loaded: bool,
    pending: Option<usize>,
    current: Option<usize>,
    idle: bool,
    /// Milliseconds left to wait before the next song.
    wait: f32,
    tick: i32,
    /// Milliseconds of real time since the last song tick.
    acc: f32,
    played: Vec<bool>,
}

/// Milliseconds per song tick (`1000 / tempo`; two server ticks for a zero tempo).
fn period_of(tempo: f32) -> f32 {
    if tempo > 0.0 { 1000.0 / tempo } else { 2.0 * MS_PER_SERVER_TICK }
}

impl Radio {
    pub fn new(spec: RadioSpec) -> Radio {
        let n = spec.songs.len();
        Radio {
            kind: spec.kind,
            name: spec.name,
            world: spec.world,
            center: spec.center,
            radius: spec.radius,
            playlist: spec.playlist,
            songs: spec.songs,
            repeat: spec.repeat,
            random: spec.random,
            actionbar: spec.actionbar,
            delay_ms: (spec.delay_secs.max(0.0) * 1000.0) as f32,
            volume: spec.volume.min(100),
            enabled: false,
            loaded: false,
            pending: None,
            current: None,
            idle: false,
            wait: 0.0,
            tick: -1,
            acc: 0.0,
            played: vec![false; n],
        }
    }

    /// The name of the song playing now.
    pub fn current_song(&self) -> Option<&str> {
        self.current.map(|i| self.songs[i].as_str())
    }

    fn first_index(&mut self, rng: &mut Rng) -> Option<usize> {
        let n = self.songs.len();
        if n == 0 {
            return None;
        }
        if self.random {
            for p in self.played.iter_mut() {
                *p = false;
            }
            Some(rng.below(n))
        } else {
            Some(0)
        }
    }

    /// Switches the radio on: the first song starts on the next tick.
    pub fn start(&mut self, rng: &mut Rng) {
        self.enabled = true;
        self.idle = false;
        self.current = None;
        self.wait = 0.0;
        self.tick = -1;
        self.acc = 0.0;
        self.pending = self.first_index(rng);
        if self.pending.is_none() {
            self.idle = true;
        }
    }

    /// Switches the radio off; the song stops.
    pub fn stop(&mut self) {
        self.enabled = false;
        self.current = None;
        self.pending = None;
        self.idle = false;
        self.wait = 0.0;
        self.tick = -1;
        self.acc = 0.0;
    }

    /// Ends the current song now (`skip`). Returns its name, or `None` when nothing plays.
    pub fn skip(&mut self, rng: &mut Rng) -> Option<String> {
        let name = self.current_song()?.to_string();
        self.end_song(rng);
        Some(name)
    }

    fn start_song(&mut self, index: usize, lib: &Library) -> Option<Event> {
        let name = self.songs.get(index)?.clone();
        let entry = lib.get(&name)?;
        let per = period_of(entry.song.tempo);
        self.current = Some(index);
        self.tick = -1;
        // Starting one server tick short makes the first song tick come on the first normal step.
        self.acc = per - MS_PER_SERVER_TICK;
        if self.random {
            self.played[index] = true;
        }
        Some(Event::Start { song: name })
    }

    /// The song index after `finished` (None at a start), or `None` when the playlist is over.
    fn next_after(&mut self, finished: Option<usize>, rng: &mut Rng) -> Option<usize> {
        let n = self.songs.len();
        if n == 0 {
            return None;
        }
        if self.random {
            if let Some(f) = finished {
                self.played[f] = true;
            }
            let left: Vec<usize> = (0..n).filter(|&i| !self.played[i]).collect();
            if !left.is_empty() {
                return Some(left[rng.below(left.len())]);
            }
            for p in self.played.iter_mut() {
                *p = false;
            }
            let pick = rng.below(n);
            return if self.repeat { Some(pick) } else { None };
        }
        match finished {
            Some(f) if f + 1 < n => Some(f + 1),
            _ => {
                if self.repeat {
                    Some(0)
                } else {
                    None
                }
            }
        }
    }

    fn end_song(&mut self, rng: &mut Rng) {
        let finished = self.current.take();
        self.wait = self.delay_ms;
        self.acc = 0.0;
        match self.next_after(finished, rng) {
            Some(i) => self.pending = Some(i),
            None => {
                self.pending = None;
                self.idle = true;
            }
        }
    }

    /// One server tick (`MS_PER_SERVER_TICK` of time): see `step_for`.
    pub fn step(&mut self, lib: &Library, rng: &mut Rng) -> Vec<Event> {
        self.step_for(lib, rng, MS_PER_SERVER_TICK)
    }

    /// `elapsed_ms` of real time: starts a song when the wait is over, and returns the song ticks due now.
    pub fn step_for(&mut self, lib: &Library, rng: &mut Rng, elapsed_ms: f32) -> Vec<Event> {
        let mut out = Vec::new();
        if !self.enabled || self.idle {
            return out;
        }
        let elapsed = elapsed_ms.clamp(0.0, MAX_STEP_MS);
        if self.current.is_none() {
            if self.wait > 0.0 {
                self.wait -= elapsed;
                return out;
            }
            let Some(next) = self.pending.take() else {
                self.idle = true;
                return out;
            };
            match self.start_song(next, lib) {
                Some(event) => out.push(event),
                None => {
                    // The file is gone: skip it.
                    self.pending = self.next_after(Some(next), rng);
                    if self.pending.is_none() {
                        self.idle = true;
                    }
                    return out;
                }
            }
        }
        let Some(index) = self.current else {
            return out;
        };
        let name = self.songs[index].clone();
        let Some(entry) = lib.get(&name) else {
            self.end_song(rng);
            return out;
        };
        let per = period_of(entry.song.tempo);
        let length = entry.song.length;
        self.acc += elapsed;
        while self.acc >= per {
            self.acc -= per;
            self.tick += 1;
            if self.tick > length {
                out.push(Event::End { song: name.clone() });
                self.end_song(rng);
                break;
            }
            out.push(Event::Tick {
                song: name.clone(),
                tick: self.tick,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::horizon_noteblock::nbs;
    use crate::horizon_noteblock::nbs::testutil::{FileSpec, note};

    /// A song of `length` ticks at 10 ticks per second (one song tick every 2 server ticks), with one note per tick.
    fn library_with(songs: &[(&str, i32)]) -> Library {
        let mut lib = Library::new(std::path::PathBuf::from("unused"), Default::default());
        for (name, length) in songs {
            let notes: Vec<_> = (0..=*length).map(|t| note(t, 0, 0, 45)).collect();
            // Format 5 stores the length; a legacy (format 0) file cannot have length 0, because a
            // leading 0 marks the new format.
            let spec = FileSpec::new(5, *length as u16, 1, 1000, notes);
            lib.insert(name, nbs::parse(&spec.bytes()).expect("parses"));
        }
        lib
    }

    fn spec(songs: &[&str], repeat: bool, random: bool, delay_secs: f64) -> RadioSpec {
        RadioSpec {
            kind: Kind::World,
            name: "fixture_radio".into(),
            world: "fixture_world".into(),
            center: [0.0, 0.0, 0.0],
            radius: 0.0,
            playlist: "fixture".into(),
            songs: songs.iter().map(|s| s.to_string()).collect(),
            repeat,
            random,
            actionbar: true,
            delay_secs,
            volume: 100,
        }
    }

    /// Runs `ticks` server ticks and returns (server tick index, event) for every event.
    fn run(radio: &mut Radio, lib: &Library, rng: &mut Rng, ticks: usize) -> Vec<(usize, Event)> {
        let mut out = Vec::new();
        for t in 0..ticks {
            for e in radio.step(lib, rng) {
                out.push((t, e));
            }
        }
        out
    }

    fn starts(events: &[(usize, Event)]) -> Vec<(usize, String)> {
        events
            .iter()
            .filter_map(|(t, e)| match e {
                Event::Start { song } => Some((*t, song.clone())),
                _ => None,
            })
            .collect()
    }

    fn ticks_of(events: &[(usize, Event)]) -> Vec<(usize, i32)> {
        events
            .iter()
            .filter_map(|(t, e)| match e {
                Event::Tick { tick, .. } => Some((*t, *tick)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_disabled_radio_plays_nothing() {
        let lib = library_with(&[("tone", 3)]);
        let mut radio = Radio::new(spec(&["tone"], true, false, 0.0));
        let mut rng = Rng::new(7);
        assert!(run(&mut radio, &lib, &mut rng, 100).is_empty());
    }

    #[test]
    fn song_ticks_follow_the_tempo_and_run_to_the_length_inclusive() {
        let lib = library_with(&[("tone", 3)]);
        let mut radio = Radio::new(spec(&["tone"], false, false, 0.0));
        let mut rng = Rng::new(7);
        radio.start(&mut rng);
        let events = run(&mut radio, &lib, &mut rng, 20);
        assert_eq!(starts(&events), vec![(0, "tone".to_string())]);
        // 10 song ticks per second: one every 2 server ticks, starting at once; ticks 0..=3 (length 3).
        assert_eq!(ticks_of(&events), vec![(0, 0), (2, 1), (4, 2), (6, 3)]);
        assert!(radio.current_song().is_none(), "the song ended");
    }

    #[test]
    fn playlist_order_and_delay_between_songs() {
        let lib = library_with(&[("a", 0), ("b", 0)]);
        // Delay 0.5 s = 10 server ticks between the songs.
        let mut radio = Radio::new(spec(&["a", "b"], false, false, 0.5));
        let mut rng = Rng::new(7);
        radio.start(&mut rng);
        let events = run(&mut radio, &lib, &mut rng, 40);
        let s = starts(&events);
        assert_eq!(
            s.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        let gap = s[1].0 - s[0].0;
        assert!(gap >= 10, "the second song waits the delay, gap was {gap}");
    }

    #[test]
    fn repeat_restarts_the_playlist_and_no_repeat_goes_idle() {
        let lib = library_with(&[("a", 0), ("b", 0)]);
        let mut rng = Rng::new(7);

        let mut repeating = Radio::new(spec(&["a", "b"], true, false, 0.0));
        repeating.start(&mut rng);
        let names: Vec<String> = starts(&run(&mut repeating, &lib, &mut rng, 60))
            .into_iter()
            .map(|(_, n)| n)
            .collect();
        assert!(names.len() >= 4, "{names:?}");
        assert_eq!(&names[..4], &["a", "b", "a", "b"]);

        let mut once = Radio::new(spec(&["a", "b"], false, false, 0.0));
        once.start(&mut rng);
        let names: Vec<String> = starts(&run(&mut once, &lib, &mut rng, 60))
            .into_iter()
            .map(|(_, n)| n)
            .collect();
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn random_order_plays_each_song_once_before_repeating() {
        let lib = library_with(&[("a", 0), ("b", 0), ("c", 0)]);
        let mut rng = Rng::new(12345);
        let mut radio = Radio::new(spec(&["a", "b", "c"], false, true, 0.0));
        radio.start(&mut rng);
        let names: Vec<String> = starts(&run(&mut radio, &lib, &mut rng, 80))
            .into_iter()
            .map(|(_, n)| n)
            .collect();
        assert_eq!(names.len(), 3, "without repeat each song once: {names:?}");
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["a", "b", "c"]);
    }

    #[test]
    fn skip_ends_the_current_song_and_starts_the_next() {
        let lib = library_with(&[("a", 50), ("b", 0)]);
        let mut rng = Rng::new(3);
        let mut radio = Radio::new(spec(&["a", "b"], false, false, 0.0));
        radio.start(&mut rng);
        let _ = run(&mut radio, &lib, &mut rng, 4);
        assert_eq!(radio.current_song(), Some("a"));
        assert_eq!(radio.skip(&mut rng), Some("a".to_string()));
        let events = run(&mut radio, &lib, &mut rng, 4);
        assert_eq!(starts(&events), vec![(0, "b".to_string())]);
    }

    #[test]
    fn skip_on_an_idle_radio_does_nothing() {
        let mut rng = Rng::new(3);
        let mut radio = Radio::new(spec(&["a"], false, false, 0.0));
        assert_eq!(radio.skip(&mut rng), None);
    }

    #[test]
    fn stop_silences_the_radio() {
        let lib = library_with(&[("a", 100)]);
        let mut rng = Rng::new(3);
        let mut radio = Radio::new(spec(&["a"], true, false, 0.0));
        radio.start(&mut rng);
        let _ = run(&mut radio, &lib, &mut rng, 5);
        radio.stop();
        assert!(run(&mut radio, &lib, &mut rng, 50).is_empty());
        assert!(!radio.enabled);
    }

    #[test]
    fn rng_is_deterministic_and_bounded() {
        let mut a = Rng::new(9);
        let mut b = Rng::new(9);
        assert_eq!(a.next_u64(), b.next_u64());
        for _ in 0..200 {
            assert!(a.below(5) < 5);
        }
        assert_eq!(a.below(0), 0);
    }

    #[test]
    fn period_follows_tempo() {
        assert_eq!(period_of(10.0), 100.0);
        assert_eq!(period_of(5.0), 200.0);
        assert_eq!(period_of(0.0), 100.0);
    }

    #[test]
    fn song_ticks_follow_real_time_not_server_ticks() {
        // Tempo 10: a song tick every 100 ms. Steps of 25 ms (a fast server) give one every 4 steps.
        let lib = library_with(&[("tone", 3)]);
        let mut radio = Radio::new(spec(&["tone"], false, false, 0.0));
        let mut rng = Rng::new(7);
        radio.start(&mut rng);
        let mut ticks = Vec::new();
        for t in 0..16 {
            for e in radio.step_for(&lib, &mut rng, 25.0) {
                if let Event::Tick { tick, .. } = e {
                    ticks.push((t, tick));
                }
            }
        }
        // A start holds 50 ms, the first step adds 25 (75 ms), the second reaches 100 ms: tick 0 on step 1.
        assert_eq!(ticks, vec![(1, 0), (5, 1), (9, 2), (13, 3)]);
    }

    #[test]
    fn a_stalled_step_counts_at_most_the_cap() {
        // A 10 s stall at tempo 10 would be 100 song ticks; the cap lets through 200 ms = two song ticks.
        let lib = library_with(&[("tone", 50)]);
        let mut radio = Radio::new(spec(&["tone"], false, false, 0.0));
        let mut rng = Rng::new(7);
        radio.start(&mut rng);
        let _ = radio.step_for(&lib, &mut rng, 0.0);
        let events = radio.step_for(&lib, &mut rng, 10_000.0);
        let n = events.iter().filter(|e| matches!(e, Event::Tick { .. })).count();
        assert_eq!(n, 2, "{events:?}");
    }

    #[test]
    fn a_song_that_runs_out_reports_its_end() {
        let lib = library_with(&[("tone", 1)]);
        let mut radio = Radio::new(spec(&["tone"], false, false, 0.0));
        let mut rng = Rng::new(7);
        radio.start(&mut rng);
        let events = run(&mut radio, &lib, &mut rng, 20);
        let ends: Vec<&Event> = events.iter().map(|(_, e)| e).filter(|e| matches!(e, Event::End { .. })).collect();
        assert_eq!(ends, vec![&Event::End { song: "tone".to_string() }]);
    }
}
