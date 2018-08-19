// Handles tactics (for team 0), pathfinding, identifying friends/foes.
use actor::Actor;
use std::collections::{HashMap, HashSet};
use std::i32;
use world::{World, MOVE_ACTIONS};

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
            world_size,
            distances: HashMap::new(),
            occupied_cells: HashMap::new(),
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
        let maximum_steps = if team == 0 { 200 } else { 24 };
        let mut open_list = self.open_list(team, world, actors);
        self.initialize_all_distances(team, &open_list);
        let mut step = 0;
        while !open_list.is_empty() && step < maximum_steps {
            open_list = self.relax_distances_and_expand(open_list, step, team, world);
            step += 1;
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

    fn initialize_all_distances(&mut self, team: usize, open_list: &[(u16, u16)]) {
        for idx in 0..self.distances[&team].len() {
            self.distances.get_mut(&team).unwrap()[idx] = UNKNOWN_DISTANCE;
        }
        for pos in open_list {
            self.set_distance_to_goal(team, *pos, 0);
        }
    }

    fn relax_distances_and_expand(
        &mut self,
        open_list: Vec<(u16, u16)>,
        step: i32,
        team: usize,
        wld: &World,
    ) -> Vec<(u16, u16)> {
        let mut next_open_list: Vec<(u16, u16)> = Vec::new();
        for pos in open_list {
            for dir in &MOVE_ACTIONS {
                let next_pos = wld.neighbor(pos, *dir, team, DONT_PROPAGATE_INTO);
                // propogate forest to forest, but not forest to grass:
                if self.distance_to_goal(next_pos, team) == UNKNOWN_DISTANCE
                    && (!DONT_PROPAGATE_OUT_OF.contains(wld.glyph_at(pos))
                        || wld.glyph_at(next_pos) == wld.glyph_at(pos))
                {
                    self.set_distance_to_goal(team, next_pos, step + 1);
                    next_open_list.push(next_pos);
                }
            }
        }
        next_open_list
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

    pub fn distance_to_goal_avg(&self, pos: (u16, u16), team: usize, wld: &World) -> i32 {
        let mut avg = 0;
        for mv in &MOVE_ACTIONS {
            let new_pos = wld.neighbor(pos, *mv, team, DONT_PROPAGATE_OUT_OF);
            let dist = self.distance_to_goal(new_pos, team);
            if dist != UNKNOWN_DISTANCE {
                avg += dist;
            }
        }
        avg / (MOVE_ACTIONS.len() as i32)
    }

    pub fn distance_to_goal(&self, from: (u16, u16), team: usize) -> i32 {
        if let Some(distances) = self.distances.get(&team) {
            return distances[(from.1 * self.world_size.0 + from.0) as usize];
        }
        0
    }

    fn set_distance_to_goal(&mut self, team: usize, pos: (u16, u16), val: i32) {
        if let Some(field) = self.distances.get_mut(&team) {
            field[(pos.1 * self.world_size.0 + pos.0) as usize] = val;
        }
    }

    pub fn is_near_enemy(&self, pos: (u16, u16), team: usize) -> bool {
        (self.is_attacking(team) || self.is_retreating(team))
            && self.distance_to_goal(pos, team) < 10
    }

    pub fn num_enemies(&self) -> usize {
        self.team_0_enemies
    }

    pub fn whos_at(&self, pos: (u16, u16)) -> Option<&usize> {
        self.occupied_cells.get(&pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter::FromIterator;

    fn fixtures() -> (Plan, World, Vec<Actor>, HashSet<usize>) {
        let team_idxs = HashSet::from_iter(vec![0, 1]);
        let mut world = World::new();
        world.reshape((5, 5));
        let plan = Plan::new((5, 5), &team_idxs);
        let actors = vec![
            Actor::new(1, 1, 0, (0, 0), 0),
            Actor::new(1, 1, 1, (1, 4), 0),
        ];
        return (plan, world, actors, team_idxs);
    }

    #[test]
    fn test_tactics() {
        let (mut plan, _, _, _) = fixtures();
        plan.tactic_defend((2, 2));
        assert!(plan.is_defending(0));
        assert!(!plan.is_defending(1));
        assert_eq!(plan.muster_point(0), (2, 2));
        plan.tactic_attack();
        assert!(plan.is_attacking(0));
        plan.tactic_retreat();
        assert!(plan.is_retreating(0));
        plan.tactic_follow();
        assert!(!(plan.is_attacking(0) || plan.is_defending(0) || plan.is_retreating(0)));
    }

    #[test]
    fn test_fast_update_and_whos_at() {
        let (mut plan, _, actors, _) = fixtures();
        plan.fast_update(&actors);
        assert_eq!(plan.num_enemies(), 1);
        assert_eq!(plan.whos_at((0, 0)).unwrap(), &0);
        assert_eq!(plan.whos_at((1, 4)).unwrap(), &1);
        assert!(plan.whos_at((2, 2)).is_none());
    }

    #[test]
    fn test_update() {
        let (mut plan, world, actors, team_idxs) = fixtures();
        plan.tactic_attack(); // ensure teams are attacking each other
        plan.fast_update(&actors); // ensures enemy counts are correct
        plan.update(&team_idxs, &world, &actors);
        for actor in actors {
            assert_eq!(plan.distance_to_goal(actor.pos, actor.team), 4);
            assert!(
                plan.distances[&actor.team]
                    .iter()
                    .all(|distance| distance != &UNKNOWN_DISTANCE)
            );
        }
    }
}
