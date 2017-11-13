extern crate inflector;
extern crate ncurses;
extern crate rand;
extern crate csv;

#[macro_use]
mod skills;
mod skills_registry;
mod actor;
mod world;
mod item;
mod item_effects;
mod plan;
mod view;
mod state;
mod glad_helper;

use state::State;

fn main() {
    let mut game_state = State::new();
    game_state.loop_game();
}
