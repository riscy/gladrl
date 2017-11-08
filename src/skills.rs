// Handles actors' special abilities and side effects.
use std::{cmp, u16};
use rand::*;
use actor::Actor;
use world::World;
use item::Item;
use plan::Plan;
use inflector::Inflector; // for to_sentence_case

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

fn raycast(slf: &Actor, dir: u8, wld: &World, p: &Plan, len: u16) -> Option<(usize, u16)> {
    let mut pos = slf.pos;
    for dist in 0..len {
        let new_pos = wld.neighbor(pos, dir, slf.team, &slf.walls);
        if new_pos == pos {
            break;
        }
        match p.whos_at(new_pos) {
            Some(&team) => return Some((team, dist)),
            None => {}
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
    if slf.time % 7 == 0 {
        let drift_dir = match slf.random_state % 4 {
            0 => slf.direction + 2,
            1 => slf.direction + 6,
            _ => slf.direction,
        };
        let pos = wld.neighbor(slf.pos, drift_dir % 8, slf.team, &slf.walls);
        match pos != slf.pos {
            true => slf.pos = pos,
            false => slf.lose_momentum(1),
        }
    }
}

pub fn passive_descend(slf: &mut Actor, wld: &mut World) {
    slf.hurt(1, wld);
    if slf.momentum == 0 {
        slf.act_die(wld);
    }
}

pub fn passive_charge(slf: &mut Actor, action: u8, vic: &mut Actor, wld: &World, p: &Plan) {
    if slf.momentum != 0 && vic.is_mobile() && slf.direction == action {
        slf.log_interaction("slammed into", vic);
        vic.stun(2);
        let pos = wld.neighbor(vic.pos, slf.direction, vic.team, &vic.walls);
        match p.whos_at(pos) {
            None => vic.pos = pos,
            Some(_team) => return,
        }
    }
}

pub fn passive_whirl(slf: &mut Actor, action: u8, vic: &mut Actor) {
    let left = (slf.direction + 2) % 8;
    let right = (slf.direction + 6) % 8;
    if action == left || action == right {
        slf.log_interaction("whirled at", vic);
        vic.stun(3);
    }
}

pub fn passive_trip(slf: &mut Actor, dir: u8, vic: &mut Actor) {
    let angle = (dir as i16 - slf.direction as i16).abs();
    if angle == 3 || angle == 4 || angle == 5 {
        slf.log_interaction("spun and tripped", vic);
        vic.stun(1);
    }
}

pub fn passive_backstab(slf: &mut Actor, dir: u8, vic: &mut Actor) {
    if vic.is_mobile() {
        let angle = (dir as i16 - vic.direction as i16).abs();
        if angle == 0 || angle == 1 || angle == 7 {
            vic.health = vic.health / 2;
            vic.stun(2);
            slf.log_action(&format!("stabbed {} from behind!", vic.name))
        } else if angle == 2 || angle == 6 {
            vic.health = vic.health * 2 / 3;
            vic.stun(1);
            slf.log_action(&format!("stabbed {} in the arm!", vic.name))
        }
    }
}

pub fn passive_heal(slf: &mut Actor, pal: &mut Actor, _ww: &mut World) {
    if slf.mana >= 5 {
        if pal.health < pal.max_health() {
            slf.exert(5, &format!("healed {}", pal.name));
            let time = slf.time;
            pal.log_event(&format!("{} healed me.", slf.name.to_sentence_case()), time);
            pal.recover(20);
        }
    }
}

pub fn passive_aim(slf: &mut Actor, wld: &World, p: &Plan) {
    let mut closest = u16::MAX;
    let init_dir = slf.direction;
    for dir in (0..8).map(|delta_dir| (init_dir + delta_dir) % 8) {
        match raycast(slf, dir, wld, p, 10) {
            Some((team, dist)) => {
                if dist < closest && team != slf.team {
                    closest = dist;
                    slf.direction = dir;
                }
            }
            None => {}
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
    slf.exert(2, "sprinted ahead");
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
    rand_int(30) == 0
}
pub fn charge(slf: &mut Actor, wld: &mut World, p: &Plan, _spawn: &mut Vec<Actor>) {
    slf.exert(4, "charged!");
    for _step in 0..2 {
        let new_pos = wld.neighbor(slf.pos, slf.direction, slf.team, &slf.walls);
        match p.whos_at(new_pos) {
            Some(&team) => {
                if team == slf.team {
                    return slf.lose_momentum(1);
                }
                return slf.gain_momentum(1);
            }
            None => slf.pos = new_pos,
        }
    }
}

pub fn can_cloak(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 5
}
pub fn should_cloak(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.is_hurt() || rand_int(20) == 0
}
pub fn cloak(slf: &mut Actor, _wld: &mut World, _p: &Plan, _spawn: &mut Vec<Actor>) {
    slf.exert(5, "started to sneak around.");
    slf.invis += 10;
}

pub fn can_shoot(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 2
}
pub fn should_shoot(slf: &Actor, wld: &World, p: &Plan) -> bool {
    match raycast(slf, slf.direction, wld, p, slf.level as u16 + 10) {
        Some((team, _dist)) => team != slf.team,
        None => false,
    }
}
pub fn shoot(slf: &mut Actor, wld: &World, p: &Plan, spawn: &mut Vec<Actor>) {
    passive_effect!(passive_aim => slf, wld, p);
    slf.exert(2, "released an arrow");
    spawn.push(Actor::new(50, slf.level + 10, slf.team, slf.pos, slf.direction));
    spawn.last_mut().unwrap().glyph = match slf.direction {
        0 | 4 => '|',
        2 | 6 => '-',
        1 | 5 => '/',
        _ => '\\',
    };
}

pub fn can_barrage(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 6
}
pub fn should_barrage(slf: &Actor, wld: &World, p: &Plan) -> bool {
    slf.is_in_danger(p) && should_shoot(slf, wld, p)
}
pub fn barrage(slf: &mut Actor, wld: &World, p: &Plan, spawn: &mut Vec<Actor>) {
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
    !p.dist_is_greater_than(slf.pos, slf.team, 3)
}
pub fn boomerang(slf: &mut Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    slf.exert(10, "threw a boomerang");
    let dir = (slf.direction + 7) % 8;
    spawn.push(Actor::new(53, slf.level, slf.team, slf.pos, dir));
    spawn.last_mut().unwrap().momentum = 100;
}

pub fn can_warp_space(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 20
}
pub fn should_warp_space(slf: &Actor, _wld: &World, p: &Plan) -> bool {
    slf.is_in_danger(p) && !slf.is_hurt()
}
pub fn warp_space(slf: &mut Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    slf.exert(20, "casted 'warp space'");
    for dir in 0..8 {
        spawn.push(Actor::new(54, slf.level + 5, slf.team, slf.pos, dir));
    }
}

pub fn can_blast(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 5
}
pub fn should_blast(slf: &Actor, wld: &World, p: &Plan) -> bool {
    should_shoot(slf, wld, p)
}
pub fn blast(slf: &mut Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    slf.exert(5, "released an energy blast");
    spawn.push(Actor::new(51, slf.level + 5, slf.team, slf.pos, slf.direction));
}

pub fn can_teleport(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 3
}
pub fn should_teleport(slf: &Actor, _wld: &World, p: &Plan) -> bool {
    slf.is_in_danger(p) && slf.health < slf.max_health() / 2
}
pub fn teleport(slf: &mut Actor, wld: &World, p: &Plan, _spawn: &mut Vec<Actor>) {
    loop {
        let pos = (rand_int(wld.size.0 as u16), rand_int(wld.size.1 as u16));
        if p.whos_at(pos).is_none() && !slf.walls.contains(wld.glyph_at(pos)) {
            slf.exert(3, "teleported");
            return slf.pos = pos;
        }
    }
}

pub fn can_heal(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 5
}
pub fn should_heal(slf: &Actor, wld: &World, p: &Plan) -> bool {
    match raycast(slf, slf.direction, wld, p, 2) {
        Some((team, _dist)) => !slf.is_hurt() && team == slf.team,
        None => false,
    }
}
pub fn heal(slf: &mut Actor, _ww: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    slf.exert(5, "released a healing current");
    for direction in 0..8 {
        spawn.push(Actor::new(52, 4, slf.team, slf.pos, direction));
    }
}

pub fn can_lie(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    true
}
pub fn should_lie(slf: &Actor, _wld: &World, p: &Plan) -> bool {
    slf.team != 0 && p.distance_to_target(slf.pos, slf.team) > 10
}
pub fn lie(slf: &mut Actor, _wld: &World, _p: &Plan, _spawn: &mut Vec<Actor>) {
    slf.log_action("crumpled to the ground.");
    slf.stun(10 + rand_int(1) as i16);
    slf.recover(10);
}

pub fn can_summon_faerie(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 5
}
pub fn should_summon_faerie(slf: &Actor, _wld: &World, p: &Plan) -> bool {
    slf.is_in_danger(p)
}
pub fn summon_faerie(slf: &mut Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    slf.exert(5, "called a faerie");
    spawn.push(Actor::new(7, slf.level + 5, slf.team, slf.pos, slf.direction));
}

pub fn can_grow_tree(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.mana >= 6
}
pub fn should_grow_tree(slf: &Actor, wld: &World, p: &Plan) -> bool {
    should_shoot(slf, wld, p)
}
pub fn grow_tree(slf: &mut Actor, wld: &mut World, p: &Plan, _spawn: &mut Vec<Actor>) {
    for dir in vec![0, 7, 1, 6, 2, 5, 3] {
        if !can_grow_tree(slf, wld, p) {
            break;
        }
        let dir = (slf.direction + dir) % 8;
        let pos = wld.neighbor(slf.pos, dir, slf.team, &slf.walls);
        slf.exert(6, "grew a tree");
        wld.items.push(Item::new(100, pos, slf.level, slf.team));
    }
}

pub fn can_expand(slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    slf.health == slf.max_health()
}
pub fn should_expand(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    rand_int(5) == 0
}
pub fn expand(slf: &mut Actor, _wld: &World, _p: &Plan, _spawn: &mut Vec<Actor>) {
    slf.log_action("expanded.");
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
    spawn.push(Actor::new(8, slf.level, slf.team, pos, slf.direction));
    spawn.last_mut().unwrap().health /= 2;
    slf.log_action("split in half.");
}

pub fn can_spawn_elf(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    true
}
pub fn should_spawn_elf(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    rand_int(50) == 0
}
pub fn spawn_elf(slf: &Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    spawn.push(Actor::new(1, slf.level, slf.team, slf.pos, 0));
}

pub fn can_spawn_dead(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    true
}
pub fn should_spawn_dead(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    rand_int(50) == 0
}
pub fn spawn_dead(slf: &Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    spawn.push(Actor::new(4, slf.level, slf.team, slf.pos, 0));
}

pub fn can_spawn_mage(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    true
}
pub fn should_spawn_mage(_slf: &Actor, _wld: &World, _p: &Plan) -> bool {
    rand_int(50) == 0
}
pub fn spawn_mage(slf: &Actor, _wld: &World, _p: &Plan, spawn: &mut Vec<Actor>) {
    spawn.push(Actor::new(3, slf.level, slf.team, slf.pos, 0));
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
        if item.kind == 18 {
            item.initialize(19);
            return slf.exert(cost, "picked the lock");
        } else if item.kind == 19 && p.whos_at(door_pos).is_none() {
            item.initialize(18);
            item.team = slf.team;
            return slf.exert(cost, "relocked the door");
        }
    }
}
