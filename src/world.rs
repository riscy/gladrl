// Handles the scenario's map and the items scattered around it.
use constants::{ITEM_DOOR, ITEM_TREE};
use csv;
use item::Item;
use item_effects::{use_as_portal, use_on_item};
use std::collections::HashMap;
use std::error::Error;
use std::str;

pub struct World {
    pub size: (u16, u16), // cols x rows
    pub items: Vec<Item>,
    pub exits: Vec<Item>,
    pub tiles: Vec<u16>,
    pub log: Vec<((u16, u16), String, bool)>,
    config: String,
    tileset: HashMap<u16, (char, i16)>,
}

impl World {
    pub fn new(config: &str) -> World {
        let mut world = World {
            config: format!("config/{}/world.csv", config),
            size: (0, 0),
            items: Vec::new(),
            exits: Vec::new(),
            tiles: Vec::new(),
            log: Vec::new(),
            tileset: HashMap::new(),
        };
        world._load_tileset().unwrap();
        world
    }

    pub fn reshape(&mut self, size: (u16, u16)) {
        self.size = size;
        for _index in 0..self.size.0 * self.size.1 {
            self.tiles.push(1);
        }
    }

    pub fn neighbor(&self, from: (u16, u16), dir: u8, team: usize, walls: &str) -> (u16, u16) {
        let pos = self.offset(from, dir);
        let mut final_pos = pos;
        for item in &self.items {
            if item.pos == final_pos {
                final_pos = use_as_portal(item, from, final_pos, team, &self.items);
            }
        }
        if walls.contains(self.glyph_at(final_pos)) {
            return from;
        }
        final_pos
    }

    pub fn change_tiles(&mut self, at: (u16, u16), tile: u16) {
        let mut dir = at.0 % 8;
        for _ii in 0..at.1 % 4 {
            let pos = self.offset(at, dir as u8);
            if self.glyph_at(pos) == '.' {
                self.tiles[(pos.1 * self.size.0 + pos.0) as usize] = tile;
            }
            dir = (dir + 3) % 8;
        }
    }

    pub fn push_wall(&mut self, from: (u16, u16), dir: u8, tools: &[Item]) -> Option<Item> {
        let dest = self.offset(from, dir);
        for idx in 0..self.items.len() {
            if self.items[idx].pos == dest {
                if self.items[idx].can_get {
                    return Some(self.items.swap_remove(idx));
                }
                return self._push_item(from, idx, tools);
            }
        }
        None
    }

    fn _push_item(&mut self, from: (u16, u16), idx: usize, tools: &[Item]) -> Option<Item> {
        if self.items[idx].kind != ITEM_DOOR {
            return None;
        }
        for tool in tools {
            if use_on_item(&mut self.items[idx], tool.kind) {
                self.log_global("A door swung open.", from, false);
                return None;
            }
        }
        self.items[idx].damage();
        Some(Item::new(18, 1, 0))
    }

    pub fn log_global(&mut self, txt: &str, pos: (u16, u16), important: bool) {
        self.log.push((pos, txt.to_owned(), important));
    }

    pub fn offset(&self, from: (u16, u16), dir: u8) -> (u16, u16) {
        let mut to = (from.0 as i16, from.1 as i16);
        match dir {
            // handle n/s components
            0 | 1 | 7 => to.1 -= 1,
            3 | 4 | 5 => to.1 += 1,
            _ => {}
        }
        match dir {
            // handle e/w components
            1 | 2 | 3 => to.0 += 1,
            5 | 6 | 7 => to.0 -= 1,
            _ => {}
        }
        if self.is_out_of_bounds(to) {
            return from;
        }
        (to.0 as u16, to.1 as u16)
    }

    pub fn is_out_of_bounds(&self, pos: (i16, i16)) -> bool {
        pos.0 < 0 || pos.1 < 0 || pos.0 >= self.size.0 as i16 || pos.1 >= self.size.1 as i16
    }

