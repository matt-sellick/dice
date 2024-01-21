use crate::Code;
use crate::D;
use crate::input_handling::get_command_values;
use crate::graph::Graph;
use crate::util::*;

use std::io::{Stdout, Write};
use std::collections::HashMap;
use std::time::Duration;
use std::thread;

use termion::{cursor::{self, Goto}, terminal_size, color};
use termion::raw::{RawTerminal, IntoRawMode};
use termion::screen::{AlternateScreen, IntoAlternateScreen};

// struct representing the surface on which the dice are rolled
// largely concerned with displaying objects and information to the screen, and tracking information for each die

const DISPLAY_RESULTS: usize = 5; // for return strings on Normal rolls

pub struct Table {
    pub surface: RawTerminal<AlternateScreen<Stdout>>, // DOES NOT WORK IN TERMION 3.0.0
    code: Code,
    command_log: Vec<String>,
    kinds: HashMap<usize, D>, // ID, kind (updated at start of roll)
    tracker: HashMap<usize, (u16, u16)>, // ID, position (updated repeatedly during rolling)
    results: HashMap<usize, u16>, // ID, face (updated repeatedly during rolling)
    modifiers: Vec<i16>, // updated at throw
    pub graph_on: bool, // whether the results graph is on screen
    pub error_on: bool, // whether the results display error is on screen
}

impl Table {
    pub fn new(code: Code, modifiers: Vec<i16>, command_log: Vec<String>) -> Table {
        Table { 
            surface: std::io::stdout().into_alternate_screen().unwrap().into_raw_mode().unwrap(),
            code,
            command_log,
            kinds: HashMap::new(),
            tracker: HashMap::new(),
            results: HashMap::new(),
            modifiers,
            graph_on: false,
            error_on: false,
        }
    }

    pub fn update(&mut self, id: usize, face: u16, new_position: (u16, u16)) { // updates table data (die positions and faces) and redraws die when new info is sent

        // log new position, retrieve old position
        let (old_col, old_row): (u16, u16);
        let (new_col, new_row) = new_position;
        match self.tracker.insert(id, new_position) { // insert() returns "old" (/previous) value for the key
            Some((col, row)) => (old_col, old_row) = (col, row),
            None => (old_col, old_row) = (new_col, new_row),
        }

        // log new face up, make "eraser" based on old one's length
        let mut eraser = String::from(" ");
        let kind = self.kinds.get(&id).unwrap();
        if let Some(old_face) = self.results.insert(id, face) { // RESULTS MAP IS UPDATED HERE
            if is_two_digits(old_face, *kind) { // erase two spaces if the old face was double-digit (or percentile rolling zero)
                eraser.push(' ');
            }
        }

        // if die shows double digits and you're on the last col, offset draw position one column back (don't modify "actual" position) to prevent overflow
        let offset: u16;
        let (last_col, _) = terminal_size().unwrap();
        match new_col == last_col && is_two_digits(face, *kind) {
            true => offset = 1,
            false => offset = 0,
        }
        
        // ^^ there are fringe - but significant - cases where dice slip through the Die::detect_walls() overflow catcher, that this block prevents
        // basically, you need that block because if a die is single-digit on the second-last column, it could move to the last column
        // and then change to double digits and cause an overflow

        // erase old position and redraw at new
        write!(self.surface, "{}{eraser}{}{face}",
            Goto(old_col, old_row),
            Goto(new_col - offset, new_row)
        ).unwrap();
        if face == 0 && id == 0 { // for case of PercentTens die rolling 0, adds a second zero. Tens spot always has ID 0.
            write!(self.surface, "0").unwrap();
        }
        self.surface.flush().unwrap();
    }

    pub fn redraw(&mut self) {
        self.clear_screen();

        // for each die
        for (id, result) in self.results.iter() {

            // right edge overflow safety
            let (col, row) = self.tracker.get(id).expect("die location should exist");
            let kind = self.kinds.get(id).unwrap();
            let offset: u16;
            let (last_col, _) = terminal_size().unwrap();
            match *col == last_col && is_two_digits(*result, *kind) {
                true => offset = 1,
                false => offset = 0,
            }

            // actually reprint
            write!(self.surface, "{}{result}", Goto(*col - offset, *row)).unwrap();
            if *result == 0 && *id == 0 {
                write!(self.surface, "0").unwrap();
            }
        }
        
        self.surface.flush().unwrap();
        self.crit_colour();
        self.graph_on = false;
        self.error_on = false;
    }

    pub fn log_kind(&mut self, id: usize, kind: D) {
        self.kinds.insert(id, kind);
    }

