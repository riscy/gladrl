// Handles interactivity: reading keys, writing to the screen.
use std::time::Duration;
use std::thread::sleep;
use std::collections::HashMap;
use std::cmp;
use csv;
use ncurses::*;
use state::State;
use actor::Actor;
use world::World;
use item::Item;

const RENDER: bool = true;

pub struct View {
    animation_delay: u64,
    scrollback: usize,
    screen_xy: (i32, i32),
    keybindings: HashMap<i32, usize>,
}

impl View {
    pub fn new() -> View {
        let mut view = View {
            animation_delay: 0,
            scrollback: 0,
            screen_xy: (0, 0),
            keybindings: HashMap::new(),
        };
        view.reset(0);
        view.reload_keybindings();
        view
    }

    pub fn reset(&mut self, animation_delay: u64) {
        let (mut max_x, mut max_y) = (0, 0);
        getmaxyx(stdscr(), &mut max_y, &mut max_x);
        if max_y != self.screen_xy.1 || max_x != self.screen_xy.0 {
            self.screen_xy = (max_x, max_y);
            clear();
        }
        self.animation_delay = animation_delay;
    }

    pub fn reload_keybindings(&mut self) -> Vec<String> {
        self.keybindings.clear();
        let mut online_help = vec!["[Reloading config/keybindings.csv...]".to_owned()];
        let mut reader = csv::Reader::from_file("config/keybindings.csv").unwrap();
        for record in reader.decode() {
            let (kbd, num, desc): (String, usize, String) = record.unwrap();
            online_help.push(format!("{} --{}", kbd, desc));
            let kbd_char = kbd.chars().nth(0).unwrap() as i32;
            self.keybindings.insert(kbd_char, num);
        }
        self.keybindings.insert(KEY_UP as i32, 0);
        self.keybindings.insert(KEY_RIGHT as i32, 2);
        self.keybindings.insert(KEY_DOWN as i32, 4);
        self.keybindings.insert(KEY_LEFT as i32, 6);
        online_help
    }

    pub fn get_key_input(&mut self) -> u8 {
        loop {
            match self.keybindings.get(&getch()) {
                Some(input) => return *input as u8,
                None => continue,
            }
        }
    }

    pub fn render(&self, gs: &State) {
        if !RENDER {
            return;
        }
        let focus = gs.player().pos;
        let (min_x, min_y, max_x, max_y) = self.rect_around(focus, &gs.world);
        self.render_world(gs, (min_x, min_y, max_x, max_y));
        let xx = (max_x - min_x) as i32;
        let yy = self.render_roster(&gs.actors, xx);
        let rows = (max_y - min_y) as usize - yy as usize;
        self.render_log(&gs.player().log, (xx + 1, yy), rows, gs.player().time);
        mv(i32::from(focus.1) - min_y, i32::from(focus.0) - min_x);
        refresh();
        sleep(Duration::from_millis(self.animation_delay));
    }

    fn render_world(&self, state: &State, rect: (i32, i32, i32, i32)) {
        let (min_x, min_y, max_x, max_y) = rect;
        for y in min_y..max_y {
            for x in min_x..max_x {
                mv(y - min_y, x - min_x);
                self.render_cell((x as u16, y as u16), &state.actors, &state.world);
            }
        }
    }

    pub fn yes_or_no(&self, prompt: &str) -> bool {
        mv(0, 0);
        clrtoeol();
        printw(format!("{} (Y/n) ", prompt).as_str());
        loop {
            match char::from(getch() as u8) {
                'Y' | ';' => return true,
                'n' => return false,
                _ => {}
            }
        }
    }

    /// Draw actors on top of items on top of exits on top of corpses.
    fn render_cell(&self, pos: (u16, u16), actors: &[Actor], world: &World) {
        assert!(!world.is_out_of_bounds((pos.0 as i16, pos.1 as i16)));
        if let Some(actor) = actors
               .iter()
               .find(|a| a.pos == pos && a.invis == 0 && a.is_alive()) {
            return self.render_actor(actor);
        } else if let Some(item) = world.items.iter().rev().find(|i| i.pos == pos) {
            return self.render_item_or_exit(item);
        } else if let Some(exit) = world.exits.iter().find(|ex| ex.pos == pos) {
            return self.render_item_or_exit(exit);
        } else if let Some(actor) = actors.iter().find(|a| a.pos == pos && a.invis == 0) {
            return self.render_actor(actor);
        }
        self.render_floor(world, pos)
    }

    fn render_actor(&self, actor: &Actor) {
        let color = match self.animation_delay != 0 {
            true => self.actor_status_color(actor),
            false => self.actor_color(actor),
        };
        attron(COLOR_PAIR(color));
        addch(actor.glyph() as chtype);
        attroff(COLOR_PAIR(color));
    }

    fn actor_color(&self, actor: &Actor) -> i16 {
        if !actor.is_alive() || actor.is_undead() {
            return COLOR_RED;
        } else if actor.is_projectile() {
            return COLOR_YELLOW;
        }
        let mut color = match actor.team {
            1 => COLOR_GREEN,
            2 => COLOR_BLUE,
            3 => COLOR_YELLOW,
            4 => COLOR_MAGENTA,
            5 => COLOR_CYAN,
            6 => COLOR_RED,
            7 => COLOR_WHITE,
            _ => 0,
        };
        if !actor.is_projectile() && color != 0 {
            color += 100;
        }
        color
    }

