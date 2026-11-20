# hmod port: noteblock
Parity: 77% now / 77% with our PRs / 77% with engine changes / 77% with new SDK (77/100 items; before round 5: 71%; method: commands, nodes, config keys, 44 message keys, help texts and the features of NoteBlockAPI/NBMP counted; holograms, PlaceholderAPI expansion, stereo and the eye position need engine API; no item moves at the PR, engine or SDK steps yet, see Round 5; fade is not counted, NBMP has no fade key; blind, uncompiled)
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

Round 5 (hundred brief, blind; sources: the NBMP 1.10.0 jar's plugin.yml, config.yml and its class constant pools, read from the museum copy without writing; no class was decompiled)
- Fixed: help texts are NBMP's own strings (`radio`: header + the two `Use /radio ...` lines; `wr`/`sr`/`pr`: header, enable/disable, skip, reload, `help - shows this help`; `nbmp help`: the five root lines; `nbmp playlist help`: header + seven `Use /nbmp playlist ...` lines). Order from the class constant pools (first use).
- Fixed: `/radio toggle` and `/radio volume` from the console answer `[NoteBlockMusicPlayer] You can't use this command` (was: not handled, so Horizon answered its own unknown-command text).
- Added: plugin.yml `childrens`: a holder of `nbmp.admin` or `radio.volume` also has the child nodes, transitively (`commands::granting_nodes`, table `CHILDREN`); `lib.rs` asks the matched node and its granting parents.
- Changed: volume and mute are kept in memory after a log-out (NBMP's `PlayerVolume` map has no removal; the old code dropped them when `savePlayerVolumes` was off). With `savePlayerVolumes: true` they are written to `plugins/NoteBlockMusicPlayer/playerVolumes.yml` (`playerVolumes.<uuid>.volume` / `.mute`, keys in Bukkit's dashed UUID form) on every change and read at the start.
- Added: join action bar (NBMP `PlayerListener.onPlayerJoin` with `isActionBarShownOnAdd`): a player who was not here at the last tick gets the song line of the first enabled radio with `actionbar` that plays to them now.
- Tests (blind, not run): help texts as literal lists, console refusal, child nodes, UUID text round trip, volume file write/read with a temp copy of the fixture, join action bar.
- Not done: holograms (`summon` exists in the vocab host, but per-viewer visibility is PR #18742 and per-viewer data PR #18743, neither in the pin, and the text-display layer is not written); PlaceholderAPI expansion (`placeholder_register` is missing, GAPS_UPSTREAM.md); eye position and stereo (per-player sound API, GAPS_UPSTREAM.md).
- Unsure (blind): (1) the order of the help lines is the constant pool order, not seen in game; (2) plugin.yml spells the key `childrens`, assumed to be read by Bukkit; (3) the join radio is the first in the file's order (NBMP's priorities are not read); (4) `playerVolumes.yml` layout comes from the class strings only, no saved file was seen; (5) `positionradio.hologram` is a node in plugin.yml with no command in the executors, not declared.

Checklist round 100 (ticked = matches the original now; "API" = blocked by a missing engine API)
- [x] roots: radio, worldradio/wr, serverradio/sr (config gate), positionradio/pr, noteblockmusicplayer/nbmp
- [x] radio: help, toggle, volume (get/set, 0-100)
- [x] wr/sr/pr: help, enable, disable, skip, reload
- [x] nbmp: help, reload, playlist (help, list, songs, create, add, remove, reload)
- [ ] nbmp hologram reload/list: answer "not enabled" (ticked as answered), the holograms themselves: API
- [x] permission nodes of plugin.yml, default levels, `childrens` (round 5)
- [x] `nbmp.hologram.admin` declared; `positionradio.hologram` not declared (no command uses it): API
- [x] config keys: language, defaultPlayerVolume, savePlayerVolumes (file, round 5), use10OctaveRange, config_* files
- [x] config `holograms.yml` read; holograms: API
- [x] 44 message keys; help texts (round 5); the console line
- [x] NBS formats 0-5, pitch, volume formula, instruments incl. custom (round 3), use10OctaveRange (round 3)
- [x] real-time song clock, world/server/position radios, playlists, repeat/random, delay, action bar on song start
- [x] action bar on join (round 5, approximated radio priority)
- [x] obfuscated colour code kept, tab completion of sub-commands, radios, playlists, songs
- [x] song events and public API as plain Rust functions (not a mod API)
- [ ] fade: not in NBMP's config (not counted)
- [ ] stereo/panning: API (per-player sound position)
- [ ] eye position of the note sound: API (per-player sound)
- [ ] PlaceholderAPI expansion of NBMP: API (`placeholder_register`)
- [ ] per-sub-command operator levels in the command tree: kept out on purpose (typed usages would replace the plugin's own validation messages; checked at run time instead)