    fn full_sum(&self) -> Option<i16> { // adds together all die results and modifiers
        Some(self.results.values().sum::<u16>() as i16 + self.modifiers.iter().sum::<i16>())
    }

    fn advantage(&self) -> Option<u16> { // assesses rolls with advantage
        if self.results.iter().count() > 2 {
            return None // Cannot roll with advantage on more than two dice
        }
        Some(*self.results.values().max()?)
    }

    fn disadvantage(&self) -> Option<u16> { // assessing rolls with disadvantage
        if self.results.iter().count() > 2 {
            return None // Cannot roll with disadvantage on more than two dice.
        }
        Some(*self.results.values().min()?)
    }

    fn percent_sum(&self) -> Option<u16> { // similar to regular sum but has a caveat if they're both zero
        if self.results.iter().count() > 2 {
            return None // Cannot roll percent on more than two dice.
        }
        let mut sum = self.results.values().sum::<u16>(); 
        if sum == 0 {
            sum = 100; // if you roll two zeros, that's actually 100
        }
        Some(sum)
    }

    pub fn print_throw(&mut self) {

        // display pending throws at centre
        let (mut col, mut row) = terminal_centre();
        row -= self.command_log.len() as u16 / 2;
        let roll_msg = "Rolling:";
        write!(self.surface, "{}{roll_msg}", Goto(centre(roll_msg), row - 2)).unwrap();
        for item in self.command_log.iter() {
            write!(self.surface, "{}{item}", Goto(centre(item), row)).unwrap();
            row += 1;
        }
        let (adv, disadv, percent) = ("Advantage", "Disadvantage", "Percentile");
        match self.code {
            Code::Advantage => write!(self.surface, "{}{adv}", Goto(centre(adv), row)).unwrap(),
            Code::Disadvantage => write!(self.surface, "{}{disadv}", Goto(centre(disadv), row)).unwrap(),
            Code::Percentile => write!(self.surface, "{}{percent}", Goto(centre(percent), row)).unwrap(),
            Code::Normal => row -= 1, // compensation for not needing the extra space for a code print
        }
        self.surface.flush().unwrap();

        // little loading animation
        row += 2;
        col -= 10;
        for space in 0..=20 {
            thread::sleep(Duration::from_millis(15));
            write!(self.surface, "{}-", Goto(col + space, row)).unwrap();
            self.surface.flush().unwrap();
        }
        
        // press to continue
        let msg = "Press any key to roll";
        write!(self.surface, "{}{msg}", Goto(centre(msg), row + 1)).unwrap();
        self.surface.flush().unwrap();
        press_to_continue();
    }

    pub fn show_math(&mut self) -> Result<(), &'static str> { // performs and shows calculations
        // do_math() is similar logic, but returns the calculations as a string instead of printing it in a graph

        // safety
        let (max_cols, max_rows) = terminal_size().unwrap();
        let height: u16 = (self.results.len() + self.command_log.len() + 7) as u16; // one row per result and command divider, plus 7 for header/footer/label
        let width: u16 = 34; // graph width (window needs 36 cols -- clearing one extra col on either side)
        if max_rows < height || max_cols < width + 2 {
            return Err(" Window too small to display results ");
        }

        // setup
        let mut graph = Graph::new(height as usize);
        graph.clear_area(&mut self.surface);
        let mut results = self.results.clone().drain().collect::<Vec<(usize, u16)>>();
        results.sort_by_key(|k| k.0);
        self.graph_on = true;
        self.error_on = false;

        // header
        match self.code {
            Code::Advantage => graph.print_header(&mut self.surface, "Advantage roll"),
            Code::Disadvantage => graph.print_header(&mut self.surface, "Disadvantage roll"),
            Code::Percentile => graph.print_header(&mut self.surface, "Percentile roll"),
            Code::Normal => graph.print_header(&mut self.surface, "Normal roll"),
        }

