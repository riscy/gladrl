// Handles moving objects like living entities and projectiles.
use std::cmp;
use csv;
use inflector::Inflector;
use plan::Plan;
use world::World;
use skills::*;
use skills_registry::{choose_skill, use_skill};
use item::Item;
use item_effects::use_on_actor;

pub const MOVE_ACTIONS: [u8; 9] = [0, 1, 2, 3, 4, 5, 6, 7, DO_WAIT];
pub const TURN_ACTIONS: [u8; 8] = [16, 17, 18, 19, 20, 21, 22, 23];

const DO_WAIT: u8 = 8;
const DO_SKILL: u8 = 30;
const DO_DROP: u8 = 40;

pub struct Actor {
    pub name: String,
    pub kind: u8,
    pub team: usize,
    pub glyph: char,
    pub pos: (u16, u16),
    pub direction: u8,
    pub time: u32,

    pub level: u16,
    pub health: u16,
    pub speed: u16,
    pub mana: u16,
    pub intel: u16,
    pub con: u16,
    pub strength: u16,
    pub walls: String,
    pub stun_counter: i16,
    pub invis: i16,
    pub selected_skill: usize,
    pub momentum: u8,

    pub log: Vec<(u32, String, usize)>,
    pub random_state: u16,
    pub skills: Vec<String>,
    inventory: Vec<Item>,

    pub is_leader: bool,
    pub is_persistent: bool,
}

impl Actor {
    pub fn new(kind: u8, level: u16, team: usize, pos: (u16, u16), dir: u8) -> Actor {
        let mut actor = Actor {
            kind: 0,
            pos: pos,
            level: level,
            health: 1,
            strength: 1,
            con: 1,
            intel: 1,
            mana: 1,
            team: team,
            direction: dir,
            name: String::new(),
            walls: String::new(),
            random_state: rand_int(200),
            is_leader: false,
            stun_counter: 0,
            glyph: '?',
            speed: 1,
            is_persistent: false,
            momentum: 0,
            selected_skill: 0,
            skills: Vec::new(),
            time: 1,
            log: Vec::new(),
            inventory: Vec::new(),
            invis: 0,
        };
        actor.initialize(kind);
        actor.restore();
        actor
    }

    pub fn initialize(&mut self, kind: u8) {
        let mut reader = csv::Reader::from_file("config/actor.csv").unwrap();
        for record in reader.decode() {
            let row: (u8, char, String, String, u16, String, u16, u16, u16, u16) = record.unwrap();
            if row.0 == kind {
                self.kind = kind;
                self.glyph = row.1;
                self.walls = row.2;
                if self.name.is_empty() {
                    self.name = row.3;
                }
                self.speed = row.4;
                self.strength = row.6;
                self.con = row.8;
                self.intel = row.9;
                self.skills.clear();
                for skill in row.5.split(' ') {
                    self.skills.push(skill.into());
                }
                for kind in self.inventory.iter().map(|it| it.kind).collect::<Vec<u8>>() {
                    use_on_actor(self, kind);
                }
                break;
            }
        }
    }

    pub fn glyph(&self) -> char {
        if !self.is_alive() {
            return 'x';
        } else if self.stun_counter > 0 {
            return self.glyph.to_lowercase().next().unwrap();
        }
        self.glyph
    }

    pub fn max_health(&self) -> u16 {
        cmp::max(1, self.con * self.level)
    }

    pub fn max_mana(&self) -> u16 {
        cmp::max(1, self.intel * self.level)
    }

    pub fn log_event(&mut self, txt: &str, time: u32) {
        match self.log.last_mut() {
            Some(last_log) => {
                if last_log.1 == txt {
                    last_log.0 = time;
                    return last_log.2 += 1;
                }
            }
            _ => {}
        };
        self.log.push((time, txt.to_owned(), 1));
    }

    pub fn log_action(&mut self, verb: &str) {
        let time = self.time;
        self.log_event(&format!("I {}", verb), time);
    }

