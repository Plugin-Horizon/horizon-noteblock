// Every tick: the radios' sounds and action bar lines go out to the players.
fn noteblock_tick(game: &mut crate::game::Game<'_>) {
    crate::game::noteblock_glue::nb_tick(game);
}
