// Handles inanimate objects like exits, potions, and treasure.
use csv;

pub const PORTAL: u8 = 9;
pub const DOOR: u8 = 18;
pub const DOOR_OPEN: u8 = 19;
pub const KEY: u8 = 11;
pub const TREE: u8 = 100;
pub const DEBRIS: u8 = 200;

pub struct Item {
    pub name: String,
    pub kind: u8,
    pub team: usize,
    pub level: u16,
    pub glyph: char,
    pub color: i16,
    pub pos: (u16, u16),
    pub can_get: bool,
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
            health: 5,
        };
        item.initialize(kind);
        item
    }

    pub fn initialize(&mut self, kind: u8) {
        let mut reader = csv::Reader::from_file("config/item.csv").unwrap();
        for record in reader.decode() {
            let (kind_idx, glyph, color, name, get): (u8, String, i16, String, bool) =
                record.unwrap();
            if kind_idx == kind {
                self.kind = kind;
                self.glyph = glyph.chars().nth(0).unwrap();
                self.color = color;
                self.name = name;
                self.can_get = get;
            }
        }
    }

    pub fn damage(&mut self) {
        self.health -= 1;
        if self.health == 0 {
            self.initialize(DEBRIS);
        }
    }

    pub fn use_on(&self, other: &mut Item) -> bool {
        if other.kind == DOOR && self.kind == KEY {
            other.initialize(DOOR_OPEN);
            return true;
        }
        false
    }

    pub fn is_debris(&self) -> bool {
        self.kind == DEBRIS
    }

    pub fn step_on(&self, from: (u16, u16), to: &mut (u16, u16), team: usize, others: &[Item]) {
        match self.kind {
            PORTAL => {
                if from == *to {
                    for portal in others.iter().filter(|item| item.kind == 9) {
                        if portal.level == self.level && portal.pos != self.pos {
                            to.0 = portal.pos.0;
                            to.1 = portal.pos.1;
                        }
                    }
                }
            }
            DOOR | TREE => {
                if self.team != team {
                    to.0 = from.0;
                    to.1 = from.1;
                }
            }
            _ => {}
        }
    }
}
