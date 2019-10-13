// Handles tactics (for team 0), pathfinding, identifying friends/foes.
use actor::Actor;
use constants::ACT_MOVES;
use std::collections::{HashMap, HashSet};
use std::i32;
use world::World;

const PLAN_ATTACK: u8 = 0;
const PLAN_FOLLOW: u8 = 1;
const PLAN_DEFEND: u8 = 2;
const PLAN_RETREAT: u8 = 3;
const PLAN_EXIT: u8 = 4;

const PATH_UNKNOWN_DISTANCE: i32 = i32::MAX;
const PATH_DONT_PROPAGATE_INTO: &str = "`'*#^";
const PATH_DONT_PROPAGATE_OUT_OF: &str = "~`'*#%^";

pub struct Plan {
    _enemies: HashMap<usize, usize>,
    _tactics: HashMap<usize, u8>,
    _muster_point: HashMap<usize, (u16, u16)>,
    _occupied_cells: HashMap<(u16, u16), usize>,
    _world_size: (u16, u16),
    _distances: HashMap<usize, Vec<i32>>,
}

impl Plan {
    pub fn new(_world_size: (u16, u16), teams: &HashSet<usize>) -> Plan {
        let mut plan = Plan {
            _world_size,
            _distances: HashMap::new(),
            _occupied_cells: HashMap::new(),
            _enemies: HashMap::new(),
            _tactics: HashMap::new(),
            _muster_point: HashMap::new(),
        };
        for &team in teams {
            let area = (_world_size.1 * _world_size.0) as usize;
            plan._distances
                .insert(team, vec![PATH_UNKNOWN_DISTANCE; area]);
            plan._enemies.insert(team, 0);
            plan._muster_point.insert(team, (0, 0));
            plan._tactics
                .insert(team, if team == 0 { PLAN_FOLLOW } else { PLAN_ATTACK });
        }
        plan
    }

    pub fn tactic_defend(&mut self, pos: (u16, u16)) {
        self._tactics.insert(0, PLAN_DEFEND);
        self._muster_point.insert(0, pos);
    }

    pub fn tactic_follow(&mut self) {
        self._tactics.insert(0, PLAN_FOLLOW);
    }

    pub fn tactic_attack(&mut self) {
        self._tactics.insert(0, PLAN_ATTACK);
    }

    pub fn tactic_retreat(&mut self) {
        self._tactics.insert(0, PLAN_RETREAT);
    }

    fn _muster_point(&self, team: usize) -> (u16, u16) {
        self._muster_point[&team]
    }

    pub fn is_defending(&self, team: usize) -> bool {
        self._tactics[&team] == PLAN_DEFEND
    }

    pub fn is_attacking(&self, team: usize) -> bool {
        self._tactics[&team] == PLAN_ATTACK
    }

    pub fn is_retreating(&self, team: usize) -> bool {
        self._tactics[&team] == PLAN_RETREAT
    }

    pub fn fast_update(&mut self, actors: &[Actor]) {
        self._occupied_cells.clear();
        self._enemies.insert(0, 0);
        for actor in actors.iter().filter(|actor| actor.is_combatant()) {
            self._occupied_cells.insert(actor.pos, actor.team);
            if actor.team != 0 {
                let current_enemies = self._enemies[&0];
                self._enemies.insert(0, current_enemies + 1);
            }
        }
    }

    pub fn update(&mut self, teams: &HashSet<usize>, world: &World, actors: &[Actor]) {
        if self._enemies[&0] == 0 && self._tactics[&0] == PLAN_ATTACK {
            self._tactics.insert(0, PLAN_EXIT);
        }
        for &team in teams {
            self._update_paths(team, world, actors);
        }
    }

    fn _update_paths(&mut self, team: usize, world: &World, actors: &[Actor]) {
        let maximum_steps = if team == 0 { 200 } else { 12 };
        let mut open_list = self._open_list(team, world, actors);
        self._initialize_all_distances(team, &open_list);
        let mut step = 0;
        while !open_list.is_empty() && step < maximum_steps {
            open_list = self._relax_distances_and_expand(open_list, step, team, world);
            step += 1;
        }
    }

