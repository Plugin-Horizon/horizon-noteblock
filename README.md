# horizon-noteblock

> **Unofficial Rust port of NoteBlockMusicPlayer and NoteBlockAPI for the Horizon server (https://github.com/arismoko/horizon). Not affiliated with or endorsed by the NoteBlockMusicPlayer or NoteBlockAPI authors.**

> **Built for and working only with [arismoko/horizon](https://github.com/arismoko/horizon)** (the Horizon fork that carries the mod API). [horizon-rust/horizon](https://github.com/horizon-rust/horizon) is experimental for mods and not supported: the earlier version for its native hooks / horizon-mixins is frozen on the `horizon-plain` branch (work stopped on 2026-11-12).

## Which Horizon

This is a native trait mod: `horizon-mod.toml`, a `Cargo.toml` that depends on `horizon-mod-api`, `src/lib.rs` with the hooks, and the plugin logic in the `core/` crate (plain Rust, no engine needed for its tests).

- Pin: `[native] horizon = "54090ee36c4e2c7b43b66db864220c577b94b7e0"` in `horizon-mod.toml` (game `26.3`). That is a development build of arismoko/horizon `main` plus mod API additions that are not all merged upstream yet, so a stock `main` may lack some host methods this mod calls until they land.
- It compiled and loaded on that build and was smoke-tested there; it has not seen a long run with real players. Treat it as experimental.
- Feature descriptions below were written for the earlier horizon-rust version; where they name engine calls (`native_*`) or old file paths, read them as descriptions only. The Known gaps list is current.

## What it does

- **NBS songs**: formats 0 (legacy) to 5, all header fields, layers, custom instrument records; trailing padding is
  ignored. Parsed the way NoteBlockAPI 1.7.0 parses them (`core/src/nbs.rs`).
- **Instruments**: ids 0 to 19 are vanilla note block sounds on Minecraft 26.x (16 to 19 are the trumpets); custom
  instruments (from 20) are read and not played.
- **Pitch and volume**: NoteUtils' 2401-entry table, the two-octave window (`use10OctaveRange: false`, this server),
  the base-octave formula for `true`; volume = layer x radio x player x velocity (each 0 to 100, over 10^8).
- **Clock**: real time, a song tick every `1000 / tempo` ms, ticks `0..=length`; `delay` seconds between songs.
- **Radios**: world radios (players in the world), server radios (everyone), position radios (players within the
  radius of `x y z`); playlist order, `repeat`, `random`; one world radio per world and one server radio, as the
  messages say; the checks (playlist, songs, world) with the messages of `messages_en.yml`.
- **Titles**: `songs.yml` title and author first (block scalars such as `|2-` included), then the song's own title,
  then the file name. Action bar `You are now listening %song_title%` when a song starts (`actionbar: true`).
- **Player state**: `/radio toggle` (mute all radios for you), `/radio volume <0-100>`; the default volume from
  `config.yml`. Kept in memory across log-outs while the server runs; with `savePlayerVolumes: true` written to
  `playerVolumes.yml` and read at the next start.
- **Joins**: a player who joins a radio's audience sees the action bar of its song (the first radio in the file's order).
- **Commands**: `/radio`, `/worldradio` (`wr`), `/serverradio` (`sr`), `/positionradio` (`pr`), `/noteblockmusicplayer`
  (`nbmp`): enable, disable, skip, reload, playlist list/songs/create/add/remove/reload, hologram (answers "not
  enabled"). Permission nodes from `plugin.yml`. `playlists.yml` is written back in place on create/add/remove, as
  NBMP writes it.
- **Holograms**: `holograms.yml` is read; no hologram provider exists on the server, so nothing is shown.

What is ported and what is not: the Known gaps section. What Horizon would need for the rest: the Known gaps section.

On the server this replaces, every radio is disabled in the files, so this mod starts nothing; it only reads them.

## Build

Build it as a native trait mod with arismoko's launcher (see the "native mods" how-to in the arismoko/horizon docs):

```sh
horizon-launcher native-build <path to this folder> --server-output <dir> --version <tag> --trust-native-code
```

`--trust-native-code` is your consent to compile and run this code inside the server. The logic crate tests run without the engine: `(cd core && cargo test)`.

## Known gaps

What does not work yet on arismoko/horizon (each is marked `TODO(arismoko-api: ...)` in `src/lib.rs`):

- Per-sub-command operator levels are not part of the command tree (every root takes the rest of the line; the
  nodes are checked when the command runs, which keeps the plugin's own messages); tab completion lists every
  sub-command whatever the permission.
- `/sr` is left to the SkinsRestorer port unless `register_sr = true` is set in `config/noteblock.toml`; use
  `/serverradio` otherwise.
- Custom instruments play their sound file's name from the resource pack; with `use10OctaveRange: true` notes
  outside the two octaves use the `_-2`, `_-1`, `_1`, `_2` suffixed sounds.
- Not ported: holograms (no per-viewer entity API; `summon` exists, per-viewer visibility is upstream PR #18742), the
  PlaceholderAPI expansion of NBMP (no `placeholder_register`), fade in/out (NoteBlockAPI's default is off and NBMP has
  no fade key), stereo/panning (no per-player sound position in the host API). Song events and the API are plain Rust
  functions (`horizon_noteblock::api`, `engine::SongEvent`), not a mod API. Notes play at the listener, not at the eye.
- The child permission nodes follow plugin.yml's `childrens` (granted by the parent); whether Bukkit reads that
  spelling was not verified on a server.

## Licence and sources

GPL-3.0-or-later (see `LICENSE` and `NOTICE`). Derived from NoteBlockAPI (LGPL-3.0; its source was read, the logic was
written again in Rust, nothing copied) and NoteBlockMusicPlayer 1.10.0 (its behaviour follows the server's files and
its command strings; its source could not be located).

Not affiliated with NoteBlockMusicPlayer, NoteBlockAPI or their authors.
