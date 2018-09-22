// Handles interactivity: reading keys, writing to the screen.
use actor::Actor;
use csv;
use item::Item;
use ncurses::*;
use std::cmp;
use std::collections::HashMap;
use world::World;

pub struct View {
    headless: bool,
    scrollback: usize,
    screen_xy: (i32, i32),
    keybindings: HashMap<i32, usize>,
    last_key_pressed: i32,
    animation_frame: i32, // delay per frame
    animation_cycle: i32, // total delay
}

impl View {
    pub fn new(animation_cycle: i32) -> View {
        let mut view = View {
            headless: true,
            scrollback: 0,
            screen_xy: (0, 0),
            keybindings: HashMap::new(),
            last_key_pressed: ERR,
            animation_frame: 0,
            animation_cycle,
        };
        view.reload_keybindings();
        view
    }

    pub fn start_ncurses(&mut self) {
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
        self.headless = false;
    }

    pub fn end_ncurses(&mut self) {
        clear();
        endwin();
        self.headless = true;
    }

    fn reset(&mut self, roster_count: usize, log_len: usize) {
        let (mut max_x, mut max_y) = (0, 0);
        getmaxyx(stdscr(), &mut max_y, &mut max_x);
        if max_y != self.screen_xy.1 || max_x != self.screen_xy.0 {
            self.screen_xy = (max_x, max_y);
            clear();
        }
        let logs_to_show = cmp::min(max_y as usize - roster_count - 1, log_len);
        self.scrollback = cmp::min(log_len - logs_to_show, self.scrollback);
    }

    pub fn reload_keybindings(&mut self) -> Vec<String> {
        self.keybindings.clear();
        let mut online_help = vec!["[Reloading config/keybindings.csv...]".to_owned()];
        let mut reader = csv::Reader::from_file("config/keybindings.csv").unwrap();
        for record in reader.decode() {
            let (kbd, num, desc): (char, usize, String) = record.unwrap();
            online_help.push(format!("{} --{}", kbd, desc));
            self.keybindings.insert(kbd as i32, num);
        }
        self.keybindings.insert(KEY_UP as i32, 0);
        self.keybindings.insert(KEY_RIGHT as i32, 2);
        self.keybindings.insert(KEY_DOWN as i32, 4);
        self.keybindings.insert(KEY_LEFT as i32, 6);
        online_help
    }

    pub fn get_key_input(&mut self) -> u8 {
        loop {
            let key_pressed = match self.last_key_pressed {
                ERR => getch(),
                _ => self.last_key_pressed,
            };
            self.last_key_pressed = ERR;
            match self.keybindings.get(&key_pressed) {
                Some(input) => return *input as u8,
                None => continue,
            }
        }
    }

    pub fn render(&mut self, world: &World, actors: &[Actor], player: usize) {
        if self.headless {
            return;
        }
        self.animation_frame = self.animation_cycle / i32::from(actors[player].move_lag);
        self.reset(
            actors.iter().filter(|a| a.is_playable()).count(),
            actors[player].log.len(),
        );
        let focus = actors[player].pos;
        let (min_x, min_y, max_x, max_y) = self.rect_around(focus, world);
        self.render_world(world, actors, (min_x, min_y, max_x, max_y));
        let xx = (max_x - min_x) as i32;
        let yy = self.render_roster(actors, xx);
        self.render_log(&actors[player].log, actors[player].time, (xx + 1, yy));
        mv(i32::from(focus.1) - min_y, i32::from(focus.0) - min_x);
        refresh();
        if self.last_key_pressed == ERR {
            timeout(self.animation_frame);
            self.last_key_pressed = getch();
            timeout(-1);
        }
    }

    fn render_world(&self, world: &World, actors: &[Actor], rect: (i32, i32, i32, i32)) {
        let (min_x, min_y, max_x, max_y) = rect;
        for y in min_y..max_y {
            for x in min_x..max_x {
                mv(y - min_y, x - min_x);
                self.render_cell((x as u16, y as u16), actors, world);
            }
        }
    }

    pub fn yes_or_no(&self, prompt: &str) -> bool {
        mv(0, 0);
        clrtoeol();
        printw(format!("{} (Y/N) ", prompt).as_str());
        loop {
            match char::from(getch() as u8) {
                'Y' | ';' => return true,
                'N' => return false,
                _ => {}
            }
        }
    }

    /// Draw actors on top of items on top of exits on top of corpses.
    fn render_cell(&self, pos: (u16, u16), actors: &[Actor], world: &World) {
        assert!(!world.is_out_of_bounds((pos.0 as i16, pos.1 as i16)));
        if let Some(actor) = actors
            .iter()
            .find(|a| a.pos == pos && a.invis == 0 && a.is_alive())
        {
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
        let color = if self.animation_frame != 0 {
            self.actor_status_color(actor)
        } else {
            self.actor_color(actor)
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

    fn render_log(&self, log: &[(u32, String, usize)], time: u32, pos: (i32, i32)) {
        let height = (self.screen_xy.1 - pos.1) as usize;
        let max_amount_to_show = cmp::min(height, log.len());
        for row in 0..height {
            mv(row as i32 + pos.1, pos.0);
            clrtoeol();
            let idx = row + log.len() - self.scrollback - max_amount_to_show;
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
            mv(height as i32 - 1 + pos.1, pos.0);
            clrtoeol();
            attron(COLOR_PAIR(COLOR_CYAN));
            printw(&format!(
                "({:>2} more lines: scroll with <,>)",
                self.scrollback
            ));
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
        (
            i32::from(x_range.0),
            i32::from(y_range.0),
            i32::from(x_range.1),
            i32::from(y_range.1),
        )
    }
}