    fn _open_list(&self, team: usize, world: &World, actors: &[Actor]) -> Vec<(u16, u16)> {
        match self._tactics[&team] {
            PLAN_EXIT => world.exits(),
            PLAN_DEFEND => vec![self._muster_point(team)],
            PLAN_FOLLOW => self._locate_leaders(team, actors),
            PLAN_ATTACK | PLAN_RETREAT => self._locate_enemies(team, actors),
            _ => Vec::new(),
        }
    }

    fn _initialize_all_distances(&mut self, team: usize, open_list: &[(u16, u16)]) {
        for idx in 0..self._distances[&team].len() {
            self._distances.get_mut(&team).unwrap()[idx] = PATH_UNKNOWN_DISTANCE;
        }
        for pos in open_list {
            self._set_distance_to_goal(team, *pos, 0);
        }
    }

    fn _relax_distances_and_expand(
        &mut self,
        open_list: Vec<(u16, u16)>,
        step: i32,
        team: usize,
        wld: &World,
    ) -> Vec<(u16, u16)> {
        let mut next_open_list: Vec<(u16, u16)> = Vec::new();
        for pos in open_list {
            for dir in &ACT_MOVES {
                let next_pos = wld.neighbor(pos, *dir, team, PATH_DONT_PROPAGATE_INTO);
                // propogate forest to forest, but not forest to grass:
                if self.distance_to_goal(next_pos, team) == PATH_UNKNOWN_DISTANCE
                    && (!PATH_DONT_PROPAGATE_OUT_OF.contains(wld.glyph_at(pos))
                        || wld.glyph_at(next_pos) == wld.glyph_at(pos))
                {
                    self._set_distance_to_goal(team, next_pos, step + 1);
                    next_open_list.push(next_pos);
                }
            }
        }
        next_open_list
    }

    fn _locate_enemies(&self, team: usize, actors: &[Actor]) -> Vec<(u16, u16)> {
        actors
            .iter()
            .filter(|actor| actor.is_enemy_of(team) && actor.invis == 0)
            .map(|actor| actor.pos)
            .collect()
    }

    fn _locate_leaders(&self, team: usize, actors: &[Actor]) -> Vec<(u16, u16)> {
        actors
            .iter()
            .filter(|actor| actor.is_leader && actor.team == team)
            .map(|actor| actor.pos)
            .collect()
    }

    pub fn distance_to_goal(&self, from: (u16, u16), team: usize) -> i32 {
        if let Some(distances) = self._distances.get(&team) {
            return distances[(from.1 * self._world_size.0 + from.0) as usize];
        }
        0
    }

    fn _set_distance_to_goal(&mut self, team: usize, pos: (u16, u16), val: i32) {
        if let Some(field) = self._distances.get_mut(&team) {
            field[(pos.1 * self._world_size.0 + pos.0) as usize] = val;
        }
    }

    pub fn is_near_enemy(&self, pos: (u16, u16), team: usize) -> bool {
        (self.is_attacking(team) || self.is_retreating(team))
            && self.distance_to_goal(pos, team) < 10
    }

    pub fn num_enemies(&self) -> usize {
        self._enemies[&0]
    }

    pub fn whos_at(&self, pos: (u16, u16)) -> Option<&usize> {
        self._occupied_cells.get(&pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter::FromIterator;

    fn fixtures() -> (Plan, World, Vec<Actor>, HashSet<usize>) {
        let team_idxs = HashSet::from_iter(vec![0, 1]);
        let mut world = World::new("glad");
        world.reshape((5, 5));
        let plan = Plan::new((5, 5), &team_idxs);
        let actors = vec![Actor::new(1, 1, 0, (0, 0)), Actor::new(1, 1, 1, (1, 4))];
        return (plan, world, actors, team_idxs);
    }

    #[test]
    fn test_tactics() {
        let (mut plan, _, _, _) = fixtures();
        plan.tactic_defend((2, 2));
        assert!(plan.is_defending(0));
        assert!(!plan.is_defending(1));
        assert_eq!(plan._muster_point(0), (2, 2));
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
            assert!(plan._distances[&actor.team]
                .iter()
                .all(|distance| distance != &PATH_UNKNOWN_DISTANCE));
        }
    }
}
