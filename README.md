# horizon-noteblock

> **Unofficial Rust port of NoteBlockMusicPlayer and NoteBlockAPI for the Horizon server (https://github.com/horizon-rust/horizon). Not affiliated with or endorsed by the NoteBlockMusicPlayer or NoteBlockAPI authors.**

Note block music for [Horizon](https://github.com/horizon-rust/horizon) servers, as a native mod (a horizon-mixins
bundle): Note Block Studio songs (`.nbs`) are played as vanilla note block sounds to the players of a world radio, a
server radio or a position radio, with playlists, the song titles of `songs.yml` and the action bar, and the commands
of NoteBlockMusicPlayer. It reads the original plugin folder (`plugins/NoteBlockMusicPlayer/`) as it is, and
NoteBlockAPI's song player is part of it (no separate library).

## What it does

- **NBS songs**: formats 0 (legacy) to 5, all header fields, layers, custom instrument records; trailing padding is
  ignored. Parsed the way NoteBlockAPI 1.7.0 parses them (`core/src/nbs.rs`).
- **Instruments**: ids 0 to 19 are vanilla note block sounds on Minecraft 26.x (16 to 19 are the trumpets); custom
  instruments (from 20) are read and not played.
- **Pitch and volume**: NoteUtils' 2401-entry table, the two-octave window (`use10OctaveRange: false`, this server),
  the base-octave formula for `true`; volume = layer x radio x player x velocity (each 0 to 100, over 10^8).
- **Clock**: a song tick every `20 / tempo` server ticks, ticks `0..=length`; `delay` seconds between songs.
- **Radios**: world radios (players in the world), server radios (everyone), position radios (players within the
  radius of `x y z`); playlist order, `repeat`, `random`; one world radio per world and one server radio, as the
  messages say; the checks (playlist, songs, world) with the messages of `messages_en.yml`.
- **Titles**: `songs.yml` title and author first (block scalars such as `|2-` included), then the song's own title,
  then the file name. Action bar `You are now listening %song_title%` when a song starts (`actionbar: true`).
- **Player state**: `/radio toggle` (mute all radios for you), `/radio volume <0-100>`; the default volume from
  `config.yml`. Not saved across log-outs (`savePlayerVolumes: false`).
- **Commands**: `/radio`, `/worldradio` (`wr`), `/serverradio` (`sr`), `/positionradio` (`pr`), `/noteblockmusicplayer`
  (`nbmp`): enable, disable, skip, reload, playlist list/songs/create/add/remove/reload, hologram (answers "not
  enabled"). Permission nodes from `plugin.yml`. `playlists.yml` is written back in place on create/add/remove, as
  NBMP writes it.
- **Holograms**: `holograms.yml` is read; no hologram provider exists on the server, so nothing is shown.

What is ported and what is not: [STATUS.md](STATUS.md). What Horizon would need for the rest: [GAPS.md](GAPS.md).

On the server this replaces, every radio is disabled in the files, so this mod starts nothing; it only reads them.

## Layout

- `core/`: the logic (`nbs`, `instrument`, `pitch`, `config`, `library`, `radio`, `engine`, `commands`, `messages`),
  unit tests beside the code and `engine_tests.rs` / `commands_tests.rs`; `core/testdata/nbmp/` is a synthetic
  folder with the same flags as the server's files and invented values; `core/src/yaml.rs` is horizon-yaml (MIT),
  vendored unchanged.
- `core/examples/check_original.rs`: `cargo run --example check_original -- <files>` prints, per file, the format,
  length, tempo, layer and note counts of an `.nbs` song, and for a `.yml` file the key paths read and not read (key
  paths only; no values, no titles, no song names beyond key names).
- `helpers/`: the glue (`glue.rs` becomes `crate::game::noteblock_glue`), the three hook helpers and the API module.
- `horizon-mixin.toml`: the bundle (tick, register commands, perform command).

## Testing

```
cd core && CARGO_TARGET_DIR=<repo>/target cargo test
cd core && CARGO_TARGET_DIR=<repo>/target cargo run --example check_original -- <original files>
```

## Licence and sources

GPL-3.0-or-later (see `LICENSE` and `NOTICE`). Derived from NoteBlockAPI (LGPL-3.0; its source was read, the logic was
written again in Rust, nothing copied) and NoteBlockMusicPlayer 1.10.0 (its behaviour follows the server's files and
its command strings; its source could not be located).

Not affiliated with NoteBlockMusicPlayer, NoteBlockAPI or their authors.
