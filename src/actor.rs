// Handles active objects like living entities and projectiles.
use constants::{ACT_DROP, ACT_MOVES, ACT_SKILL, ACT_TURNS, ACT_WAIT, TILE_BLOOD};
use csv;
use inflector::Inflector;
use item::Item;
use item_effects;
use plan::Plan;
use skills::*;
use skills_registry::{choose_skill, use_skill};
use std::{cmp, i32, u16};
use world::World;

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
    pub move_lag: u16,
    pub mana: u16,
    pub intel: u16,
    pub con: u16,
    pub strength: u16,
    pub walls: String,
    skill_idx: usize,

    pub momentum: u8,
    pub stun: i16,
    pub invis: i16,

    pub log: Vec<(u32, String, usize)>,
    pub random_seed: u16,
    pub skills: Vec<String>,
    pub inventory: Vec<Item>,

    pub is_leader: bool,
    pub is_persistent: bool,
}

impl Actor {
    pub fn new(kind: u8, level: u16, team: usize, pos: (u16, u16)) -> Actor {
        let mut actor = Actor {
            kind,
            pos,
            level,
            team,
            direction: 0,
            health: 1,
            strength: 1,
            con: 1,
            intel: 1,
            mana: 1,
            name: String::new(),
            walls: String::new(),
            random_seed: rand_int(u16::MAX),
            is_leader: false,
            stun: 0,
            glyph: '?',
            move_lag: 1,
            is_persistent: false,
            momentum: 0,
            skill_idx: 0,
            skills: Vec::new(),
            time: 1,
            log: Vec::new(),
            inventory: Vec::new(),
            invis: 0,
        };
        actor.initialize(kind);
        actor.recover_fully();
        actor
    }

    pub fn initialize(&mut self, kind: u8) {
        self.skills.clear();
        let reader = csv::Reader::from_path("config/glad/actor.csv");
        for record in reader.unwrap().deserialize() {
            let row: (u8, char, String, String, u16, String, u16, u16, u16, u16) = record.unwrap();
            if row.0 != kind {
                continue;
            }
            self.kind = kind;
            self.glyph = row.1;
            self.walls = row.2;
            if self.name.is_empty() {
                self.name = row.3;
            }
            self.move_lag = row.4;
            for skill in row.5.split(' ') {
                self.skills.push(skill.into());
            }
            self.strength = row.6;
            self.con = row.8;
            self.intel = row.9;
            break;
        }
        self.initialize_inventory();
    }

    fn initialize_inventory(&mut self) {
        for kind in self.inventory.iter().map(|it| it.kind).collect::<Vec<u8>>() {
            item_effects::use_on_actor(self, kind);
        }
    }

    pub fn glyph(&self) -> char {
        if !self.is_alive() {
            return 'x';
        } else if self.stun == 0 {
            return self.glyph;
        }
        if let Some(lower_char) = self.glyph.to_lowercase().next() {
            return lower_char;
        }
        '?'
    }

    pub fn max_health(&self) -> u16 {
        cmp::max(1, self.con * self.level)
    }

    pub fn max_mana(&self) -> u16 {
        cmp::max(1, self.intel * self.level)
    }

    pub fn teleport(&mut self, wld: &World) -> (u16, u16) {
        loop {
            let pos = (rand_int(wld.size.0 as u16), rand_int(wld.size.1 as u16));
            if !self.walls.contains(wld.glyph_at(pos)) {
                self.pos = pos;
                return self.pos;
            }
        }
    }

    pub fn log_event(&mut self, txt: &str, time: u32) {
        if let Some(last_log) = self.log.last_mut() {
            if last_log.1 == txt {
                last_log.0 = time;
                return last_log.2 += 1;
            }
        }
        self.log.push((time, txt.to_owned(), 1));
    }

    pub fn log_action(&mut self, verb: &str) {
        let time = self.time;
        self.log_event(&format!("I {}", verb), time);
    }

