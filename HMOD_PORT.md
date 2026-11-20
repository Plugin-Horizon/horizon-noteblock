# hmod port: noteblock
Parity: before 65% -> after 68% (68/100 items; method: commands, nodes, config keys, 44 message keys and the features of NoteBlockAPI/NBMP counted; holograms, fade, stereo, events and API need engine support or are out of scope)
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