    fn actor_status_color(&self, actor: &Actor) -> i16 {
        if actor.is_alive() && actor.is_hurt() && actor.is_flesh() {
            return 100 + COLOR_RED;
        } else if actor.is_leader && actor.team != 0 {
            return 100 + COLOR_CYAN;
        }
        self.actor_color(actor)
    }

    fn render_item_or_exit(&self, item: &Item) {
        attron(COLOR_PAIR(item.color));
        addch(item.glyph as chtype);
        attroff(COLOR_PAIR(item.color));
    }

    fn render_floor(&self, world: &World, pos: (u16, u16)) {
        let (character, color) = world.tile_at(pos);
        attron(COLOR_PAIR(color));
        addch(character as chtype);
        attroff(COLOR_PAIR(color));
    }

    fn render_roster(&self, actors: &[Actor], col: i32) -> i32 {
        let mut idx = 0;
        for actor in actors.iter().filter(|actor| actor.is_playable()) {
            mv(idx, col);
            clrtoeol();
            if actor.is_leader {
                attron(COLOR_PAIR(COLOR_WHITE + 100));
            }
            printw(&format!("{:>2}> ", idx + 1));
            printw(&format!("{:<width$} ", actor.name, width = 10));
            if actor.is_leader {
                attroff(COLOR_PAIR(COLOR_WHITE + 100));
            }
            if actor.is_hurt() {
                attron(COLOR_PAIR(COLOR_RED + 100));
            }
            printw(&format!("{:>3}/{:<3} ", actor.health, actor.max_health()));
            if actor.is_hurt() {
                attroff(COLOR_PAIR(COLOR_RED + 100));
            }
            printw(&format!("{:>3}/{:<3} ", actor.mana, actor.max_mana()));
            idx += 1;
        }
        mv(idx, col);
        clrtoeol();
        idx + 1 as i32
    }

    pub fn scroll_log_up(&mut self, amt: usize) {
        if amt == 0 {
            self.scrollback = 0;
        }
        self.scrollback += amt;
    }

    pub fn scroll_log_down(&mut self, mut amt: usize) {
        if amt == 0 {
            amt = 12;
        }
        if amt > self.scrollback {
            return self.scrollback = 0;
        }
        self.scrollback -= amt
    }

    fn render_log(&self,
                  log: &[(u32, String, usize)],
                  pos: (i32, i32),
                  mut rows: usize,
                  time: u32) {
        let amount_to_show = cmp::min(rows, log.len());
        if self.scrollback > 0 {
            rows -= 1;
        }
        for row in 0..rows {
            mv(row as i32 + pos.1, pos.0);
            clrtoeol();
            let idx = row + log.len() - amount_to_show - self.scrollback;
            if idx < log.len() {
                let entry = &log[idx as usize];
                if entry.1.starts_with('[') {
                    attron(COLOR_PAIR(COLOR_RED));
                } else if entry.0 >= time {
                    attron(COLOR_PAIR(COLOR_YELLOW));
                }
                printw(&entry.1);
                if entry.2 > 1 {
                    printw(&format!(" ({}x)", entry.2));
                }
                if entry.1.starts_with('[') {
                    attroff(COLOR_PAIR(COLOR_RED));
                } else if entry.0 >= time {
                    attroff(COLOR_PAIR(COLOR_YELLOW));
                }
            }
        }
        if self.scrollback > 0 {
            mv(rows as i32 + pos.1, pos.0);
            clrtoeol();
            attron(COLOR_PAIR(COLOR_CYAN));
            printw(&format!("({:>2} more lines: scroll with <,>)", self.scrollback));
            attroff(COLOR_PAIR(COLOR_CYAN));
        }
    }

    fn rect_around(&self, focus: (u16, u16), world: &World) -> (i32, i32, i32, i32) {
        let hlf_width = cmp::min((self.screen_xy.0 / 4) as u16, world.size.0 / 2);
        let hlf_height = cmp::min((self.screen_xy.1 / 2) as u16, world.size.1 / 2);
        let x_range = {
            if focus.0 < hlf_width {
                (0, hlf_width * 2)
            } else if focus.0 + hlf_width > world.size.0 {
                (world.size.0 - hlf_width * 2, world.size.0)
            } else {
                (focus.0 - hlf_width, focus.0 + hlf_width)
            }
        };
        let y_range = {
            if focus.1 < hlf_height {
                (0, hlf_height * 2)
            } else if focus.1 + hlf_height > world.size.1 {
                (world.size.1 - hlf_height * 2, world.size.1)
            } else {
                (focus.1 - hlf_height, focus.1 + hlf_height)
            }
        };
        (i32::from(x_range.0), i32::from(y_range.0), i32::from(x_range.1), i32::from(y_range.1))
    }
}

pub fn start_ncurses() {
    if !RENDER {
        return;
    }
    initscr();
    start_color();
    for color in COLOR_BLACK..COLOR_WHITE + 1 {
        init_pair(color, color, COLOR_BLACK);
        init_pair(color + 100, COLOR_WHITE, color);
    }
    // minor adjustments:
    init_pair(COLOR_GREEN + 100, COLOR_BLACK, COLOR_GREEN);
    init_pair(COLOR_YELLOW + 100, COLOR_BLACK, COLOR_YELLOW);
    init_pair(COLOR_CYAN + 100, COLOR_BLACK, COLOR_CYAN);
    init_pair(COLOR_WHITE + 100, COLOR_BLACK, COLOR_WHITE);
    keypad(stdscr(), true);
    cbreak();
    noecho();
    clear();
}

pub fn end_ncurses() {
    clear();
    endwin();
}
