// Handles actors' special abilities and side effects.
use actor::Actor;
use constants::{ITEM_DOOR, ITEM_DOOR_OPEN, ITEM_TELEPORT_MARKER, TILE_BLOOD, TILE_TREE};
use inflector::Inflector;
use item::Item;

use plan::Plan;
use rand::*;
use std::{cmp, u16};
use world::World; // for to_sentence_case

// hook simple skills into any part of the actor's behavior
macro_rules! passive_effect {
    ($skill:ident => $actor:expr) => {
        if $actor.has_skill(stringify!($skill)) {
            $skill($actor);
        }
    };
    ($skill:ident => $actor:expr, $( $arg:expr ),*) => {
        if $actor.has_skill(stringify!($skill)) {
            $skill($actor, $($arg,)*);
        }
    };
}

/// Set actor's current skill to this skill and return from current method
macro_rules! choose_skill {
    ($should:ident if $can:ident => $actor:expr, $( $arg:expr ),*) => {
        // split at 4 turns 'can_teleport' into 'teleport' for example
        let skill_name = &stringify!($can)[4..];
        if $actor.has_skill(skill_name) &&
            $can($actor, $($arg,)*) && $should($actor, $($arg,)*) {
                $actor.select_skill(skill_name);
                return true;
            }
    };
}

/// Use actor's current skill and return from the current method
macro_rules! use_skill {
    ($skill:ident if $can:ident => $actor:expr, $world:expr, $p:expr, $spawn:expr) => {
        if $actor.selected_skill() == stringify!($skill) {
            if $can($actor, $world, $p) {
                return $skill($actor, $world, $p, $spawn);
            }
            $actor.log_action("was too tired!");
        }
    };
}

pub fn rand_int(max: u16) -> u16 {
    thread_rng().gen_range(0, cmp::max(1, max))
}

fn _raycast(slf: &Actor, dir: u8, wld: &World, p: &Plan, len: u16) -> Option<(usize, u16)> {
    let mut pos = slf.pos;
    for dist in 0..len {
        let new_pos = wld.neighbor(pos, dir, slf.team, "#%\"'");
        if new_pos == pos {
            break;
        }
        if let Some(&team) = p.whos_at(new_pos) {
            return Some((team, dist));
        }
        pos = new_pos;
    }
    None
}

pub fn passive_spin(slf: &mut Actor) {
    if slf.time % 2 == 0 {
        slf.direction = (slf.direction + 1) % 8;
    }
}

pub fn passive_drift(slf: &mut Actor, wld: &World) {
    if slf.time % 10 == 0 {
        let drift_dir = match slf.random_seed % 4 {
            0 => slf.direction + 2,
            1 => slf.direction + 6,
            _ => slf.direction,
        };
        let pos = wld.neighbor(slf.pos, drift_dir % 8, slf.team, &slf.walls);
        if pos == slf.pos {
            return slf.lose_momentum(1);
        }
        slf.pos = pos;
    }
}

pub fn passive_descend(slf: &mut Actor, wld: &mut World) {
    slf.hurt(1, wld);
    if slf.momentum == 0 {
        slf.act_die(wld);
    }
}

pub fn passive_slam(slf: &mut Actor, action: u8, vic: &mut Actor, wld: &mut World, p: &Plan) {
    if slf.momentum != 0 && vic.is_mobile() && slf.direction == action {
        slf.log_interaction("slammed into", vic);
        vic.stun(2);
        for _ii in 0..2 {
            wld.change_tiles(vic.pos, TILE_BLOOD);
            let pos = wld.neighbor(vic.pos, slf.direction, vic.team, &vic.walls);
            match p.whos_at(pos) {
                None => vic.pos = pos,
                Some(&_team) => return,
            }
        }
    }
}

pub fn passive_whirl(slf: &mut Actor, action: u8, vic: &mut Actor) {
    let left = (slf.direction + 2) % 8;
    let right = (slf.direction + 6) % 8;
    if action == left || action == right {
        slf.log_interaction("whirled at", vic);
        vic.stun(1);
    }
}

pub fn passive_trip(slf: &mut Actor, dir: u8, vic: &mut Actor) {
    let angle = (i16::from(dir) - i16::from(slf.direction)).abs();
    if angle == 3 || angle == 4 || angle == 5 {
        slf.log_interaction("spun and tripped", vic);
        vic.stun(1);
    }
}