    pub fn log_interaction(&mut self, verb: &str, other: &mut Actor) {
        let time = cmp::max(self.time, other.time);
        let msg = format!("I {} {}.", verb, other.name);
        self.log_event(&msg, time);
        let msg = format!("{} {} me!", self.name.to_sentence_case(), verb);
        other.log_event(&msg, time);
    }

    pub fn select_skill(&mut self, skill: &str) {
        for (idx, self_skill) in self.skills.iter().enumerate() {
            if self_skill == skill {
                return self.selected_skill = idx;
            }
        }
    }

    pub fn selected_skill(&self) -> String {
        if self.skills.len() > 0 {
            return self.skills[self.selected_skill].to_owned();
        }
        String::new()
    }

    pub fn next_skill(&mut self) {
        self.selected_skill = (self.selected_skill + 1) % self.skills.len();
        for _idx in 0..self.skills.len() {
            if self.selected_skill().starts_with("passive") {
                self.selected_skill = (self.selected_skill + 1) % self.skills.len();
            }
        }
        let skill = self.selected_skill();
        self.log_action(&format!("switched to {}.", skill));
    }

    pub fn choose(&mut self, world: &World, plan: &Plan) -> u8 {
        if self.is_projectile() {
            return self.direction;
        } else if choose_skill(self, world, plan) {
            return DO_SKILL;
        }
        self.choose_move(world, plan)
    }

    fn choose_move(&self, world: &World, plan: &Plan) -> u8 {
        let start_mv = self.choose_preferred_dir();
        let (mut best_gradient, mut best_direction) = (0, start_mv);
        for mv in MOVE_ACTIONS.iter().map(|offset| (start_mv + offset) % 9) {
            let mut pos = world.neighbor(self.pos, mv, self.team, &self.walls);
            let movement = pos != self.pos;
            if !movement {
                pos = world.offset(self.pos, mv)
            }
            if !self.is_hurt() {
                match plan.whos_at(pos) {
                    Some(&team) => {
                        if team != self.team || (pos != self.pos && self.can_help()) {
                            return mv;
                        } else if !self.can_displace() {
                            continue;
                        }
                    }
                    None => {}
                }
            }
            if movement {
                let retreat = self.is_hurt();
                let gradient = plan.gradient(self.pos, pos, self.team, retreat);
                if gradient > best_gradient {
                    best_direction = mv;
                    best_gradient = gradient;
                }
            }
        }
        best_direction
    }

    fn choose_preferred_dir(&self) -> u8 {
        if !self.is_projectile() && rand_int(5) == 0 {
            return rand_int(8) as u8;
        }
        self.direction
    }

    pub fn act(&mut self,
               mv: u8,
               wld: &mut World,
               plan: &Plan,
               others: (&mut [Actor], &mut [Actor]),
               spawn: &mut Vec<Actor>) {
        if self.stun_counter == 0 {
            match mv {
                DO_SKILL => use_skill(self, wld, plan, spawn),
                DO_DROP => self.act_drop_item(wld),
                _ => self.act_move(mv, wld, plan, others),
            };
        }
        self.side_effects(wld);
    }

    fn act_move(&mut self,
                mv: u8,
                wld: &mut World,
                plan: &Plan,
                other: (&mut [Actor], &mut [Actor])) {
        if !self.is_mobile() {
            return self.act_change_direction(mv);
        }
        let mut pos = wld.neighbor(self.pos, mv, self.team, &self.walls);
        let movement = self.pos != pos;
        if !movement {
            pos = wld.offset(self.pos, mv);
            self.lose_momentum(1);
        }
        match plan.whos_at(pos) {
            Some(&_team) => {
                for other in other.0.iter_mut().filter(|xx| xx.is_blocking(pos)) {
                    self.act_touch(other, wld, mv, plan);
                }
                for other in other.1.iter_mut().filter(|xx| xx.is_blocking(pos)) {
                    self.act_touch(other, wld, mv, plan);
                }
            }
            None => {
                if !movement {
                    if MOVE_ACTIONS.contains(&mv) {
                        self.act_push_wall(wld, mv);
                    }
                } else {
                    self.pos = pos;
                    self.gain_momentum(1);
                }
            }
        }
        self.act_change_direction(mv);
        passive_effect!(passive_aim => self, wld, plan);
    }

