// Handles basic item effects.
use actor::Actor;
use item::Item;

pub const PORTAL: u8 = 9;
pub const DOOR: u8 = 18;
pub const DOOR_OPEN: u8 = 19;
pub const KEY: u8 = 11;
pub const TREE: u8 = 100;

pub fn use_on_actor(actor: &mut Actor, kind: u8) -> bool {
    match kind {
        1 => actor.health = actor.max_health(),
        4 => actor.intel += 1,
        5 => actor.invis = 200,
        6 => actor.con += 1,
        7 => actor.walls = actor.walls.replace("~", ""),
        12 => actor.move_lag = (actor.move_lag / 2) + 1,
        _ => return false,
    }
    true
}

pub fn use_on_item(item: &mut Item, kind: u8) -> bool {
    if kind == KEY && item.kind == DOOR {
        item.initialize(DOOR_OPEN);
        return true;
    }
    false
}

pub fn use_as_portal(
    item: &Item,
    from: (u16, u16),
    to: (u16, u16),
    team: usize,
    other_items: &[Item],
) -> (u16, u16) {
    if item.kind == PORTAL && from == to {
        for portal in other_items.iter().filter(|item| item.kind == PORTAL) {
            if portal.level == item.level && portal.pos != item.pos {
                return portal.pos;
            }
        }
    } else if (item.kind == DOOR || item.kind == TREE) && item.team != team {
        return from;
    }
    to
}
