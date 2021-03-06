// Handles loading of game assets from the original Gladiator 3.8 sources.
use actor::Actor;
use constants::{ITEM_EXIT, ITEM_PORTAL};
use inflector::Inflector;
use item::Item;
use rand;
use state::State;
use std::fs::File;
use std::io::prelude::*;
use std::str;
use world::World;
use zip;

pub const CONFIG_DIRECTORY: &str = "glad";

const ARCHIVE: &str = "glad3.8/org.openglad.gladiator.glad";
//const ARCHIVE: &str = "glad3.8/org.openglad.arenas.glad";
//const ARCHIVE: &str = "glad3.8/org.openglad.tryxian.glad";
const ORD_ACTOR: u8 = 0;
const ORD_DOOR: u8 = 1;
const ORD_ITEM_OR_EXIT: u8 = 2;
const ORD_GENERATOR: u8 = 3;
const ORD_EFFECT: u8 = 4;
const ORD_SPAWN: u8 = 5;

// See: https://github.com/openglad/openglad/blob/master/src/base.h
// NOTE: Will pop state.player_team into spawn locations.
pub fn load_world_and_spawn_team(state: &mut State) {
    state.world = World::new(CONFIG_DIRECTORY);
    state.world_desc = String::new();
    let mut archive = _get_archive();
    let filename = _resolve_filename(&mut archive, state.world_idx);
    let mut file = archive.by_name(&filename).unwrap();
    assert!(_read_c_string(3, &mut file) == "FSS");
    let version = _read_bytes(1, &mut file)[0];
    _load_world_layout(&mut state.world, &_read_c_string(8, &mut file));
    if version >= 6 {
        state.world_name = _read_c_string(30, &mut file);
    }
    let _scenario_type = _read_bytes(1, &mut file);
    if version >= 8 {
        let _cash_bonus = _read_bytes(2, &mut file);
    }
    if version >= 9 {
        let _unknown = _read_bytes(2, &mut file);
    }

    let num_objects = _read_bytes(2, &mut file); // 2 bytes for number of objects
    let num_objects = (num_objects[0] as usize) + (num_objects[1] as usize) * 256;
    for _obj_idx in 0..num_objects {
        _load_next_object(state, &mut file, version);
    }

    if !state.world_completed.contains(&state.world_idx) {
        let num_lines = _read_bytes(1, &mut file)[0];
        for _line in 0..num_lines {
            let num_chars = u64::from(_read_bytes(1, &mut file)[0]);
            state.world_desc += str::from_utf8(&_read_bytes(num_chars, &mut file)).unwrap();
            state.world_desc += "\n";
        }
        state.world_desc = state.world_desc.to_uppercase();
    } else {
        state.world_desc += "Wild dogs have picked the area clean.";
        let mut fillers = create_random_team(3, 3);
        for mut filler in fillers.drain(0..) {
            filler.teleport(&state.world);
            state.add_actor(filler);
        }
    }
}

// See: https://github.com/openglad/openglad/blob/master/src/pixdefs.h
fn _load_world_layout(world: &mut World, pix: &str) {
    let mut archive = _get_archive();
    let filename = format!("pix/{}.pix", pix).to_lowercase();
    let mut file = archive.by_name(&filename).unwrap();
    world.reshape((
        u16::from(_read_bytes(2, &mut file)[1]),
        u16::from(_read_bytes(1, &mut file)[0]),
    ));
    for index in 0..((world.size.0 * world.size.1) as usize) {
        world.tiles[index] = u16::from(_read_bytes(1, &mut file)[0]);
    }
}

pub fn create_random_team(team: usize, count: usize) -> Vec<Actor> {
    let mut actors = Vec::new();
    for kind in rand::sample(&mut rand::thread_rng(), &[0, 1, 2, 3, 5, 11, 13], count) {
        actors.push(Actor::new(*kind, 1, team, (0, 0)));
    }
    actors
}

