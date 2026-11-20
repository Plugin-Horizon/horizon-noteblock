# hmod port: noteblock
Parity: before 68% -> after 71% (71/100 items; method: commands, nodes, config keys, 44 message keys and the features of NoteBlockAPI/NBMP counted; holograms, fade and stereo need engine support or are out of scope; events and API are plain Rust functions now)
Round 1
- Fixed: src/lib.rs: the command check used `permission_value` in the engine's closure (1 error). Now `commands::needs(&line)` names the matched node, `permission_bool` asks it (before the engine lock; the engine closure is `Fn`, so the query cannot run inside it), and any error is a denial. The `op_level` lookup is gone (the node default replaces it).
- Fixed: src/lib.rs: added `permission_gather_nodes` with `permission_nodes()`, which declares every node named by `commands::usages()` as a Boolean node.
- Permission nodes: all nodes from `commands::usages()` (`radio.toggle`, `radio.volume`, `{worldradio,serverradio,positionradio}.{admin,enable,skip}`, `nbmp.admin`, ...) -> default = the usage's operator level (0 or 1). Declared in `permission_nodes()` / `permission_gather_nodes` in src/lib.rs.
- Gaps: none
- Unsure: (1) when several usages name one node with different levels, the first usage's level is the default (the usages agree today); (2) the `needs` lookup runs once more than the engine's own check, same result.

Round 2 (parity audit fixes)
- Fixed: console, RCON, command blocks and functions can run the commands (all permissions, as Bukkit's console); world radios may name `<level>_nether` / `<level>_the_end`; `savePlayerVolumes` keeps a player's volume across log-outs (in memory); `§k` obfuscated is kept; `tab_complete` completes sub-commands, radio, playlist and song names.
- Gaps (found): per-usage levels/nodes are not in the registered command tree; `sr` stays with SkinsRestorer; custom instruments, `use10OctaveRange` suffixed sounds, fade, stereo, events/API and holograms are not ported.

Round 3 (100 % round)
- Custom instruments (NBS ids from 20) are played by their sound file name (`.ogg` removed, `minecraft:` namespace, `pling` = the vanilla pling); `Out::Sound.sound` is now a `String`.
- `use10OctaveRange: true`: notes outside keys 33..56 (and every custom instrument) use the suffixed sound names (`InstrumentUtils.warpNameOutOfRange`).
- `/sr` is registered when `register_sr = true` in `config/noteblock.toml` (default off).
- Left as is: typed usages in the command tree (the catch-all keeps the original's own validation messages), fade/stereo/events/API, real-time clock, holograms.
- Unsure (blind): a pack's custom sound id may need a namespace the NBS file does not give.

Round 4 (blind, real time, events and API)
- Fixed: the song clock runs on real time. `Engine::tick_for` / `Radio::step_for` take the milliseconds since the last tick (`lib.rs` measures them with `Instant`, capped at 200 ms so a stalled server plays no burst); a song tick every `1000 / tempo` ms and the delay between songs in seconds, as NoteBlockAPI. `tick` and `step` (50 ms per call) are kept for the tests.
- Added: song events (`engine::SongEvent`: Started, Ended, Stopped; `Event::End` from the radio when a song runs out; a skip ends with Ended, switching off a playing radio gives Stopped). The queue keeps the newest 1024.
- Added: `horizon_noteblock::api`, plain Rust functions for other code in this binary: `player_volume`, `set_player_volume`, `has_radio`, `set_playing`, `current_song`, `skip_song`, `take_events`.
- Not done: fade in/out (NoteBlockAPI's library default is `FadeType.NONE` and NBMP exposes no fade key, so nothing to match or verify); stereo/panning (the host plays a sound for a player without a position, see GAPS_UPSTREAM.md).
- Tests (blind, not run): real-time step periods, the stall cap, the End event, the API on an unknown radio and the volume cap.