    pub fn log_interaction(&mut self, verb: &str, other: &mut Actor) {
        let time = cmp::max(self.time, other.time);
        let capitalized_name = self.name.to_sentence_case();
        self.log_event(&format!("I {} {}.", verb, other.name), time);
        other.log_event(&format!("{} {} me!", capitalized_name, verb), time);
    }

    pub fn select_skill(&mut self, skill: &str) {
        for (idx, self_skill) in self.skills.iter().enumerate() {
            if self_skill == skill {
                return self.skill_idx = idx;
            }
        }
    }

    pub fn selected_skill(&self) -> String {
        match self.skills.get(self.skill_idx) {
            Some(skill) => skill.to_owned(),
            None => String::new(),
        }
    }

    pub fn next_skill(&mut self) {
        self.skill_idx = (self.skill_idx + 1) % self.skills.len();
        for _idx in 0..self.skills.len() {
            if self.selected_skill().starts_with("passive") {
                self.skill_idx = (self.skill_idx + 1) % self.skills.len();
            }
        }
        let skill = self.selected_skill();
        self.log_action(&format!("switched to {}.", skill));
    }

    pub fn inventory(&mut self) {
        self.log_action("turned out my pockets.");
        for idx in 0..self.inventory.len() {
            let log = &format!("had {}.", self.inventory[idx].name);
            self.log_action(log);
        }
    }

    pub fn choose(&mut self, world: &World, plan: &Plan) -> u8 {
        if self.is_projectile() {
            return self.direction;
        } else if choose_skill(self, world, plan) {
            return ACT_SKILL;
        }
        self.choose_action(world, plan)
    }

    fn choose_action(&self, world: &World, plan: &Plan) -> u8 {
        let start_dir = self.choose_preferred_dir();
        let (mut best_value, mut best_direction) = (i32::MIN, start_dir);
        for mv in ACT_MOVES.iter().map(|offset| (start_dir + offset) % 9) {
            let mut pos = world.neighbor(self.pos, mv, self.team, &self.walls);
            let mut movement = pos != self.pos;
            if !movement {
                pos = world.offset(self.pos, mv)
            }
            if !self.is_hurt() && !plan.is_retreating(self.team) {
                if let Some(&team) = plan.whos_at(pos) {
                    if team != self.team || (pos != self.pos && self.can_help()) {
                        return mv;
                    } else if !self.can_displace(plan) {
                        movement = false;
                    }
                }
            }
            if movement || mv == ACT_WAIT {
                let value = self.value_of_pos(pos, plan);
                if value < best_value {
                    continue;
                }
                if value > best_value {
                    best_direction = mv;
                    best_value = value;
                }
            }
        }
        best_direction
    }

    // How good this position is to the actor (larger is better)
    fn value_of_pos(&self, pos: (u16, u16), plan: &Plan) -> i32 {
        let dist = plan.distance_to_goal(pos, self.team);
        if self.is_retreating(plan) {
            dist
        } else {
            -dist
        }
    }

    fn choose_preferred_dir(&self) -> u8 {
        if !self.is_projectile() && rand_int(5) == 0 {
            return rand_int(8) as u8;
        }
        self.direction
    }

    pub fn act(
        &mut self,
        mv: u8,
        time: u32,
        wld: &mut World,
        plan: &Plan,
        other: &mut Vec<&mut [Actor]>,
        spawn: &mut Vec<Actor>,
    ) {
        self.time = time;
        if self.stun == 0 {
            match mv {
                ACT_SKILL => use_skill(self, wld, plan, spawn),
                ACT_DROP => self.act_drop_item(wld),
                _ => {
                    if self.is_mobile() {
                        self.act_move(mv, wld, plan, other);
                    }
                    self.act_change_direction(mv, wld, plan);
                }
            };
        }
    }