fn _load_next_object(state: &mut State, file: &mut zip::read::ZipFile, version: u8) {
    let buffer = _read_bytes(10, file);
    let order = buffer[0];
    let mut kind = buffer[1];
    let pos = (
        (u16::from(buffer[2]) + u16::from(buffer[3]) * 256) / 16,
        (u16::from(buffer[4]) + u16::from(buffer[5]) * 256) / 16,
    );
    let team = buffer[6] as usize;
    let direction = buffer[7];
    let _command = buffer[8];
    let mut level = buffer[9] as usize;
    if version >= 7 {
        level += _read_bytes(1, file)[0] as usize * 256;
    }
    let level = level as u16; // relax range
    let name = _read_c_string(12, file);
    let _reserved_bytes = _read_bytes(10, file);

    // must load every time:
    if order == ORD_SPAWN {
        if team == 0 {
            if let Some(mut teammate) = state.player_team.pop_back() {
                teammate.pos = pos;
                state.actors.push(teammate);
                state.team_idxs.insert(team);
            }
        } else if let Some(mut enemy) = create_random_team(team, 1).pop() {
            enemy.pos = pos;
            state.actors.push(enemy);
            state.team_idxs.insert(team);
        }
        return;
    } else if order == ORD_ITEM_OR_EXIT {
        if kind == ITEM_EXIT {
            let mut exit = Item::new(kind, level, team);
            exit.pos = pos;
            state.world.exits.push(exit);
            return;
        } else if kind == ITEM_PORTAL {
            state.world.add_item(Item::new(kind, level, team), pos);
        }
    }

    if order == ORD_EFFECT || state.world_completed.contains(&state.world_idx) {
        return;
    }

    if order == ORD_DOOR || order == ORD_ITEM_OR_EXIT {
        state.world.add_item(Item::new(kind, level, team), pos);
        return;
    } else if order == ORD_GENERATOR {
        // generators are regular actors in the > 30 range:
        kind += 30;
    } else if order != ORD_ACTOR {
        return;
    }

    let mut actor = Actor::new(kind, level, team, pos);
    actor.direction = direction;
    actor.is_leader = !name.is_empty() && team != 0;
    if !name.is_empty() {
        actor.name = name.to_sentence_case();
    }
    _give_random_inventory(&mut actor);
    state.add_actor(actor);
}

fn _give_random_inventory(actor: &mut Actor) {
    if actor.team == 0 || !actor.is_leader {
        return;
    }
    let gold = Item::new(2, actor.level, actor.team);
    let silver = Item::new(3, actor.level, actor.team);
    let armor = Item::new(6, actor.level, actor.team);
    actor.inventory.push(gold);
    actor.inventory.push(silver);
    actor.inventory.push(armor);
}

fn _resolve_filename(archive: &mut zip::read::ZipArchive<File>, idx: usize) -> String {
    let filename = format!("scen/scen{}.fss", idx);
    let mut fallback = String::new();
    for ii in 0..archive.len() {
        let file = archive.by_index(ii).unwrap();
        if file.name() == filename {
            return filename;
        }
        if fallback.is_empty() && file.name().to_lowercase().ends_with(".fss") {
            fallback = file.name().to_owned();
        }
    }
    fallback
}

fn _read_bytes(amt: u64, file: &mut zip::read::ZipFile) -> Vec<u8> {
    let mut buffer = vec![0; amt as usize];
    let mut handler = file.take(amt);
    handler.read_exact(&mut buffer).expect("Failed to read.");
    buffer
}

// Interpret a c-style string with a nul terminator.
fn _read_c_string(max_amt: u64, mut file: &mut zip::read::ZipFile) -> String {
    let buffer = _read_bytes(max_amt, &mut file);
    if let Some(strlen) = buffer.iter().position(|&byte| (byte as char) < ' ') {
        return str::from_utf8(&buffer[..strlen]).unwrap().to_owned();
    }
    str::from_utf8(&buffer).unwrap().to_owned()
}

fn _get_archive() -> zip::ZipArchive<File> {
    zip::ZipArchive::new(File::open(ARCHIVE).unwrap()).unwrap()
}