pub fn passive_backstab(slf: &mut Actor, dir: u8, vic: &mut Actor) {
    if vic.is_mobile() {
        let angle = (i16::from(dir) - i16::from(vic.direction)).abs();
        if angle == 0 || angle == 1 || angle == 7 {
            vic.health /= 2;
            vic.stun(2);
            slf.log_interaction("backstabbed", vic);
        } else if angle == 2 || angle == 6 {
            vic.health = vic.health * 2 / 3;
            vic.stun(1);
            slf.log_interaction("blindsided", vic);
        }
    }
}

pub fn passive_heal(slf: &mut Actor, pal: &mut Actor, _ww: &mut World) {
    if slf.mana >= 5 && pal.health < pal.max_health() {
        slf.act_exert(5, &format!("healed {}.", pal.name));
        let time = slf.time;
        pal.log_event(&format!("{} healed me.", slf.name.to_sentence_case()), time);
        pal.recover(20);
    }
}

pub fn passive_grow(slf: &Actor, wld: &mut World) {
    wld.change_tiles(slf.pos, TILE_TREE);
}

pub fn passive_aim(slf: &mut Actor, wld: &World, p: &Plan) {
    let mut closest = u16::MAX;
    let init_dir = slf.direction;
    for dir in (0..8).map(|delta_dir| (init_dir + delta_dir) % 8) {
        if let Some((team, dist)) = _raycast(slf, dir, wld, p, 10) {
            if dist < closest && team != slf.team {
                closest = dist;
                slf.direction = dir;
            }
        }
    }
}

pub fn can_sprint(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.momentum > 0 && slf.mana >= 2
}
pub fn should_sprint(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    !slf.is_hurt() && rand_int(10) == 0
}
pub fn sprint(slf: &mut Actor, wld: &mut World, p: &Plan, _spawn: &mut Vec<Actor>) {
    slf.act_exert(2, "sprinted ahead.");
    for _ii in 0..3 {
        let new_pos = wld.neighbor(slf.pos, slf.direction, slf.team, &slf.walls);
        match p.whos_at(new_pos) {
            None => slf.pos = new_pos,
            _ => break,
        }
    }
    slf.lose_momentum(1);
}

pub fn can_charge(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 4
}
pub fn should_charge(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    rand_int(60) == 0
}
pub fn charge(slf: &mut Actor, wld: &mut World, p: &Plan, _spawn: &mut Vec<Actor>) {
    slf.act_exert(4, "charged!");
    for _step in 0..2 {
        let new_pos = wld.neighbor(slf.pos, slf.direction, slf.team, &slf.walls);
        if let Some(&team) = p.whos_at(new_pos) {
            if team == slf.team {
                return slf.lose_momentum(1);
            }
        } else {
            slf.pos = new_pos;
        }
    }
}

pub fn can_leap(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 1
}
pub fn should_leap(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.is_hurt()
}
pub fn leap(slf: &mut Actor, wld: &mut World, p: &Plan, _spawn: &mut Vec<Actor>) {
    slf.act_exert(1, "leapt back!");
    slf.direction = (slf.direction + 4) % 8;
    for _step in 0..2 {
        let new_pos = wld.neighbor(slf.pos, slf.direction, slf.team, &slf.walls);
        if let Some(&team) = p.whos_at(new_pos) {
            if team == slf.team {
                return slf.lose_momentum(1);
            }
        } else {
            slf.pos = new_pos;
        }
    }
}

pub fn can_cloak(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 5
}
pub fn should_cloak(slf: &Actor, _wld: &World, p: &Plan) -> bool {
    (slf.is_hurt() && !p.is_near_enemy(slf.pos, slf.team))
}
pub fn cloak(slf: &mut Actor, _wld: &mut World, _p: &Plan, _spawn: &mut Vec<Actor>) {
    slf.act_exert(5, "started to sneak around.");
    slf.invis += 10;
}

pub fn can_shoot(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 2
}
pub fn should_shoot(slf: &Actor, wld: &World, p: &Plan) -> bool {
    match _raycast(slf, slf.direction, wld, p, slf.level as u16 + 5) {
        Some((team, _dist)) => team != slf.team,
        None => false,
    }
}
pub fn shoot(slf: &mut Actor, wld: &World, p: &Plan, spawn: &mut Vec<Actor>) {
    passive_effect!(passive_aim => slf, wld, p);
    if slf.momentum > 0 {
        slf.log_action("steadied my aim.");
        return slf.momentum = 0;
    }
    let mut shot = Actor::new(50, slf.level + 10, slf.team, slf.pos);
    slf.act_exert(2, &format!("released {}.", shot.name));
    shot.glyph = match slf.direction {
        0 | 4 => '|',
        2 | 6 => '-',
        1 | 5 => '/',
        _ => '\\',
    };
    shot.direction = slf.direction;
    spawn.push(shot);
}