    pub fn glyph_at(&self, pos: (u16, u16)) -> char {
        self.tile_at(pos).0
    }

    pub fn tile_at(&self, pos: (u16, u16)) -> (char, i16) {
        if let Some(tile) = self.tiles.get((pos.1 * self.size.0 + pos.0) as usize) {
            if self.tileset.contains_key(tile) {
                return self.tileset[tile];
            }
        }
        ('?', 0)
    }

    fn _load_tileset(&mut self) -> Result<(), Box<dyn Error>> {
        self.tileset.clear();
        let reader = csv::Reader::from_path(&self.config);
        for record in reader?.deserialize() {
            let (idx, glyph, color, _desc): (u16, char, i16, String) = record?;
            self.tileset.insert(idx, (glyph, color));
        }
        Ok(())
    }

    pub fn add_item(&mut self, mut new_item: Item, pos: (u16, u16)) {
        // prevent multiple placement of doors, trees:
        new_item.pos = pos;
        if new_item.kind == ITEM_DOOR || new_item.kind == ITEM_TREE {
            if let Some(_item) = self
                .items
                .iter()
                .find(|i| i.kind == new_item.kind && i.pos == new_item.pos)
            {
                return;
            }
        }
        self.items.push(new_item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use constants::{ITEM_DOOR, ITEM_DOOR_OPEN, ITEM_KEY, TILE_BLOOD};

    fn fixtures() -> (World, String) {
        let mut world = World::new("glad");
        world.reshape((5, 5));
        world.add_item(Item::new(ITEM_DOOR, 0, 0), (1, 1));
        world.add_item(Item::new(ITEM_KEY, 0, 0), (4, 4));
        let impassable_tiles = String::from("#");
        return (world, impassable_tiles);
    }

    #[test]
    fn test_reshape() {
        let (world, _) = fixtures();
        assert_eq!(world.tiles.len(), 25);
        assert!(world.is_out_of_bounds((6, 6)));
        for xx in 0..5 {
            for yy in 0..5 {
                assert_eq!(world.tile_at((xx, yy)), ('.', 0));
                assert_eq!(world.glyph_at((xx, yy)), '.');
            }
        }
    }

    #[test]
    fn test_offset_and_neighbor() {
        let (world, impassable_tiles) = fixtures();
        let dir = 0;
        assert_eq!(world.offset((0, 0), dir), (0, 0));
        assert_eq!(world.neighbor((0, 0), dir, 0, &impassable_tiles), (0, 0));
        assert_eq!(world.offset((2, 2), dir), (2, 1));
        assert_eq!(world.neighbor((2, 2), dir, 0, &impassable_tiles), (2, 1));
        assert_eq!(world.offset((2, 2), dir), (2, 1));
        assert_eq!(world.neighbor((2, 2), dir, 0, "."), (2, 2));
    }

    #[test]
    fn test_change_tiles() {
        let (mut world, _) = fixtures();
        world.change_tiles((2, 2), TILE_BLOOD);
        assert!(world.tiles.iter().any(|tile| tile == &TILE_BLOOD));
    }

    #[test]
    fn test_push_wall() {
        let (mut world, _) = fixtures();
        let mut actor_inventory = vec![];

        // pushing the door does not create an open door
        world.push_wall((0, 1), 2, &actor_inventory);
        assert!(!world.items.iter().any(|item| item.kind == ITEM_DOOR_OPEN));

        // reaching for the key on the ground picks it up:
        let treasure = world.push_wall((3, 4), 2, &actor_inventory).unwrap();
        assert_eq!(world.items.len(), 1);
        assert_eq!(treasure.kind, ITEM_KEY);

        // pushing against the locked door, with the key, opens it:
        actor_inventory.push(treasure);
        world.push_wall((0, 1), 2, &actor_inventory);
        assert!(world.items[0].kind == ITEM_DOOR_OPEN);
    }
}
