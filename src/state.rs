// Handling of the global game state.
use actor::Actor;
use plan::Plan;
use std::cmp;
use std::collections::{HashSet, VecDeque};
use view::View;
use world::World;

pub const AUTOPILOT: bool = false;

pub struct State {
    pub world: World,
    pub world_idx: usize,
    pub world_completed: Vec<usize>,
    pub score: u32,
    time: u32,

    pub actors: Vec<Actor>,
    pub player_idx: usize,
    pub player_team: VecDeque<Actor>,
    pub team_idxs: HashSet<usize>,
    pub plan: Plan,
    spawn: Vec<Actor>,
    view: View,
}

impl State {
    pub fn new() -> State {
        State {
            world: World::new(),
            world_idx: 1,
            world_completed: Vec::new(),
            time: 1,
            score: 0,

            actors: Vec::new(),
            player_idx: 0,
            player_team: VecDeque::new(),
            team_idxs: HashSet::new(),
            plan: Plan::new((0, 0), &HashSet::new()),
            spawn: Vec::new(),
            view: View::new(),
        }
    }

    pub fn add_actor(&mut self, actor: Actor) {
        let team = actor.team;
        self.actors.push(actor);
        self.team_idxs.insert(team);
    }

    pub fn loop_game(&mut self, setup_game: fn(&mut State), setup_scenario: fn(&mut State)) {
        setup_game(self);
        while self.world_idx != 0 {
            setup_scenario(self);
            self.plan = Plan::new(self.world.size, &self.team_idxs);
            self.player_idx = 0;
            self.player_control_confirm();
            self.load_world_description();
            self.loop_turns();
            self.actors.clear();
            self.team_idxs.clear();
        }
    }

    fn load_world_description(&mut self) {
        let name = &self.world.name.clone();
        self.player_mut().log_event(&format!("[{}]", name), 0);
        for line in self.world.desc.clone().lines() {
            self.view.scroll_log_up(1);
            self.player_mut().log_event(line, 0);
        }
    }

    fn extract_team(&mut self, level_up: bool) {
        for mut actor in self.actors.drain(0..) {
            if actor.is_persistent && actor.is_alive() {
                actor.is_leader = false;
                if level_up {
                    actor.level += 1;
                }
                actor.log.clear();
                actor.recover_fully();
                self.player_team.push_front(actor);
            }
        }
    }

    fn loop_turns(&mut self) {
        let current_world_idx = self.world_idx;
        while current_world_idx == self.world_idx {
            self.player_control_confirm();
            for idx in 0..self.actors.len() {
                if self.actors[idx].is_ready_to_act(self.time) {
                    self.update_logs();
                    self.give_turn(idx);
                }
            }
            self.update_view(true);

            self.actors.append(&mut self.spawn);
            self.actors.retain(|a| a.is_alive() || !a.is_projectile());
            self.world.items.retain(|item| !item.is_debris());
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
        let (actor, yet_to_act) = yet_to_act.split_first_mut().unwrap();
        let others = (have_acted, yet_to_act);
        actor.act(
            choice,
            self.time,
            &mut self.world,
            &self.plan,
            others,
            &mut self.spawn,
        );
        actor.update(&mut self.world);
    }

    fn choice_from_ai(&mut self, idx: usize) -> u8 {
        self.actors[idx].choose(&self.world, &self.plan)
    }

    fn choice_from_player(&mut self) -> u8 {
        let player_idx = self.player_idx;
        loop {
            self.update_view(false);
            let input = if AUTOPILOT {
                self.plan.tactic_attack();
                self.actors[player_idx].choose(&self.world, &self.plan)
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
                61...69 => {
                    self.player_control_set_by_number(input as usize - 60);
                    return self.choice_from_ai(player_idx);
                }
                70 => self.view.scroll_log_up(1),
                71 => self.view.scroll_log_down(1),
                90 => {
                    for key in self.view.reload_keybindings() {
                        self.player_mut().log_event(key.as_str(), 0);
                        self.view.scroll_log_up(1);
                    }
                    self.view.scroll_log_down(2);
                }
                _ => return input,
            }
        }
    }

    fn update_view(&mut self, is_animating: bool) {
        let animation_delay = if is_animating {
            500 / u64::from(self.player().move_lag)
        } else {
            0
        };
        let team_len = self.actors.iter().filter(|a| a.is_playable()).count();
        let log_len = self.player().log.len();
        self.view.reset(animation_delay, team_len, log_len);
        self.view.render(&self.world, &self.actors, self.player_idx);
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
                if AUTOPILOT || self.view.yes_or_no("Exit?") {
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
