// Handling of the global game state.
use actor::Actor;
use plan::Plan;
use std::cmp;
use std::collections::{HashSet, VecDeque};
use view::View;
use world::World;

const TEAM_SIZE: usize = 4;

pub struct State {
    pub world: World,
    pub world_idx: usize,
    pub world_completed: Vec<usize>,
    pub world_desc: String,
    pub world_name: String,
    pub score: u32,
    time: u32,
    autopilot: bool,

    pub actors: Vec<Actor>,
    pub player_idx: usize,
    pub player_team: VecDeque<Actor>,
    pub team_idxs: HashSet<usize>,
    pub plan: Plan,
    pub view: View,
    spawn: Vec<Actor>,

    create_team: fn(usize, usize) -> Vec<Actor>,
    setup_scenario: fn(&mut State),
}

impl State {
    pub fn new(
        config: &str,
        create_team: fn(usize, usize) -> Vec<Actor>,
        setup_scenario: fn(&mut State),
    ) -> State {
        State {
            world: World::new(config),
            world_idx: 1,
            world_completed: Vec::new(),
            world_desc: String::new(),
            world_name: String::new(),
            time: 1,
            autopilot: false,
            score: 0,

            actors: Vec::new(),
            player_idx: 0,
            player_team: VecDeque::new(),
            team_idxs: HashSet::new(),
            plan: Plan::new((0, 0), &HashSet::new()),
            spawn: Vec::new(),
            view: View::new(),
            create_team,
            setup_scenario,
        }
    }

    pub fn add_actor(&mut self, actor: Actor) {
        let team = actor.team;
        self.actors.push(actor);
        self.team_idxs.insert(team);
    }

    pub fn loop_game(&mut self) {
        let mut player_team = (self.create_team)(0, TEAM_SIZE);
        for mut actor in player_team.drain(0..) {
            actor.is_persistent = true;
            self.player_team.push_front(actor);
        }
        while self.world_idx != 0 {
            (self.setup_scenario)(self);
            self.plan = Plan::new(self.world.size, &self.team_idxs);
            self.player_idx = 0;
            self.player_control_confirm();
            self.load_world_description();
            self.view.show();
            self.loop_turns();
            self.view.hide();
            self.actors.clear();
            self.team_idxs.clear();
        }
    }

    fn load_world_description(&mut self) {
        let name = &self.world_name.clone();
        self.player_mut().log_event(&format!("[:{}:]", name), 0);
        for line in self.world_desc.clone().lines() {
            self.view.scroll_log_up(1);
            self.player_mut().log_event(line, 0);
        }
    }

    fn extract_team(&mut self, level_up: bool) {
        for mut actor in self.actors.drain(0..) {
            actor.inventory.retain(|item| item.can_retain);
            if actor.is_persistent && actor.is_alive() {
                actor.is_leader = false;
                if level_up {
                    actor.level += 1;
                    actor.log_action("survived the battle!");
                }
                actor.recover_fully();
                self.player_team.push_front(actor);
            }
        }
    }

    fn loop_turns(&mut self) {
        let current_world_idx = self.world_idx;
        while current_world_idx == self.world_idx {
            self.give_turns();
            self.view.render(&self.world, &self.actors, self.player_idx);
            self.actors.append(&mut self.spawn);
            self.actors.retain(|a| a.is_alive() || !a.is_projectile());
            self.world.clear_debris();
            self.check_exits();
            self.time += 1;
        }
        let victory =
            self.plan.num_enemies() <= 5 && !self.world_completed.contains(&current_world_idx);
        self.extract_team(victory);
        if victory {
            self.world_completed.push(current_world_idx);
            self.score += 10 * self.player_team.len() as u32;
        }
    }

    fn give_turns(&mut self) {
        self.player_control_confirm();
        for idx in 0..self.actors.len() {
            if self.actors[idx].is_ready_to_act(self.time) {
                self.update_logs();
                self.give_turn(idx);
            }
        }
    }

    fn give_turn(&mut self, idx: usize) {
        self.plan.fast_update(&self.actors);
        let choice = if idx == self.player_idx {
            // do the expensive update while waiting for the player
            self.plan.update(&self.team_idxs, &self.world, &self.actors);
            self.choice_from_player()
        } else {
            self.choice_from_ai(idx)
        };
        // split actors, excluding current, to prevent reborrowing
        let (have_acted, yet_to_act) = (&mut self.actors).split_at_mut(idx);
        if let Some((actor, yet_to_act)) = yet_to_act.split_first_mut() {
            actor.time = self.time;
            actor.act(
                choice,
                &mut self.world,
                &self.plan,
                &mut vec![have_acted, yet_to_act],
                &mut self.spawn,
            );
            actor.update(&mut self.world);
        }
    }

    fn choice_from_ai(&mut self, idx: usize) -> u8 {
        self.actors[idx].choose(&self.world, &self.plan)
    }

