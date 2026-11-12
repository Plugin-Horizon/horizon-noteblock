// The commands of NoteBlockMusicPlayer: `/radio`, `/worldradio` (`wr`), `/serverradio` (`sr`), `/positionradio`
// (`pr`) and `/noteblockmusicplayer` (`nbmp`), with the permission nodes of plugin.yml.
fn noteblock_register_commands(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    crate::game::noteblock_glue::nb_register(commands);
}
