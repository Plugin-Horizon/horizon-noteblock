# Gaps: what Horizon would need for a complete port

Nothing in the engine was edited. The port works without these, with the limits named.

## 1. The list of the server's worlds (a helper that names every loaded level)

- **What**: the list of the server's level (dimension folder) names, Bukkit's `Bukkit.getWorlds()` by name.
- **Feature**: a radio's world must exist (`world_radio_badworld`), and a player's world is taken from their dimension.
  Without the list, the port knows the level's folder name (`native_world_dir`) and derives `<level>_nether` and
  `<level>_the_end` from the player's dimension; a world of another name is refused as missing.
- **Proposed signature**: `pub(super) fn native_level_names(&self) -> Vec<String>` on `Game`, the folder names of the
  levels the server has open (the level and any other world), in the order of `native_level` (`native_entity_refs.rs`).

## 2. Sound events of custom instruments (a resource-pack sound by name)

- **What**: a way to play a sound event that a resource pack defines (a sound file a song names in its custom
  instrument record), with the same `native_play_sound_to` call.
- **Feature**: NBS custom instruments (ids from 20). Read by the port; not played. None of the server's 20 songs has
  one, so this is not needed here.
- **Proposed signature**: none needed for `native_play_sound_to` (it takes any `namespace:path` name); the open part
  is the sound event's registration from the resource pack the song names (`minecraft:` events only today).
