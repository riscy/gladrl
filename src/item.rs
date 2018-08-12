// Handles inanimate objects like exits, potions, and treasure.
use csv;

const DEBRIS: u8 = 200;

pub struct Item {
    pub name: String,
    pub kind: u8,
    pub team: usize,
    pub level: u16,
    pub glyph: char,
    pub color: i16,
    pub pos: (u16, u16),
    pub can_get: bool,
    pub can_consume: bool,
    health: u16,
}

impl Item {
    pub fn new(kind: u8, pos: (u16, u16), level: u16, team: usize) -> Item {
        let mut item = Item {
            name: String::new(),
            kind: 0,
            team: team,
            level: level,
            glyph: '0',
            color: 0,
            pos: pos,
            can_get: false,
            can_consume: false,
            health: 5,
        };
        item.initialize(kind);
        item
    }

    pub fn initialize(&mut self, kind: u8) {
        let mut reader = csv::Reader::from_file("config/item.csv").unwrap();
        for record in reader.decode() {
            let row: (u8, char, i16, String, bool, bool) = record.unwrap();
            if row.0 == kind {
                self.kind = row.0;
                self.glyph = row.1;
                self.color = row.2;
                self.name = row.3;
                self.can_get = row.4;
                self.can_consume = row.5;
            }
        }
    }

    pub fn damage(&mut self) {
        self.health -= 1;
        if self.health == 0 {
            self.initialize(DEBRIS);
        }
    }

    pub fn is_debris(&self) -> bool {
        self.kind == DEBRIS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize() {
        let mut item = Item::new(DEBRIS, (0, 0), 1, 0);
        assert!(item.is_debris());
        // reinitializing changes the item's type from debris:
        item.initialize(0);
        assert!(!item.is_debris());
    }

    #[test]
    fn test_damage() {
        let mut item = Item::new(0, (0, 0), 1, 0);
        assert!(!item.is_debris());
        for _ii in 0..5 {
            item.damage();
        }
        assert!(item.is_debris());
    }
}
