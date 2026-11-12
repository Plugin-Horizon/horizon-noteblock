//! NoteBlockMusicPlayer 1.10.0 with NoteBlockAPI 1.7.0 for Horizon: Note Block Studio songs (`.nbs`, formats 0 to 5)
//! played as vanilla note block sounds to the listeners of global (world, server) and position radios, with
//! playlists, song titles from `songs.yml`, the action bar and the commands of the original.
//!
//! The logic is free of game types and is unit tested on its own. The bundle's glue feeds it the players
//! and sends the sounds and action bar lines it answers with. It reads the original plugin folder
//! (`plugins/NoteBlockMusicPlayer/`) as it is.

pub mod commands;
pub mod config;
pub mod engine;
pub mod instrument;
pub mod library;
pub mod messages;
pub mod nbs;
pub mod pitch;
pub mod radio;
pub mod yaml;
