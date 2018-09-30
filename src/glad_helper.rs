// Handles loading of game assets from the original Gladiator 3.8 sources.
use actor::Actor;
use inflector::Inflector;
use item::Item;
use state::State;
use std::fs::File;
use std::io::prelude::*;
use std::str;
use world::World;

// See: https://github.com/openglad/openglad/blob/master/src/base.h
// NOTE: Will pop state.player_team into spawn locations.
pub fn load_world_and_spawn_team(state: &mut State) {
    state.world = World::new();
    let mut file = File::open(format!("glad3.8/scen{}.fss", state.world_idx)).unwrap();
    let version = read_bytes(4, &mut file)[3]; // "FSS<version>"
    load_world_layout(
        &mut state.world,
        str::from_utf8(&read_bytes(8, &mut file)).unwrap(),
    );
    if version >= 6 {
        state.world.name = read_c_string(30, &mut file);
    }
    let _scenario_type = read_bytes(1, &mut file);
    if version >= 8 {
        let _cash_bonus = read_bytes(2, &mut file);
    }
    let num_objects = read_bytes(2, &mut file); // 2 bytes for number of objects
    let num_objects = (num_objects[0] as usize) + (num_objects[1] as usize) * 256;

    for _obj_idx in 0..num_objects {
        let buffer = read_bytes(10, &mut file);
        let mut order = buffer[0];
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
            level += read_bytes(1, &mut file)[0] as usize * 256;
        }
        let level = level as u16; // relax range
        let name = read_c_string(12, &mut file);
        let _reserved_bytes = read_bytes(10, &mut file);
        let mut is_leader = !name.is_empty() && team != 0;

        // 0=alive 1=doors 2=item|exit 3=generator 4=effects 5=spawn
        if order == 5 {
            // spawn points become teammates on team 0
            if team == 0 {
                if let Some(mut teammate) = state.player_team.pop_back() {
                    teammate.pos = pos;
                    state.actors.push(teammate);
                    state.team_idxs.insert(team);
                }
            }
            continue;
        }
        if order == 2 && kind == 8 {
            let mut exit = Item::new(kind, level, team);
            exit.pos = pos;
            state.world.exits.push(exit);
            continue;
        }
        if order == 4 || state.world_completed.contains(&state.world_idx) {
            continue;
        }
        if order == 3 {
            // generators can be regular actors in the > 30 range:
            order = 0;
            kind += 30;
            is_leader = true;
        }
        if order == 1 || order == 2 {
            let mut item = Item::new(kind, level, team);
            state.world.add_item(item, pos);
            continue;
        }
        let mut actor = Actor::new(kind, level, team, pos);
        actor.direction = direction;
        actor.is_leader = is_leader;
        if !name.is_empty() {
            actor.name = name.to_sentence_case();
        }
        give_random_inventory(&mut actor);
        assert!(actor.glyph != '?');
        assert!(actor.move_lag != 0);
        state.add_actor(actor);
    }

    if !state.world_completed.contains(&state.world_idx) {
        let num_lines = read_bytes(1, &mut file)[0];
        for _line in 0..num_lines {
            let num_chars = u64::from(read_bytes(1, &mut file)[0]);
            state.world.desc += str::from_utf8(&read_bytes(num_chars, &mut file)).unwrap();
            state.world.desc += "\n";
        }
        state.world.desc = state.world.desc.to_uppercase();
    } else {
        state.world.desc = "Wild dogs have picked the area clean.".to_owned();
    }
}

// See: https://github.com/openglad/openglad/blob/master/src/pixdefs.h
pub fn load_world_layout(world: &mut World, pix: &str) {
    let mut buffer = [0; 100_000];
    let _amt_read = File::open(format!("glad3.8/{}.pix", pix.to_lowercase()))
        .unwrap()
        .read(&mut buffer)
        .expect("Failed to open.");
    world.reshape((u16::from(buffer[1]), u16::from(buffer[2])));
    for index in 0..((world.size.0 * world.size.1) as usize) {
        world.tiles[index] = u16::from(buffer[index as usize + 3]);
    }
}

pub fn create_player_team(state: &mut State) {
    for kind in &[0, 2, 11, 1, 13, 5, 3] {
        let mut actor = Actor::new(*kind, 1, 0, (0, 0));
        actor.is_persistent = true;
        state.player_team.push_front(actor);
    }
}

fn give_random_inventory(actor: &mut Actor) {
    if actor.team == 0 || !actor.is_leader {
        return;
    }
    let gold = Item::new(2, actor.level, actor.team);
    let silver = Item::new(3, actor.level, actor.team);
    actor.inventory.push(gold);
    actor.inventory.push(silver);
}

fn read_bytes(amt: u64, file: &mut File) -> Vec<u8> {
    let mut buffer = vec![0; amt as usize];
    let mut handler = file.take(amt);
    handler.read_exact(&mut buffer).expect("Failed to read.");
    buffer
}

// Interpret a c-style string with a nul terminator.
fn read_c_string(max_amt: u64, mut file: &mut File) -> String {
    let buffer = read_bytes(max_amt, &mut file);
    if let Some(strlen) = buffer.iter().position(|&byte| (byte as char) < ' ') {
        return str::from_utf8(&buffer[..strlen]).unwrap().to_owned();
    }
    String::new()
}