    fn act_push_wall(&mut self, world: &mut World, action: u8) {
        match world.push_wall(self.pos, action, &self.inventory) {
            Some(treasure) => {
                self.log_action(&format!("grabbed {}.", treasure.name));
                use_on_actor(self, treasure.kind);
                if !treasure.can_consume {
                    self.inventory.push(treasure);
                }
            }
            None => {
                if self.is_leader {
                    self.log_action("couldn't go any further.");
                }
            }
        }
    }

    fn act_change_direction(&mut self, dir: u8) {
        if MOVE_ACTIONS.contains(&dir) {
            self.direction = dir % 8;
        } else if TURN_ACTIONS.contains(&dir) {
            self.log_action("turned in place.");
            self.direction = dir % 8;
        }
    }

    fn act_get(&mut self, world: &mut World) {
        let mut idx = 0;
        while idx < world.items.len() {
            if self.pos == world.items[idx].pos && world.items[idx].can_get {
                let item = world.items.remove(idx);
                self.log_action(&format!("found {}.", item.name));
                use_on_actor(self, item.kind);
                if !item.can_consume {
                    self.inventory.push(item);
                }
                continue;
            }
            idx += 1;
        }
    }

    fn act_drop_item(&mut self, world: &mut World) {
        match self.inventory.pop() {
            Some(mut item) => {
                self.log_action(&format!("dropped {}.", item.name));
                item.pos = world.neighbor(self.pos, self.direction, self.team, "");
                world.add_item(item);
            }
            None => self.log_action("had nothing to drop."),
        }
        let kind = self.kind;
        self.initialize(kind);
    }

    fn act_drop_all(&mut self, world: &mut World) {
        while self.inventory.len() > 0 {
            self.act_drop_item(world);
            let new_direction = self.direction + 1;
            self.act_change_direction(new_direction);
        }
    }

    fn act_touch(&mut self, other: &mut Actor, world: &mut World, action: u8, plan: &Plan) {
        if other.is_enemy_of(self.team) {
            return self.act_hit(other, action, world, plan);
        } else if self.can_displace() && other.is_mobile() {
            return self.act_displace(other, world);
        }
        self.act_help(other, world)
    }

    fn act_displace(&mut self, other: &mut Actor, world: &mut World) {
        if !self.walls.contains(world.glyph_at(other.pos)) &&
           !other.walls.contains(world.glyph_at(self.pos)) {
            let new_pos = other.pos;
            other.pos = (self.pos.0, self.pos.1);
            self.pos = new_pos;
            self.lose_momentum(1);
            self.log_interaction("displaced", other);
        }
        passive_effect!(passive_heal => self, other, world);
    }

    fn act_help(&mut self, other: &mut Actor, world: &mut World) {
        passive_effect!(passive_heal => self, other, world);
        if other.stun_counter > 0 && !self.is_projectile() {
            other.stun_counter = 0;
            self.log_action(&format!("hoisted {} up.", other.name));
            other.log_action(&format!("was hoisted up by {}.", self.name));
        }
        self.lose_momentum(1);
    }

    fn act_hit(&mut self, other: &mut Actor, action: u8, world: &mut World, p: &Plan) {
        passive_effect!(passive_trip => self, action, other);
        passive_effect!(passive_whirl => self, action, other);
        passive_effect!(passive_backstab => self, action, other);
        passive_effect!(passive_charge => self, action, other, world, p);
        self.log_interaction("hit", other);
        self.lose_momentum(1);
        other.hurt(self.strength * self.level, world);
        if self.momentum > 0 {
            self.pos = other.pos;
        }
    }

    pub fn act_die(&mut self, world: &mut World) {
        self.health = 0;
        if !self.is_projectile() {
            let verb = if self.is_flesh() { "died" } else { "collapsed" };
            let msg = format!("{} {}!", self.name, verb);
            world.log_global(&msg, self.pos, self.is_important());
        }
        match self.is_flesh() {
            true => world.blood(self.pos),
            false => self.invis = -1,
        }
        self.act_drop_all(world);
        self.is_leader = false;
    }

