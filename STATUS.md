# Status

Written blind on 2026-11-11: no `cargo` run has happened (the port's tests are written, not run, per the task). Base:
Horizon `3b225249fc` (the worktree the glue was read against). The engine names the glue uses
(`native_play_sound_to`, `native_action_bar`, `native_server_players`, `native_world_dir`, `native_config_dir`,
`native_feedback`, the `tick`, `register_commands` and `perform_command` hooks) were checked by grep against that
revision; the bundle has never been applied.

Legend: **done** = written and covered by a test in `core/`; **partly** = written with the limit named; **not done** =
absent, with the reason.

## Work items (the lobby audit, section 4, kept with the main session)

| # | Item | State | Notes |
|---|---|---|---|
| 1 | NBS reader: legacy v0 and v1-v5, all header fields, layers, custom instruments, trailing padding | done | `nbs::parse`; tests in `nbs.rs` (formats 0, 1, 2, 3, 4, 5, zero layers, custom ids, padding, truncation, CR in titles). `examples/check_original.rs` prints version, length, tempo, layers, notes |
| 2 | Instruments to sound names; pitch formula; volume | done | `instrument.rs`: ids 0-19 vanilla (see deviation 1), negative = harp, 20+ custom (not played). `pitch.rs`: 2401-entry table, `transposed` and `in_octave`, `volume` |
| 3 | Clock (tps = tempo/100); sending the sounds; global radios; position radios; playlists (repeat, random, delay); titles from `songs.yml`; action bar; messages | done | `radio.rs` (clock, order, delay), `engine.rs` (audience, sounds, action bar, checks, enable/disable/skip/reload), `library.rs` (titles), `messages.rs` |
| 4 | Read all original files with the vendored horizon-yaml, start nothing on this server | done | `config.rs` reads `config.yml`, `playlists.yml`, `globalRadio.yml`, `positionRadio.yml` (incl. the stray `radios: {}`), `songs.yml`, `holograms.yml`, `messages_<language>.yml`. Test: `fixture_flags_start_nothing` (fixture with the same flags, 400 ticks, no output) |
| 5 | Commands and permissions as 1.10.0 | partly | Roots, aliases and nodes from `plugin.yml`: done. Sub-command gates without a node of their own (`songs`, `remove`, `hologram`, `help`) are inferred (see deviation 4). Help texts reconstructed from the class strings (layout not verified). Console use not ported |
| 5b | Registered as native commands | done (blind) | `helpers/register_commands.rs`, `perform_command.rs`; `commands::usages()` |
| 5c | `holograms.yml` parsed, inert | done | `config::read_holograms`; `/nbmp hologram ...` answers "not enabled" |

## Deviations (decided here, to be checked by the owner)

1. **Instrument ids 16 to 19** (trumpets) play their vanilla sounds. The task listed 0 to 15; NoteBlockAPI 1.7.0 on
   Minecraft 26.x starts custom instruments at 20 (`getCustomInstrumentFirstIndex`), so 16 to 19 are vanilla there.
   None of the server's 20 songs uses them (ids used: 0, 1, 2, 3, 4, 5, 6).
2. **Titles** collapse their whitespace (a `|2-` block scalar of `songs.yml` becomes "Carol of the Bells" with no leading
   line break).
3. **Position radio centre** is the `x y z` of the file (NBMP's block-centre offset is not applied). Sounds go to
   the player's position (`native_play_sound_to` with no point: where they stand), not the eye position of MonoMode.
4. **Gates inferred** (no node in `plugin.yml`): `playlist songs` and `playlist list` use `nbmp.playlist.list`;
   `playlist remove` uses `nbmp.playlist.admin`; `help` and `reload` of the radio roots use `<kind>radio.admin`;
   `nbmp reload` uses `nbmp.admin`; `playlist help` uses `nbmp.playlist.admin`. Operators (level 1) where
   `plugin.yml` says `op`; players for `radio.toggle` and `radio.volume`.
5. **`sr`** means `serverradio` (registered first in `plugin.yml`, Bukkit's order). `radio` has no alias.
6. **Messages without a key** in `messages_en.yml`: `/radio toggle` answers nothing; `/radio volume` with a bad
   level answers the help; `/nbmp hologram list` and `reload` answer "Holograms are not enabled". Replies of commands
   carry the file's `prefix`, action bar lines do not.
7. **Enable state is not written** to the radio files (rule 5: the port only reads them); `playlists.yml` is written
   on playlist create, add and remove (NBMP writes it too). A playlist change reaches a radio at the next reload
   (reload restarts all radios and re-reads every file).
8. **use10OctaveRange: true** plays the base-octave pitch without NoteBlockAPI's warped sound names (not used on
   this server; logged at load).
9. **World names**: a player's world is the level's folder name in the overworld, `<level>_nether`, `<level>_the_end`.
   A radio's world must be the level name (the only world the glue knows, see GAPS 1).
10. **Players** are tracked by the tick: a player who leaves loses mute and volume (`savePlayerVolumes: false`).

## Not done

- Console commands (the `perform_console_command` hook exists; not registered).
- Custom instrument sounds (ids from 20): read, not played (see GAPS 2).
- Holograms (no provider on the server, by design).
- NoteBlockAPI's `isStereo` / panning and the `MonoStereoMode` channel: notes are played mono.
- `RepeatMode.ONE` and NoteBlockAPI's events (no listeners here).

## Test first, once a build is possible

1. `cd core && CARGO_TARGET_DIR=<repo>/target cargo test`: nbs, instrument, pitch, messages, config, library, radio,
   engine, commands (every test is named in its module).
2. `cargo run --example check_original -- <the server's plugins/NoteBlockMusicPlayer/*.yml and songs/*.nbs>` (main
   session, read-only): the `.nbs` lines must give format 0, the audit's lengths and note counts; the `.yml` lines must
   list no unread key except the stray ones the port ignores on purpose.
3. `horizon-mixins apply` with this bundle; compile errors in `helpers/glue.rs` first (`Game::native_config_dir`,
   `native_world_dir`, `native_server_players`, `native_play_sound_to`, `native_action_bar`, `native_feedback`).
4. On a throwaway server with a probe client: a world radio started by hand plays the note sounds (one `Sound` per
   note per listener, `minecraft:block.note_block.*`), the action bar line appears once per song start, and the
   sounds stop when the radio is disabled.
5. Budget: peak 15 notes in one tick for one listener (Waltz of the Flowers) times the listeners; check the tick time.
