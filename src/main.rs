extern crate csv;
extern crate inflector;
extern crate ncurses;
extern crate rand;

#[macro_use]
mod skills;
mod actor;
mod glad_helper;
mod item;
mod item_effects;
mod plan;
mod skills_registry;
mod state;
mod view;
mod world;

use state::State;

fn main() {
    let mut game_state = State::new();
    game_state.loop_game();
}
