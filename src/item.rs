// Handles inanimate objects like exits, potions, and treasure.
use constants;
use csv;

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

type ItemCSV = (
    u8,     // kind
    char,   // glyph
    i16,    // color
    String, // name
    bool,   // can_get
    bool,   // can_keep
    bool,   // can_retain
);

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
        let reader = csv::Reader::from_path("config/glad/item.csv");
        for record in reader.unwrap().deserialize() {
            let row: ItemCSV = record.unwrap();
            if row.0 == kind {
                self.kind = row.0;
                self.glyph = row.1;
                self.color = row.2;
                self.name = row.3;
                self.can_get = row.4;
                self.can_keep = row.5;
                self.can_retain = row.6;
            }
        }
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