    fn act_move(&mut self, mv: u8, wld: &mut World, plan: &Plan, other: &mut Vec<&mut [Actor]>) {
        let mut pos = wld.neighbor(self.pos, mv, self.team, &self.walls);
        let movement = self.pos != pos;
        if !movement {
            pos = wld.offset(self.pos, mv);
            self.lose_momentum(1);
        }
        if plan.whos_at(pos).is_some() {
            for actors in other {
                for actor in actors.iter_mut().filter(|xx| xx.is_blocking(pos)) {
                    self.act_touch(actor, wld, mv, plan);
                }
            }
        } else if movement {
            self.pos = pos;
            self.gain_momentum(1);
            passive_effect!(passive_grow => self, wld);
        } else if ACT_MOVES.contains(&mv) {
            self.act_push_wall(wld, mv);
        }
    }

    fn act_push_wall(&mut self, world: &mut World, action: u8) {
        if let Some(treasure) = world.push_wall(self.pos, action, &self.inventory) {
            self.log_action(&format!("pulled on {}.", treasure.name));
            item_effects::use_on_actor(self, treasure.kind);
            if treasure.can_keep {
                self.log_action(&format!("got {}.", treasure.name));
                self.inventory.push(treasure);
            }
        }
    }

    fn act_change_direction(&mut self, dir: u8, wld: &World, plan: &Plan) {
        if ACT_MOVES.contains(&dir) {
            self.direction = dir % 8;
        } else if ACT_TURNS.contains(&dir) {
            self.log_action("turned in place.");
            self.direction = dir % 8;
        }
        passive_effect!(passive_aim => self, wld, plan);
    }

    fn act_get(&mut self, world: &mut World) {
        let mut idx = 0;
        while idx < world.items.len() {
            if self.pos == world.items[idx].pos && world.items[idx].can_get {
                let item = world.items.remove(idx);
                self.log_action(&format!("got {}.", item.name));
                item_effects::use_on_actor(self, item.kind);
                if item.can_keep {
                    self.inventory.push(item);
                }
                continue;
            }
            idx += 1;
        }
    }

    fn act_drop_item(&mut self, world: &mut World) {
        if let Some(item) = self.inventory.pop() {
            self.log_action(&format!("dropped {}.", item.name));
            let pos = world.neighbor(self.pos, self.direction, self.team, "");
            world.add_item(item, pos);
            let kind = self.kind;
            return self.initialize(kind);
        }
        self.log_action("had nothing to drop.")
    }

    fn act_drop_all(&mut self, world: &mut World) {
        while !self.inventory.is_empty() {
            self.act_drop_item(world);
            self.direction = (self.direction + 1) % 8;
        }
    }

    fn act_touch(&mut self, other: &mut Actor, world: &mut World, action: u8, plan: &Plan) {
        if other.is_enemy_of(self.team) && self.strength > 0 {
            passive_effect!(passive_trip => self, action, other);
            passive_effect!(passive_whirl => self, action, other);
            passive_effect!(passive_backstab => self, action, other);
            passive_effect!(passive_slam => self, action, other, world, plan);
            return self.act_hit(other, world);
        } else if self.can_displace(plan) && other.is_mobile() {
            return self.act_displace(other, world);
        }
        passive_effect!(passive_heal => self, other, world);
        self.act_help(other)
    }

    fn act_displace(&mut self, other: &mut Actor, world: &mut World) {
        if !self.walls.contains(world.glyph_at(other.pos))
            && !other.walls.contains(world.glyph_at(self.pos))
        {
            let new_pos = other.pos;
            other.pos = (self.pos.0, self.pos.1);
            other.stun(1);
            self.pos = new_pos;
            self.lose_momentum(1);
            self.log_interaction("displaced", other);
        }
    }

    fn act_help(&mut self, other: &mut Actor) {
        if other.stun > 0 && !self.is_projectile() {
            other.stun = 0;
            self.log_action(&format!("hoisted {} up.", other.name));
            other.log_action(&format!("was hoisted up by {}.", self.name));
        }
        self.lose_momentum(1);
    }

