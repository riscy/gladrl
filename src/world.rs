// Handles the scenario's map and the items scattered around it.
use std::str;
use std::collections::HashMap;
use csv;
use item::*;
use inflector::Inflector; // for to_sentence_case

pub struct World {
    pub size: (u16, u16), // cols x rows
    pub name: String,
    pub desc: String,
    pub items: Vec<Item>,
    pub exits: Vec<Item>,
    pub tiles: Vec<u16>,
    pub log: Vec<((u16, u16), String, bool)>,
    tileset: HashMap<u16, (char, i16)>,
}

impl World {
    pub fn new() -> World {
        let mut world = World {
            size: (0, 0),
            name: String::new(),
            desc: String::new(),
            items: Vec::new(),
            exits: Vec::new(),
            tiles: Vec::new(),
            log: Vec::new(),
            tileset: HashMap::new(),
        };
        world.load_tileset("config/world.csv");
        world
    }

    pub fn neighbor(&self, from: (u16, u16), dir: u8, team: usize, walls: &str) -> (u16, u16) {
        let destination = self.offset(from, dir);
        // stepping on some items (doors, portals) can alter destination
        // or the world itself (doors opening, etc)
        let mut final_destination = destination;
        for item in self.items.iter() {
            if item.pos == final_destination {
                item.step_on(from, &mut final_destination, team, &self.items);
            }
        }
        if walls.contains(self.glyph_at(final_destination)) {
            return from;
        }
        final_destination
    }

    pub fn blood(&mut self, at: (u16, u16)) {
        let mut dir = at.0 % 8;
        for _ii in 0..at.1 % 4 {
            let pos = self.offset(at, dir as u8);
            if self.glyph_at(pos) == '.' {
                self.tiles[(pos.1 * self.size.0 + pos.0) as usize] = 200;
            }
            dir = (dir + 3) % 8;
        }
    }

    pub fn press(&mut self, from: (u16, u16), dir: u8, tools: &Vec<Item>) -> Option<Item> {
        let dest = self.offset(from, dir);
        for idx in 0..self.items.len() {
            if self.items[idx].pos == dest {
                if self.items[idx].can_get {
                    return Some(self.items.swap_remove(idx));
                }
                for tool in tools {
                    if tool.use_on(&mut self.items[idx]) {
                        self.log_global("A door swung open.", from, false);
                    }
                }
                self.items[idx].damage();
            }
        }
        None
    }

    pub fn log_global(&mut self, txt: &str, pos: (u16, u16), important: bool) {
        self.log
            .push((pos, txt.to_sentence_case().to_owned(), important));
    }

    pub fn offset(&self, from: (u16, u16), dir: u8) -> (u16, u16) {
        let mut to = (from.0 as i16, from.1 as i16);
        match dir {       // handle n/s components
            0 | 1 | 7 => to.1 -= 1,
            3 | 4 | 5 => to.1 += 1,
            _ => {}
        }
        match dir {       // handle e/w components
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
        match self.tiles.get((pos.1 * self.size.0 + pos.0) as usize) {
            Some(tile) => {
                if self.tileset.contains_key(&tile) {
                    return self.tileset[&tile];
                }
            }
            _ => {}
        }
        return ('?', 0);
    }

    fn load_tileset(&mut self, filename: &str) {
        self.tileset.clear();
        let mut reader = csv::Reader::from_file(filename).unwrap();
        for record in reader.decode() {
            let (idx, glyph, color, _desc): (u16, char, i16, String) = record.unwrap();
            self.tileset.insert(idx, (glyph, color));
        }
    }

    pub fn push_item(&mut self, new_item: Item) {
        // prevents multiple placement of the same door:
        for item in &self.items {
            if item.pos == new_item.pos && item.kind == DOOR {
                return;
            }
        }
        self.items.push(new_item);
    }
}