        // draw graph depending on code
        match self.code {
            Code::Advantage | Code::Disadvantage => {
                let command = self.command_log.iter().next().unwrap();
                let (_, kind, modifier) = get_command_values(command).unwrap();
                graph.print_command(&mut self.surface, command);
                
                let selected: u16; // which of the two die is chosen
                match self.code {
                    Code::Advantage => selected = self.advantage().expect("Should have been able to assess advantage"),
                    Code::Disadvantage => selected = self.disadvantage().expect("Should have been able to assess disadvantage"),
                    _ => selected = 0,
                }

                for (line, (_, result)) in results.drain(..).enumerate() {
                    graph.goto_result_line(&mut self.surface, line);
                    let result_format: String;
                    match result {
                        20 if selected == 20 && kind == D::Twenty => result_format = format!("{}{result}{}", color::Fg(color::Green), color::Fg(color::Reset)),
                        1 if selected == 1 && kind == D::Twenty => result_format = format!("{}{result}{}", color::Fg(color::Red), color::Fg(color::Reset)),
                        _ => result_format = format!("{result}"),
                    }
                    write!(self.surface, "{result_format}").unwrap();
                }

                graph.running_row += 2;
                graph.print_totals(&mut self.surface, selected, modifier);
            },
            Code::Percentile => {
                let command = self.command_log.iter().next().unwrap();
                let (.., modifier) = get_command_values(command).unwrap();
                let sum = self.percent_sum().expect("Should have been able to assess");
                graph.print_command(&mut self.surface, command);

                for (line, (_, result)) in results.drain(..).enumerate() {
                    graph.goto_result_line(&mut self.surface, line);
                    let mut result_format = String::from(result.to_string());
                    if line == 0 && result == 0 { // (this works because the tens-place die always rolls first)
                        result_format.push('0'); // push the extra zero onto the tens die if it's zero
                    }
                    write!(self.surface, "{result_format}").unwrap();
                }
                
                graph.running_row += 2;
                graph.print_totals(&mut self.surface, sum, modifier);
            },
            Code::Normal => {
                for command in self.command_log.iter() {
                    let (coefficient, kind, modifier) = get_command_values(command).unwrap();
                    let mut running_total = 0; // i.e. the result total for a specific command, before modifiers
                    graph.print_command(&mut self.surface, command);

                    for (line, (_, result)) in results.drain(..coefficient as usize).enumerate() {
                        let result_format: String; // with colour embedded
                        match result {
                            20 if kind == D::Twenty => result_format = format!("{}{result}{}", color::Fg(color::Green), color::Fg(color::Reset)),
                            1 if kind == D::Twenty => result_format = format!("{}{result}{}", color::Fg(color::Red), color::Fg(color::Reset)),
                            _ => result_format = format!("{result}"),
                        }

                        graph.goto_result_line(&mut self.surface, line);
                        write!(self.surface, "{result_format}").unwrap();

                        running_total += result;
                    }

                    graph.command_row += coefficient + 1; // skip rows after printing command & results, to set up where the next command will be
                    graph.running_row += coefficient; // skip rows *before* printing totals/modifier
                    graph.print_totals(&mut self.surface, running_total, modifier);
                }
        
                // print sum of all commands at the bottom
                let final_sum = self.full_sum().expect("Should have been able to sum results");
                write!(self.surface, "{}= {final_sum}", Goto(graph.sum_col - 2, graph.running_row + 1)).unwrap();
            },
        }

        // print key commands
        write!(self.surface, "{}t: Toggle display{}r: Make another roll{}esc: Exit",
            Goto(graph.command_col, graph.running_row + 1),
            Goto(graph.command_col, graph.running_row + 2),
            Goto(graph.command_col, graph.running_row + 3),
        ).unwrap();