    fn choice_from_player(&mut self) -> u8 {
        let player_idx = self.player_idx;
        loop {
            self.view.render(&self.world, &self.actors, self.player_idx);
            let input = if self.autopilot {
                self.plan.tactic_attack();
                self.choice_from_ai(player_idx)
            } else {
                self.view.get_key_input()
            };
            if input != 70 && input != 71 {
                self.view.scroll_log_up(0);
            }
            match input {
                32 => self.player_mut().next_skill(),
                41 => self.player_mut().inventory(),
                55 => {
                    let pos = self.player().pos;
                    self.plan.tactic_defend(pos);
                    self.player_mut()
                        .log_action("yelled, 'defend this position!'");
                }
                56 => {
                    self.plan.tactic_follow();
                    self.player_mut().log_action("yelled, 'follow me!'");
                }
                57 => {
                    self.plan.tactic_attack();
                    self.player_mut().log_action("yelled, 'attack!'");
                }
                58 => {
                    self.plan.tactic_retreat();
                    self.player_mut().log_action("yelled, 'retreat!'");
                }
                59 => {
                    self.plan.tactic_attack();
                    return self.choice_from_ai(player_idx);
                }
                60 => {
                    self.player_control_next();
                    return self.choice_from_ai(player_idx);
                }
                61..=69 => {
                    self.player_control_set_by_number(usize::from(input - 60));
                    return self.choice_from_ai(player_idx);
                }
                70 => self.view.scroll_log_up(1),
                71 => self.view.scroll_log_down(1),
                90 => {
                    if let Ok(keys) = self.view.reload_keybindings() {
                        for key in keys {
                            self.player_mut().log_event(key.as_str(), 0);
                            self.view.scroll_log_up(1);
                        }
                        self.view.scroll_log_down(2);
                    }
                }
                _ => return input,
            }
        }
    }

    fn update_logs(&mut self) {
        for (pos, txt, important) in self.world.log.drain(0..) {
            for actor in self.actors.iter_mut().filter(|a| a.is_alive()) {
                if important {
                    actor.log_event(&format!("[{}]", txt), self.time);
                } else if actor.is_near(pos) {
                    actor.log_event(txt.as_str(), self.time);
                }
            }
        }
    }

    fn check_exits(&mut self) {
        if self.player().is_ready_to_act(self.time) && self.plan.num_enemies() <= 5 {
            if let Some(exit) = self.world.exits.iter().find(|x| x.pos == self.player().pos) {
                if self.autopilot || self.view.yes_or_no("Exit?") {
                    return self.world_idx = exit.level as usize;
                }
            }
        }
    }

    pub fn player(&self) -> &Actor {
        &self.actors[self.player_idx]
    }

    pub fn player_mut(&mut self) -> &mut Actor {
        &mut self.actors[self.player_idx]
    }

    fn player_control_confirm(&mut self) {
        if self.player_idx >= self.actors.len()
            || !self.player().is_leader
            || !self.player().is_playable()
        {
            self.player_idx = 0;
            self.player_control_set_by_number(1);
        }
    }

    fn player_control_set_by_number(&mut self, mut num: usize) {
        if self.player().is_playable() {
            self.player_mut().is_leader = false;
        }
        num = cmp::min(num, self.actors.iter().filter(|a| a.is_playable()).count());
        for (idx, actor) in self.actors.iter_mut().enumerate() {
            if actor.is_playable() {
                num -= 1;
                if num == 0 {
                    self.player_idx = idx;
                    return actor.is_leader = true;
                }
            }
        }
        self.world_idx = 0;
    }

    fn player_control_next(&mut self) {
        if self.player().is_playable() {
            self.player_mut().is_leader = false;
        }
        for _ii in 0..self.actors.len() {
            self.player_idx = (self.player_idx + 1) % self.actors.len();
            if self.player().is_playable() {
                return self.player_mut().is_leader = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glad_loader;

    fn fixtures() -> State {
        let mut state = State::new(
            "glad",
            glad_loader::create_random_team,
            glad_loader::load_world_and_spawn_team,
        );
        state.world_idx = 42;
        let mut team = (state.create_team)(0, 3);
        for actor in team.drain(0..) {
            state.player_team.push_front(actor);
        }
        (state.setup_scenario)(&mut state);
        state.plan = Plan::new(state.world.size, &state.team_idxs);
        state.load_world_description();
        state.player_idx = 0;
        state.autopilot = true;
        state
    }

    #[test]
    fn test_player_control() {
        let mut state = fixtures();
        state.player_control_confirm();
        assert!(state.player().is_playable());
        assert!(state.player().is_leader);
    }

    #[test]
    fn test_loop_turns() {
        let mut state = fixtures();
        assert!(state.world_idx != 0);
        // state.view.show();
        state.loop_turns();
        // state.view.hide();
        assert!(state.world_idx == 0); // defeat condition
    }
}