pub fn can_barrage(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 6
}
pub fn should_barrage(slf: &Actor, wld: &World, p: &Plan) -> bool {
    p.is_near_enemy(slf.pos, slf.team) && should_shoot(slf, wld, p)
}
pub fn barrage(slf: &mut Actor, wld: &World, p: &Plan, spawn: &mut Vec<Actor>) {
    if slf.momentum > 0 {
        slf.log_action("steadied my aim.");
        return slf.momentum = 0;
    }
    slf.direction = (slf.direction + 7) % 8;
    for _arrow in 0..3 {
        shoot(slf, wld, p, spawn);
        slf.direction = (slf.direction + 1) % 8;
    }
}

pub fn can_boomerang(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 10
}
pub fn should_boomerang(slf: &Actor, _wld: &World, p: &Plan) -> bool {
    p.num_enemies() > 5 && p.is_near_enemy(slf.pos, slf.team) && rand_int(60) == 0
}
pub fn boomerang(slf: &mut Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    slf.act_exert(10, "threw a boomerang.");
    let mut boomerang = Actor::new(53, slf.level, slf.team, slf.pos);
    boomerang.direction = (slf.direction + 7) % 8;
    boomerang.momentum = 100;
    spawn.push(boomerang);
}

pub fn can_starburst(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 20
}
pub fn should_starburst(slf: &Actor, _wld: &World, p: &Plan) -> bool {
    !slf.is_hurt() && p.distance_to_goal(slf.pos, slf.team) < 5
}
pub fn starburst(slf: &mut Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    slf.act_exert(20, "unleashed fiery currents!");
    for direction in 0..8 {
        let mut blast = Actor::new(54, slf.level + 5, slf.team, slf.pos);
        blast.direction = direction;
        spawn.push(blast);
    }
}

pub fn can_blast(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 2
}
pub fn should_blast(slf: &Actor, wld: &World, p: &Plan) -> bool {
    !slf.is_hurt() && should_shoot(slf, wld, p)
}
pub fn blast(slf: &mut Actor, wld: &World, p: &Plan, spawn: &mut Vec<Actor>) {
    passive_effect!(passive_aim => slf, wld, p);
    slf.act_exert(2, "released an energy blast.");
    let mut blast = Actor::new(51, slf.level + 5, slf.team, slf.pos);
    blast.direction = slf.direction;
    spawn.push(blast);
}

pub fn can_teleport(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 3
}
pub fn should_teleport(slf: &Actor, _wld: &World, p: &Plan) -> bool {
    p.is_near_enemy(slf.pos, slf.team) && slf.health < slf.max_health() / 2
}
pub fn teleport(slf: &mut Actor, wld: &mut World, p: &Plan, _spawn: &mut Vec<Actor>) {
    slf.act_exert(3, "teleported.");
    if let Some(teleport_marker) = slf
        .inventory
        .iter()
        .rev()
        .position(|r| r.kind == ITEM_TELEPORT_MARKER && r.team == slf.team && r.pos != slf.pos)
    {
        // we reversed the list to ensure using the oldest teleport marker
        // above.  now we invert the index so it matches the correct item:
        let teleport_marker = slf.inventory.len() - teleport_marker - 1;
        slf.inventory[teleport_marker].damage();
        return slf.pos = slf.inventory[teleport_marker].pos;
    }
    while p.whos_at(slf.teleport(wld)).is_some() {}
}

pub fn can_teleport_marker(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 10
}
pub fn should_teleport_marker(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    false
}
pub fn teleport_marker(slf: &mut Actor, wld: &mut World, _p: &Plan, _s: &mut Vec<Actor>) {
    slf.log_action("conjured a strange glyph.");
    let pos = wld.offset(slf.pos, slf.direction);
    wld.add_item(Item::new(ITEM_TELEPORT_MARKER, slf.level, slf.team), pos);
}

pub fn can_heal(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 5
}
pub fn should_heal(slf: &Actor, wld: &World, p: &Plan) -> bool {
    match _raycast(slf, slf.direction, wld, p, 2) {
        Some((team, _dist)) => !slf.is_hurt() && team == slf.team && p.num_enemies() != 0,
        None => false,
    }
}
pub fn heal(slf: &mut Actor, _ww: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    slf.act_exert(5, "released a healing current.");
    for direction in 0..8 {
        let mut healing_current = Actor::new(52, 4, slf.team, slf.pos);
        healing_current.direction = direction;
        spawn.push(healing_current);
    }
}

