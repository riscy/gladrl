// Handles inanimate objects like exits, potions, and treasure.
use constants;
use csv;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;

pub struct Item {
    pub name: String,
    pub kind: u8,
    pub team: usize,
    pub level: u16,
    pub glyph: char,
    pub color: i16,
    pub pos: (u16, u16),
    pub can_get: bool,
    pub can_keep: bool,
    pub can_retain: bool,
    health: u16,
}

type ItemStats = (
    u8,     // kind
    char,   // glyph
    i16,    // color
    String, // name
    bool,   // can_get
    bool,   // can_keep
    bool,   // can_retain
);

thread_local!(static _ITEM_CSV_CACHE: RefCell<HashMap<u8, ItemStats>> = RefCell::new(HashMap::new()));

impl Item {
    pub fn new(kind: u8, level: u16, team: usize) -> Item {
        let mut item = Item {
            kind,
            level,
            team,
            pos: (0, 0),
            name: String::new(),
            glyph: '0',
            color: 0,
            can_get: false,
            can_keep: false,
            can_retain: false,
            health: 20,
        };
        item.initialize_as(kind);
        item
    }

    pub fn initialize_as(&mut self, kind: u8) {
        let row: ItemStats = _load_from_csv(kind, "config/glad/item.csv").unwrap();
        self.kind = row.0;
        self.glyph = row.1;
        self.color = row.2;
        self.name = row.3;
        self.can_get = row.4;
        self.can_keep = row.5;
        self.can_retain = row.6;
    }

    pub fn damage(&mut self) {
        match self.health {
            0 => (),
            1 => self.initialize_as(constants::ITEM_DEBRIS),
            _ => self.health -= 1,
        }
    }

    pub fn is_debris(&self) -> bool {
        self.kind == constants::ITEM_DEBRIS
    }
}

fn _load_from_csv(kind: u8, config: &str) -> Result<ItemStats, Box<dyn Error>> {
    return _ITEM_CSV_CACHE.with(|item_cache_cell| {
        let mut item_cache = item_cache_cell.borrow_mut();
        if let Some(item_csv) = item_cache.get(&kind) {
            return Ok(item_csv.clone());
        }
        for record in csv::Reader::from_path(&config)?.deserialize() {
            let row: ItemStats = record?;
            item_cache.insert(row.0, row.clone());
            if row.0 == kind {
                return Ok(row);
            }
        }
        panic!("Unable to load {} from {}", kind, config)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_as() {
        let mut item = Item::new(constants::ITEM_DEBRIS, 1, 0);
        assert!(item.is_debris());
        // reinitializing changes the item's type from debris:
        item.initialize_as(0);
        assert!(!item.is_debris());
    }

    #[test]
    fn test_damage() {
        let mut item = Item::new(0, 1, 0);
        assert!(!item.is_debris());
        for _ii in 0..100 {
            item.damage();
        }
        assert!(item.is_debris());
    }
}
