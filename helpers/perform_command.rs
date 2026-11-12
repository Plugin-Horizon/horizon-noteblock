// A player's command of this mod: its answer lines go back as the command's feedback.
fn noteblock_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &horizon_client::mod_api::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::noteblock_glue::nb_perform(game, player.uuid, command, ran);
}
