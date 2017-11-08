// Handles basic item effects.
use actor::Actor;
use item::Item;

pub fn item_effect(actor: &mut Actor, item: &Item) -> bool {
    match item.kind {
        1 => actor.health = actor.max_health(),
        4 => actor.intel += 1,
        5 => actor.invis = 200,
        6 => actor.con += 1,
        7 => actor.walls = actor.walls.replace("~", ""),
        12 => actor.speed = (actor.speed / 2) + 1,
        _ => return false,
    }
    true
}
