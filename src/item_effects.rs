// Handles basic item effects.
use actor::Actor;
use constants::{ITEM_DOOR, ITEM_DOOR_OPEN, ITEM_KEY, ITEM_PORTAL, ITEM_TREE};
use item::Item;

pub fn use_on_actor(actor: &mut Actor, kind: u8) -> bool {
    match kind {
        1 => actor.health = actor.max_health(),
        2 => {} // gold
        3 => {} // silver
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
    if kind == ITEM_KEY && item.kind == ITEM_DOOR {
        item.initialize(ITEM_DOOR_OPEN);
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
    if item.kind == ITEM_PORTAL && from == to {
        for portal in other_items.iter().filter(|item| item.kind == ITEM_PORTAL) {
            if portal.level == item.level && portal.pos != item.pos {
                return portal.pos;
            }
        }
    } else if (item.kind == ITEM_DOOR || item.kind == ITEM_TREE) && item.team != team {
        return from;
    }
    to
}