    fn act_hit(&mut self, other: &mut Actor, world: &mut World) {
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
            let msg = format!("{} {}!", self.name.to_sentence_case(), verb);
            world.log_global(&msg, self.pos, self.is_important());
        }
        self.act_drop_all(world);
        self.is_leader = false;
        if self.is_flesh() {
            world.add_item(Item::new(0, self.level, self.team), self.pos);
            world.change_tiles(self.pos, TILE_BLOOD);
        }
        self.invis = -1; // 👻
    }

    pub fn act_exert(&mut self, amt: u16, action: &str) {
        self.mana -= cmp::min(self.mana, amt);
        self.log_action(action);
    }

    pub fn hurt(&mut self, amt: u16, world: &mut World) {
        if amt < self.health {
            return self.health -= amt;
        }
        self.act_die(world);
    }

    pub fn stun(&mut self, amt: i16) {
        self.stun = amt;
        self.lose_momentum(1);
    }

    pub fn gain_momentum(&mut self, _amt: u8) {
        self.momentum = cmp::max(self.momentum, 1);
    }

    pub fn lose_momentum(&mut self, amt: u8) {
        self.momentum -= cmp::min(self.momentum, amt);
    }

    pub fn update(&mut self, world: &mut World) {
        passive_effect!(passive_spin => self);
        passive_effect!(passive_drift => self, world);
        passive_effect!(passive_descend => self, world);
        if !self.is_projectile() && self.is_mobile() {
            self.recover(1);
            self.mana = cmp::min(self.max_mana(), self.mana + 1);
            if self.walls.contains(world.glyph_at(self.pos)) {
                self.hurt(5, world);
            }
            if self.is_hurt() && self.stun == 0 && rand_int(self.health) == 0 {
                self.log_action("fell, bleeding profusely.");
                self.stun(2);
                world.change_tiles(self.pos, TILE_BLOOD);
            }
            if self.is_alive() {
                self.act_get(world);
            }
        }
        if self.invis > 0 {
            self.invis -= 1;
        }
    }

    pub fn recover(&mut self, amt: u16) {
        if self.stun > 0 {
            self.stun -= 1;
            match self.stun {
                0 => self.log_action("managed to get up."),
                _ => self.log_action("struggled on the ground."),
            }
        }
        self.health = cmp::min(self.max_health(), self.health + amt);
    }

    pub fn recover_fully(&mut self) {
        if self.is_alive() {
            self.health = self.max_health();
            self.mana = self.max_mana();
        }
    }

    pub fn is_alive(&self) -> bool {
        self.health > 0
    }

    pub fn is_combatant(&self) -> bool {
        !self.is_projectile() && self.is_alive()
    }

    fn is_blocking(&self, pos: (u16, u16)) -> bool {
        self.is_combatant() && self.pos == pos
    }

    fn can_displace(&self, plan: &Plan) -> bool {
        if !self.is_leader && self.team == 0 {
            return false;
        }
        self.has_skill("passive_displace") || plan.num_enemies() == 0
    }

    pub fn is_playable(&self) -> bool {
        self.team == 0 && self.is_alive() && self.is_mobile() && !self.is_projectile()
    }

    pub fn is_ready_to_act(&self, time: u32) -> bool {
        self.is_alive() && (time + u32::from(self.random_seed)) % u32::from(self.move_lag) == 0
    }

    pub fn is_mobile(&self) -> bool {
        !self.walls.contains('.')
    }

    pub fn is_projectile(&self) -> bool {
        self.kind >= 50 && self.kind < 60
    }

    pub fn is_undead(&self) -> bool {
        self.kind == 4 || self.kind == 12
    }

    pub fn is_flesh(&self) -> bool {
        !self.is_projectile() && !self.is_undead() && self.is_mobile()
    }

    pub fn is_enemy_of(&self, team: usize) -> bool {
        self.team != team && self.is_alive()
    }

    fn can_help(&self) -> bool {
        !self.is_hurt() && self.has_skill("heal")
    }

    pub fn is_near(&self, pos: (u16, u16)) -> bool {
        let (dx, dy) = (
            i32::from(self.pos.0) - i32::from(pos.0),
            i32::from(self.pos.1) - i32::from(pos.1),
        );
        dx * dx + dy * dy <= 18
    }

    fn is_retreating(&self, plan: &Plan) -> bool {
        plan.is_retreating(self.team) || (self.is_hurt() && plan.is_attacking(self.team))
    }

    pub fn is_hurt(&self) -> bool {
        self.health < self.max_health() / 2
    }

    pub fn has_skill(&self, skill: &str) -> bool {
        self.skills.iter().any(|s| s.as_str() == skill)
    }

    fn is_important(&self) -> bool {
        self.team == 0 || self.is_persistent || self.is_leader
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::iter::FromIterator;

    fn fixtures() -> (Actor, Actor, World, Plan) {
        let soldier = Actor::new(0, 1, 0, (2, 2));
        let archer = Actor::new(2, 1, 1, (1, 2));
        let boots = Item::new(7, 0, 0);
        let plan = Plan::new((5, 5), &HashSet::from_iter(vec![0, 1]));
        let mut world = World::new("glad");
        world.reshape((5, 5));
        world.add_item(boots, (2, 2));
        return (soldier, archer, world, plan);
    }

    #[test]
    fn test_initialize() {
        let (mut soldier, archer, _world, _plan) = fixtures();
        assert!(!soldier.skills.contains(&String::from("shoot")));
        soldier.initialize(archer.kind);
        assert!(soldier.skills.contains(&String::from("shoot")));
    }

    #[test]
    fn test_predicates() {
        let (soldier, archer, _world, _plan) = fixtures();
        assert!(soldier.is_alive());
        assert!(soldier.is_flesh());
        assert!(soldier.is_mobile());
        assert!(soldier.is_playable());
        assert!(soldier.is_enemy_of(archer.team));
        assert!(soldier.is_combatant() && soldier.is_blocking(soldier.pos));
        assert!(!soldier.is_hurt());
        assert!(!soldier.is_undead());
        assert!(!soldier.is_projectile());
        assert!(archer.has_skill("shoot"));
        assert!(soldier.has_skill("charge"));
    }

    #[test]
    fn test_is_near() {
        let (soldier, archer, _world, _plan) = fixtures();
        assert!(soldier.is_near(archer.pos));
        assert!(!soldier.is_near((100, 100)));
    }

    #[test]
    fn test_enemy_interactions() {
        let (mut soldier, mut archer, mut world, plan) = fixtures();
        let all_but_2 = soldier.health - 2;
        soldier.hurt(all_but_2, &mut world);
        assert!(soldier.is_alive() && soldier.is_hurt());
        archer.act_touch(&mut soldier, &mut world, 2, &plan);
        archer.act_hit(&mut soldier, &mut world);
        assert!(!soldier.is_alive());
    }

    #[test]
    fn test_stun_and_recover() {
        let (mut soldier, mut archer, mut world, _plan) = fixtures();
        soldier.gain_momentum(1);
        assert_eq!(soldier.glyph(), 'S');
        soldier.stun(1);
        assert_eq!(soldier.momentum, 0);
        assert_eq!(soldier.glyph(), 's');
        archer.act_help(&mut soldier);
        assert_eq!(soldier.glyph(), 'S');
        soldier.hurt(1, &mut world);
        soldier.act_exert(10, "threw an elf");
        soldier.recover_fully();
        assert_eq!(soldier.health, soldier.max_health());
        assert_eq!(soldier.mana, soldier.max_mana());
    }

    #[test]
    fn test_gain_and_lose_momentum() {
        let (mut soldier, mut archer, mut world, _plan) = fixtures();
        soldier.gain_momentum(1);
        assert_eq!(soldier.momentum, 1);
        soldier.act_hit(&mut archer, &mut world);
        assert_eq!(soldier.momentum, 0);
    }

    #[test]
    fn test_get_and_drop() {
        let (mut soldier, _archer, mut world, plan) = fixtures();
        soldier.direction = 2;
        soldier.act_get(&mut world);
        assert_eq!(soldier.inventory.len(), 1);
        soldier.act_drop_all(&mut world);
        assert_eq!(soldier.inventory.len(), 0);
        // move forward and wait for auto-pickup:
        soldier.act_move(2, &mut world, &plan, &mut vec![]);
        soldier.update(&mut world);
        assert_eq!(soldier.inventory.len(), 1);
    }
}