    pub fn act_exert(&mut self, amt: u16, action: &str) {
        self.mana -= cmp::min(self.mana, amt);
        self.log_action(action);
    }

    pub fn hurt(&mut self, amt: u16, world: &mut World) {
        self.health -= cmp::min(self.health, amt);
        if self.health == 0 {
            self.act_die(world);
        }
    }

    pub fn stun(&mut self, amt: i16) {
        self.stun_counter = amt;
        let momentum = self.momentum;
        self.lose_momentum(momentum);
    }

    pub fn gain_momentum(&mut self, _amt: u8) {
        self.momentum = cmp::max(self.momentum, 1);
    }

    pub fn lose_momentum(&mut self, amt: u8) {
        self.momentum -= cmp::min(self.momentum, amt);
    }

    fn side_effects(&mut self, world: &mut World) {
        passive_effect!(passive_spin => self);
        passive_effect!(passive_drift => self, world);
        passive_effect!(passive_descend => self, world);
        if !self.is_projectile() && self.is_mobile() {
            if self.walls.contains(world.glyph_at(self.pos)) {
                self.hurt(5, world);
            }
            if self.is_hurt() && self.stun_counter == 0 && rand_int(self.health) == 0 {
                self.log_action("fell, bleeding profusely");
                self.stun(2);
                world.blood(self.pos);
            }
            self.recover(1);
            self.mana = cmp::min(self.max_mana(), self.mana + 1);
            self.act_get(world);
        }
        if self.invis > 0 {
            self.invis -= 1;
        }
    }

    pub fn recover(&mut self, amt: u16) {
        if self.stun_counter > 0 {
            self.stun_counter -= 1;
            if self.stun_counter == 0 {
                self.log_action("managed to get up");
            }
        }
        self.health = cmp::min(self.max_health(), self.health + amt);
    }

    pub fn restore(&mut self) {
        if self.is_alive() {
            self.health = self.max_health();
            self.mana = self.max_mana();
        }
    }

    pub fn is_alive(&self) -> bool {
        self.health > 0
    }

    pub fn can_block(&self) -> bool {
        !self.is_projectile() && self.is_alive()
    }

    fn is_blocking(&self, pos: (u16, u16)) -> bool {
        self.can_block() && self.pos == pos
    }

    fn can_displace(&self) -> bool {
        self.is_leader
    }

    pub fn is_playable(&self) -> bool {
        self.team == 0 && self.is_alive() && self.is_mobile() && !self.is_projectile()
    }

    pub fn is_ready_to_act(&self, time: u32) -> bool {
        (time + self.random_state as u32) % u32::from(self.speed) == 0
    }

    pub fn is_mobile(&self) -> bool {
        !self.walls.contains('.')
    }

    pub fn is_projectile(&self) -> bool {
        self.kind == 50 || self.kind == 51 || self.kind == 52 || self.kind == 53 ||
        self.kind == 7 || self.kind == 54
    }

    pub fn is_undead(&self) -> bool {
        self.kind == 4 || self.kind == 12
    }

    pub fn is_flesh(&self) -> bool {
        !self.is_projectile() && self.kind != 7 && !self.is_undead() && self.is_mobile()
    }

    pub fn is_enemy_of(&self, team: usize) -> bool {
        self.team != team && self.is_alive()
    }

    fn can_help(&self) -> bool {
        !self.is_hurt() && self.has_skill("heal")
    }

    pub fn is_near(&self, pos: (u16, u16)) -> bool {
        let (dx, dy) = (self.pos.0 - pos.0, self.pos.1 - pos.1);
        dx * dx + dy * dy < 10
    }

    pub fn is_in_danger(&self, p: &Plan) -> bool {
        (p.is_attacking(self.team) || p.is_retreating(self.team)) &&
        p.dist_to_pos(self.pos, self.team) < 20
    }

    pub fn is_hurt(&self) -> bool {
        self.health < self.max_health() / 2
    }

    pub fn has_skill(&self, skill: &str) -> bool {
        self.skills.contains(&skill.to_owned())
    }

    fn is_important(&self) -> bool {
        self.team == 0 || self.is_persistent || self.is_leader
    }
}
