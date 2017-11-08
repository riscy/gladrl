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
const DONT_PROPAGATE_INTO: &'static str = "`*#";
const DONT_PROPAGATE_OUT_OF: &'static str = "~`*#%";

pub struct Plan {
    team_0_enemies: usize,
    team_0_tactic: u8,
    team_0_tactical_position: (u16, u16),
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
            team_0_tactical_position: (0, 0),
        };
        for &team in teams {
            let area = (world_size.1 * world_size.0) as usize;
            plan.distances.insert(team, vec![UNKNOWN_DISTANCE; area]);
        }
        plan
    }

    pub fn tactic_defend(&mut self, pos: (u16, u16)) {
        self.team_0_tactic = TACTIC_DEFEND;
        self.team_0_tactical_position = pos;
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

    fn tactic(&self, team: usize) -> u8 {
        match team {
            0 => self.team_0_tactic,
            _ => TACTIC_ATTACK,
        }
    }

    pub fn tactical_position(&self, team: usize) -> (u16, u16) {
        match team {
            0 => return self.team_0_tactical_position,
            _ => return (0, 0),
        }
    }

    pub fn num_enemies(&self) -> usize {
        self.team_0_enemies
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
        if self.team_0_enemies == 0 {
            self.team_0_tactic = TACTIC_EXIT;
        }
        for &team in teams {
            self.update_path(team, world, actors);
        }
    }

    fn pos_exits(&self, world: &World) -> Vec<(u16, u16)> {
        world.exits.iter().map(|exit| exit.pos).collect()
    }

    fn pos_enemies(&self, team: usize, actors: &[Actor]) -> Vec<(u16, u16)> {
        actors
            .iter()
            .filter(|actor| actor.is_enemy_of(team) && actor.invis == 0)
            .map(|actor| actor.pos)
            .collect()
    }

    fn pos_leaders(&self, team: usize, actors: &[Actor]) -> Vec<(u16, u16)> {
        actors
            .iter()
            .filter(|actor| actor.is_leader && actor.team == team)
            .map(|actor| actor.pos)
            .collect()
    }

    fn open_list(&self, team: usize, world: &World, actors: &[Actor]) -> Vec<(u16, u16)> {
        let mut open_list = Vec::new();
        match self.tactic(team) {
            TACTIC_EXIT => open_list.append(&mut self.pos_exits(world)),
            TACTIC_DEFEND => open_list.append(&mut vec![self.tactical_position(team)]),
            TACTIC_FOLLOW => open_list.append(&mut self.pos_leaders(team, actors)),
            TACTIC_ATTACK => open_list.append(&mut self.pos_enemies(team, actors)),
            TACTIC_RETREAT => open_list.append(&mut self.pos_enemies(team, actors)),
            _ => {}
        }
        open_list
    }

    fn update_path(&mut self, team: usize, world: &World, actors: &[Actor]) {
        for idx in 0..self.distances[&team].len() {
            self.distances.get_mut(&team).unwrap()[idx] = UNKNOWN_DISTANCE;
        }
        let mut open_list = self.open_list(team, world, actors);
        for pos in &open_list {
            self.set_distance_to_target(team, *pos, 0);
        }
        let maximum_steps = if team == 0 { 200 } else { 24 };
        for steps in 0..maximum_steps {
            let mut next_open_list: Vec<(u16, u16)> = Vec::new();
            for &pos in &open_list {
                for dir in &MOVE_ACTIONS {
                    let neighbor = world.neighbor(pos, *dir, team, DONT_PROPAGATE_INTO);
                    if self.distance_to_target(neighbor, team) != UNKNOWN_DISTANCE {
                        continue;
                    }
                    // propogate if appropriate OR this neighbor is the same
                    // glyph as this (forest to forest, but not forest to
                    // grass); lets npcs path through forests or swim to shore
                    if !DONT_PROPAGATE_OUT_OF.contains(world.glyph_at(pos)) ||
                       world.glyph_at(neighbor) == world.glyph_at(pos) {
                        // propagate distances to all neighbors
                        self.set_distance_to_target(team, neighbor, steps + 1);
                        next_open_list.push(neighbor);
                    }
                }
            }
            if next_open_list.is_empty() {
                break;
            }
            open_list = next_open_list;
        }
    }

    fn set_distance_to_target(&mut self, team: usize, pos: (u16, u16), val: i32) {
        let field = self.distances.get_mut(&team).unwrap();
        field[(pos.1 * self.world_size.0 + pos.0) as usize] = val;
    }

    pub fn dist_is_greater_than(&self, pos: (u16, u16), team: usize, amt: i32) -> bool {
        if team == 0 && self.team_0_tactic != TACTIC_ATTACK {
            return true;
        }
        let dist = self.distance_to_target(pos, team);
        dist > amt && dist != UNKNOWN_DISTANCE
    }

    pub fn distance_to_target(&self, pos: (u16, u16), team: usize) -> i32 {
        self.distances[&team][(pos.1 * self.world_size.0 + pos.0) as usize]
    }

    pub fn gradient(&self, from: (u16, u16), to: (u16, u16), team: usize, toward: bool) -> i32 {
        let dd = self.distance_to_target(from, team) - self.distance_to_target(to, team);
        let toward = toward && !self.is_retreating(team);
        if toward { dd } else { -dd }
    }

    pub fn whos_at(&self, pos: (u16, u16)) -> Option<&usize> {
        self.occupied_cells.get(&pos)
    }

    fn is_retreating(&self, team: usize) -> bool {
        team == 0 && self.team_0_tactic == TACTIC_RETREAT
    }
}