pub fn can_lie(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    true
}
pub fn should_lie(slf: &Actor, _wld: &World, p: &Plan) -> bool {
    slf.team != 0 && p.distance_to_goal(slf.pos, slf.team) > 10
}
pub fn lie(slf: &mut Actor, _wld: &World, _p: &Plan, _spawn: &mut Vec<Actor>) {
    slf.log_action("crumpled to the ground.");
    slf.stun(10 + rand_int(1) as i16);
    slf.recover_fully();
}

pub fn can_summon_faerie(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 5
}
pub fn should_summon_faerie(slf: &Actor, _wld: &World, p: &Plan) -> bool {
    p.is_near_enemy(slf.pos, slf.team)
}
pub fn summon_faerie(slf: &mut Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    slf.act_exert(5, "called a faerie.");
    let mut faerie = Actor::new(55, slf.level + 5, slf.team, slf.pos);
    faerie.direction = slf.direction;
    spawn.push(faerie);
}

pub fn can_grow_tree(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 6
}
pub fn should_grow_tree(slf: &Actor, _wld: &World, p: &Plan) -> bool {
    (p.is_defending(slf.team) && p.distance_to_goal(slf.pos, slf.team) < 3)
        || (p.is_retreating(slf.team) && p.distance_to_goal(slf.pos, slf.team) > 20)
}
pub fn grow_tree(slf: &mut Actor, wld: &mut World, p: &Plan, _spawn: &mut Vec<Actor>) {
    for dir in &[0, 7, 1, 6, 2, 5, 3] {
        if !can_grow_tree(slf, wld, p) {
            break;
        }
        let pos = wld.neighbor(slf.pos, (slf.direction + dir) % 8, slf.team, &slf.walls);
        if pos != slf.pos {
            slf.act_exert(6, "grew a tree.");
            wld.add_item(Item::new(100, slf.level, slf.team), pos);
        }
    }
}

pub fn can_expand(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.health == slf.max_health()
}
pub fn should_expand(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    rand_int(5) == 0
}
pub fn expand(slf: &mut Actor, _wld: &World, _p: &Plan, _spawn: &mut Vec<Actor>) {
    let new_kind = slf.kind + 1;
    slf.initialize(new_kind);
    slf.health = slf.max_health() / 2;
}

pub fn can_multiply(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana == slf.max_mana() && slf.health == slf.max_health()
}
pub fn should_multiply(_slf: &Actor, _wld: &World, p: &Plan) -> bool {
    p.num_enemies() < 300
}
pub fn multiply(slf: &mut Actor, wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    slf.initialize(8);
    slf.health = slf.max_health() / 2;
    let pos = wld.neighbor(slf.pos, slf.direction, slf.team, &slf.walls);
    let mut new_spawn = Actor::new(8, slf.level, slf.team, pos);
    new_spawn.health /= 2;
    spawn.push(new_spawn);
}

pub fn can_spawn_elf(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    true
}
pub fn should_spawn_elf(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    rand_int(50) == 0
}
pub fn spawn_elf(slf: &Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    spawn.push(Actor::new(1, slf.level, slf.team, slf.pos));
}

pub fn can_spawn_dead(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    true
}
pub fn should_spawn_dead(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    rand_int(50) == 0
}
pub fn spawn_dead(slf: &Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    spawn.push(Actor::new(4, slf.level, slf.team, slf.pos));
}

pub fn can_spawn_mage(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    true
}
pub fn should_spawn_mage(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    rand_int(50) == 0
}
pub fn spawn_mage(slf: &Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    spawn.push(Actor::new(3, slf.level, slf.team, slf.pos));
}

pub fn can_pick(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= slf.max_mana()
}
pub fn should_pick(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    false
}
pub fn pick(slf: &mut Actor, wld: &mut World, p: &Plan, _spawn: &mut Vec<Actor>) {
    let door_pos = wld.offset(slf.pos, slf.direction);
    let cost = slf.max_mana();
    for item in wld.items.iter_mut().filter(|item| item.pos == door_pos) {
        if item.kind == ITEM_DOOR {
            item.initialize(ITEM_DOOR_OPEN);
            return slf.act_exert(cost, "picked the lock.");
        } else if item.kind == ITEM_DOOR_OPEN && p.whos_at(door_pos).is_none() {
            item.initialize(ITEM_DOOR);
            item.team = slf.team;
            return slf.act_exert(cost, "relocked the door.");
        }
    }
}
