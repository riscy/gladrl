extern crate csv;
extern crate inflector;
extern crate ncurses;
extern crate rand;
extern crate zip;

#[macro_use]
mod skills;
mod actor;
mod constants;
mod glad_loader;
mod item;
mod item_effects;
mod plan;
mod skills_registry;
mod state;
mod view;
mod world;

use state::State;

fn main() {
    let mut game_state = State::new(
        glad_loader::CONFIG_DIRECTORY,
        glad_loader::create_random_team,
        glad_loader::load_world_and_spawn_team,
    );
    game_state.loop_game();
    println!("Score: {}", game_state.score);
}
