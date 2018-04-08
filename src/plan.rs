// Handles tactics (for team 0), pathfinding, identifying friends/foes.
use std::i32;
use std::collections::{HashMap, HashSet};
use actor::{Actor, MOVE_ACTIONS};
use world::World;

const TACTIC_ATTACK: u8 = 0;
const TACTIC_FOLLOW: u8 = 1;
const TACTIC_DEFEND: u8 = 2;
const TACTIC_RETREAT: u8 = 3;
const TACTIC_EXIT: u8 = 4;

const UNKNOWN_DISTANCE: i32 = i32::MAX;
const DONT_PROPAGATE_INTO: &str = "`'*#^";
const DONT_PROPAGATE_OUT_OF: &str = "~`'*#%^";

pub struct Plan {
    team_0_enemies: usize,
    team_0_tactic: u8,
    team_0_muster_point: (u16, u16),
    occupied_cells: HashMap<(u16, u16), usize>,
    world_size: (u16, u16),
    distances: HashMap<usize, Vec<i32>>,
}

impl Plan {
    pub fn new(world_size: (u16, u16), teams: &HashSet<usize>) -> Plan {
        let mut plan = Plan {
            distances: HashMap::new(),
            occupied_cells: HashMap::new(),
            world_size: world_size,
            team_0_enemies: 0,
            team_0_tactic: TACTIC_FOLLOW,
            team_0_muster_point: (0, 0),
        };
        for &team in teams {
            let area = (world_size.1 * world_size.0) as usize;
            plan.distances.insert(team, vec![UNKNOWN_DISTANCE; area]);
        }
        plan
    }

    fn tactic(&self, team: usize) -> u8 {
        match team {
            0 => self.team_0_tactic,
            _ => TACTIC_ATTACK,
        }
    }

    pub fn tactic_defend(&mut self, pos: (u16, u16)) {
        self.team_0_tactic = TACTIC_DEFEND;
        self.team_0_muster_point = pos;
    }

    pub fn tactic_follow(&mut self) {
        self.team_0_tactic = TACTIC_FOLLOW;
    }

    pub fn tactic_attack(&mut self) {
        self.team_0_tactic = TACTIC_ATTACK;
    }

    pub fn tactic_retreat(&mut self) {
        self.team_0_tactic = TACTIC_RETREAT;
    }

    fn muster_point(&self, team: usize) -> (u16, u16) {
        match team {
            0 => self.team_0_muster_point,
            _ => (0, 0),
        }
    }

    pub fn is_defending(&self, team: usize) -> bool {
        team == 0 && self.team_0_tactic == TACTIC_DEFEND
    }

    pub fn is_attacking(&self, team: usize) -> bool {
        team != 0 || self.team_0_tactic == TACTIC_ATTACK
    }

    pub fn is_retreating(&self, team: usize) -> bool {
        team == 0 && self.team_0_tactic == TACTIC_RETREAT
    }

    pub fn fast_update(&mut self, actors: &[Actor]) {
        self.occupied_cells.clear();
        self.team_0_enemies = 0;
        for actor in actors.iter().filter(|actor| actor.can_block()) {
            self.occupied_cells.insert(actor.pos, actor.team);
            if actor.team != 0 {
                self.team_0_enemies += 1;
            }
        }
    }

    pub fn update(&mut self, teams: &HashSet<usize>, world: &World, actors: &[Actor]) {
        if self.team_0_enemies == 0 && self.team_0_tactic == TACTIC_ATTACK {
            self.team_0_tactic = TACTIC_EXIT;
        }
        for &team in teams {
            self.update_paths(team, world, actors);
        }
    }

    fn update_paths(&mut self, team: usize, world: &World, actors: &[Actor]) {
        for idx in 0..self.distances[&team].len() {
            self.distances.get_mut(&team).unwrap()[idx] = UNKNOWN_DISTANCE;
        }
        let maximum_steps = if team == 0 { 200 } else { 24 };
        let mut open_list = self.open_list(team, world, actors);
        for pos in &open_list {
            self.set_dist_to_goal(team, *pos, 0);
        }
        for steps in 0..maximum_steps {
            let mut next_open_list: Vec<(u16, u16)> = Vec::new();
            for pos in open_list {
                for dir in &MOVE_ACTIONS {
                    let next = world.neighbor(pos, *dir, team, DONT_PROPAGATE_INTO);
                    // propogate forest to forest, but not forest to grass:
                    if self.dist_to_goal(next, team) == UNKNOWN_DISTANCE &&
                       (!DONT_PROPAGATE_OUT_OF.contains(world.glyph_at(pos)) ||
                        world.glyph_at(next) == world.glyph_at(pos)) {
                        self.set_dist_to_goal(team, next, steps + 1);
                        next_open_list.push(next);
                    }
                }
            }
            if next_open_list.is_empty() {
                break;
            }
            open_list = next_open_list;
        }
    }

    fn open_list(&self, team: usize, world: &World, actors: &[Actor]) -> Vec<(u16, u16)> {
        match self.tactic(team) {
            TACTIC_EXIT => self.locate_exits(world),
            TACTIC_DEFEND => vec![self.muster_point(team)],
            TACTIC_FOLLOW => self.locate_leaders(team, actors),
            TACTIC_ATTACK | TACTIC_RETREAT => self.locate_enemies(team, actors),
            _ => Vec::new(),
        }
    }

    fn locate_exits(&self, world: &World) -> Vec<(u16, u16)> {
        world.exits.iter().map(|exit| exit.pos).collect()
    }

    fn locate_enemies(&self, team: usize, actors: &[Actor]) -> Vec<(u16, u16)> {
        actors
            .iter()
            .filter(|actor| actor.is_enemy_of(team) && actor.invis == 0)
            .map(|actor| actor.pos)
            .collect()
    }

    fn locate_leaders(&self, team: usize, actors: &[Actor]) -> Vec<(u16, u16)> {
        actors
            .iter()
            .filter(|actor| actor.is_leader && actor.team == team)
            .map(|actor| actor.pos)
            .collect()
    }

    pub fn dist_to_goal_avg(&self, pos: (u16, u16), team: usize, wld: &World) -> i32 {
        let mut avg = 0;
        for &mv in MOVE_ACTIONS.iter() {
            let new_pos = wld.neighbor(pos, mv, team, DONT_PROPAGATE_OUT_OF);
            let dist = self.dist_to_goal(new_pos, team);
            if dist != UNKNOWN_DISTANCE {
                avg += dist;
            }
        }
        avg / (MOVE_ACTIONS.len() as i32)
    }

    pub fn dist_to_goal(&self, from: (u16, u16), team: usize) -> i32 {
        if let Some(distances) = self.distances.get(&team) {
            return distances[(from.1 * self.world_size.0 + from.0) as usize];
        }
        0
    }

    fn set_dist_to_goal(&mut self, team: usize, pos: (u16, u16), val: i32) {
        if let Some(field) = self.distances.get_mut(&team) {
            field[(pos.1 * self.world_size.0 + pos.0) as usize] = val;
        }
    }

    pub fn is_near_enemy(&self, pos: (u16, u16), team: usize) -> bool {
        (self.is_attacking(team) || self.is_retreating(team)) && self.dist_to_goal(pos, team) < 10
    }

    pub fn num_enemies(&self) -> usize {
        self.team_0_enemies
    }

    pub fn whos_at(&self, pos: (u16, u16)) -> Option<&usize> {
        self.occupied_cells.get(&pos)
    }
}
