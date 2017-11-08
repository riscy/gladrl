 #![cfg_attr(feature="clippy", feature(plugin))]
 #![cfg_attr(feature="clippy", plugin(clippy))]

// GladRL
// Authors: Chris Rayner (dchrisrayner @ gmail)
// Created: March 2017
// Keywords: roguelikes, openglad
// Homepage: https://github.com/riscy/gladrl

// I attempted to retain the feel of the original game within some rogue-like
// mechanics.  GladRL can read the original Gladiator binary maps, enabling play
// through the original campaign and custom levels from the community.

extern crate inflector;
extern crate ncurses;
extern crate rand;
extern crate csv;

#[macro_use]
mod skills;
mod skills_registry;
mod effects;
mod actor;
mod world;
mod item;
mod plan;
mod view;
mod state;
mod glad_helper;

use state::State;

fn main() {
    let mut game_state = State::new();
    game_state.loop_game();
}