        self.surface.flush().unwrap();
        Ok(())
    }

    pub fn do_math(&mut self) -> String {

        // setup
        let mut one_liner = String::new(); // return value
        let mut results = self.results.clone().drain().collect::<Vec<(usize, u16)>>();
        results.sort_by_key(|k| k.0);

        match self.code {
            Code::Advantage | Code::Disadvantage => {
                let command = self.command_log.iter().next().unwrap();
                let (_, kind, modifier) = get_command_values(command).unwrap();
                let selected: u16; // which of the two die is chosen

                match self.code {
                    Code::Advantage => selected = self.advantage().expect("Should have been able to assess advantage"),
                    Code::Disadvantage => selected = self.disadvantage().expect("Should have been able to assess disadvantage"),
                    _ => selected = 0,
                }

                for (line, (_, result)) in results.drain(..).enumerate() {
                    let result_format: String;
                    match result {
                        20 if selected == 20 && kind == D::Twenty => result_format = format!("{}{result}{}", color::Fg(color::Green), color::Fg(color::Reset)),
                        1 if selected == 1 && kind == D::Twenty => result_format = format!("{}{result}{}", color::Fg(color::Red), color::Fg(color::Reset)),
                        _ => result_format = format!("{result}"),
                    }
                    match line {
                        0 => one_liner.push_str(&result_format),
                        1 => one_liner.push_str(&format!(" | {result_format}")),
                        _ => (),
                    }
                }

                match modifier >= 0 {
                    true => one_liner.push_str(&format!(" => {selected} + {modifier} = {}", selected as i16 + modifier)),
                    false => one_liner.push_str(&format!(" => {selected} - {} = {}", modifier.abs(), selected as i16 + modifier)),
                }
            },
            Code::Percentile => {
                let command = self.command_log.iter().next().unwrap();
                let (.., modifier) = get_command_values(command).unwrap();
                let sum = self.percent_sum().expect("Should have been able to assess percentage");

                for (line, (_, result)) in results.drain(..).enumerate() {
                    let mut result_format = String::from(result.to_string());
                    if line == 0 && result == 0 { // (this works because the tens-place die always rolls first)
                        result_format.push('0'); // push the extra zero onto the tens die if it's zero
                    }
                    match line {
                        0 => one_liner.push_str(&result_format),
                        1 => one_liner.push_str(&format!(", {result_format}")),
                        _ => (),
                    }
                }
                
                match modifier >= 0 {
                    true => one_liner.push_str(&format!(" => {sum} + {modifier} = {}", sum as i16 + modifier)),
                    false => one_liner.push_str(&format!(" => {sum} - {} = {}", modifier.abs(), sum as i16 + modifier)),
                }
            },
            Code::Normal => {
                for command in self.command_log.iter() {
                    let (coefficient, kind, modifier) = get_command_values(command).unwrap();
                    let mut running_total = 0; // i.e. the result total for a specific command, before modifiers

                    for (line, (_, result)) in results.drain(..coefficient as usize).enumerate() {
                        let result_format: String; // with colour embedded
                        match result {
                            20 if kind == D::Twenty => result_format = format!("{}{result}{}", color::Fg(color::Green), color::Fg(color::Reset)),
                            1 if kind == D::Twenty => result_format = format!("{}{result}{}", color::Fg(color::Red), color::Fg(color::Reset)),
                            _ => result_format = format!("{result}"),
                        }

                        running_total += result;

                        // if there was only one command, insert individual roll results onto return, up to a maximum.
                        // will actually display six results IF there are six, but if there are more than six, it will display five then an ellipsis.
                        match line {
                            0 if self.command_log.len() == 1 => one_liner.push_str(&result_format),
                            1..=DISPLAY_RESULTS if self.command_log.len() == 1 => {
                                if line == DISPLAY_RESULTS && coefficient as usize > (DISPLAY_RESULTS + 1) {
                                    one_liner.push_str(&format!(" + ..."));
                                } else {
                                    one_liner.push_str(&format!(" + {result_format}"));
                                }
                            },
                            _ => (),
                        }
                    }

                    match modifier >= 0 {
                        true if self.command_log.len() == 1 => one_liner.push_str(&format!(" + {modifier} = {running_total} + {modifier} = ")),
                        false if self.command_log.len() == 1 => one_liner.push_str(&format!(" - {} = {running_total} - {} = ", modifier.abs(), modifier.abs())),
                        _ => (),
                    }
                }
        
                let final_sum = self.full_sum().expect("Should have been able to sum results");
                one_liner.push_str(&final_sum.to_string());
            },
        }
        one_liner
    }

    pub fn crit_colour(&mut self) { // applies green or red to crit results on d20s
        let mut d20s = self.kinds.clone();
        d20s.retain(|_, v| *v == D::Twenty);
        for id in d20s.keys() {
            let (col, row) = self.tracker.get(id).expect("die location should exist");
            match self.results.get(id).expect("results should exist") {
                1 => {
                    write!(self.surface, "{}{}{}{}",
                        Goto(*col, *row),
                        color::Fg(color::Red),
                        1,
                        color::Fg(color::Reset)
                    ).unwrap();
                },
                20 => {
                    write!(self.surface, "{}{}{}{}",
                        Goto(*col, *row),
                        color::Fg(color::Green),
                        20,
                        color::Fg(color::Reset)
                    ).unwrap();
                },
                _ => (),
            }
        }
        self.surface.flush().unwrap();  
    }

    pub fn hide_cursor(&mut self) {
        write!(self.surface, "{}", cursor::Hide).unwrap();
        self.surface.flush().unwrap();
    }

    pub fn show_cursor(&mut self) {
        write!(self.surface, "{}", cursor::Show).unwrap();
        self.surface.flush().unwrap();
    }

    pub fn clear_screen(&mut self) {
        write!(self.surface, "{}", termion::clear::All).unwrap();
        self.surface.flush().unwrap();
    }

    pub fn print_error(&mut self, error: &'static str) {
        let (_, middle) = terminal_centre();
        let help = " Resize and press 't' to try again, \n or 'r' to return to command line ";
        let print: String = format!("{error}\n{help}");
        let offset = print.lines().count() as u16 / 2;
        for (n, line) in print.lines().enumerate() {
            write!(self.surface, "{}{line}", Goto(centre(line), middle.checked_sub(offset).unwrap_or(1) + n as u16)).unwrap();
        }
        self.surface.flush().unwrap();
        self.graph_on = false;
        self.error_on = true;
    }
}