use crate::util::*;

use std::io::{Write, Stdout};

use termion::raw::RawTerminal;
use termion::screen::AlternateScreen;
use termion::cursor::Goto;

// struct representing the object printed to the screen that shows the roll results and its associated math.
// mostly relied on to help the Table show_math() function be less verbose.

pub struct Graph {
    label: String,
    divider: &'static str,
    width: u16,
    height: u16,

    pub command_col: u16,
    pub arrow_col: u16,
    pub result_col: u16,
    big_arrow_col: u16,
    running_col: u16,
    modifier_col: u16,
    equals_col: u16,
    pub sum_col: u16,

    pub top_row: u16,
    pub first_result_row: u16,
    pub command_row: u16,
    pub running_row: u16,
}

impl Graph {
    pub fn new(height: usize) -> Graph {
        let (col, row) = terminal_centre();
        let divider = "----------------------------------";
        let mut top_row = row.checked_sub(height as u16 / 2).unwrap_or(0);
        if top_row < 1 {
            top_row = 1; // terminal ceiling starts at 1
        }
        Graph {
            label: String::new(),
            divider,
            width: divider.len() as u16 + 2,
            height: height as u16,

            command_col: col - 17, // max command width is 8 characters
            arrow_col: col - 8, // arrows are 2 characters
            result_col: col - 5, // individual results are max 2 characters
            big_arrow_col: col - 2, // =>, 2 characters
            running_col: col + 1, // max running total is in theory 99 x 20 = 1,980 (4 characters)
            modifier_col: col + 6, // modifiers are in theory max 4 characters including sign and space
            equals_col: col + 11, // =, 1 character
            sum_col: col + 13, // max 4 characters

            top_row,
            first_result_row: top_row + 4,
            command_row: top_row + 4, // holds the row that next command will be printed on
            running_row: top_row + 3, // holds the row that next total, modifier, and sum will be printed on
        }
    }

    pub fn clear_area(&self, screen: &mut RawTerminal<AlternateScreen<Stdout>>) {
        let mut row_of_spaces = String::new();
        for _ in 1..=(self.width + 2) {
            row_of_spaces.push(' ');
        }
        for row in self.top_row..=(self.top_row + self.height) {
            write!(screen, "{}{row_of_spaces}", Goto(self.command_col - 1, row)).unwrap();
        }
        screen.flush().unwrap();
    }

    pub fn print_header(&mut self, screen: &mut RawTerminal<AlternateScreen<Stdout>>, label: &str) {
        let header = "Rolls    Results       Mod  Total";
        self.label.push_str(label);
        write!(screen, "{}{header}{}{}{}{label}",
            Goto(self.command_col, self.top_row + 2),
            Goto(self.command_col, self.top_row + 3),
            self.divider,
            Goto(centre(label), self.top_row)
        ).unwrap();
    }

    pub fn print_command(&mut self, screen: &mut RawTerminal<AlternateScreen<Stdout>>, command: &str) {
        write!(screen, "{}{command}", Goto(self.command_col, self.command_row)).unwrap();
    }

    pub fn goto_result_line(&mut self, screen: &mut RawTerminal<AlternateScreen<Stdout>>, line: usize) {
        write!(screen, "{}->{}", // draw the arrow and go to the line where to print the next result
            Goto(self.arrow_col, self.command_row + line as u16),
            Goto(self.result_col, self.command_row + line as u16)
        ).unwrap();
    }

    pub fn print_totals(&mut self, screen: &mut RawTerminal<AlternateScreen<Stdout>>, total: u16, modifier: i16) { // prints the total/modifier/sum line for a command
        let mut sign = String::new();
        match modifier >= 0 {
            true => sign.push('+'), // so plus sign will print on positive modifiers and zero
            false => sign.push('-'), // so the sign is displayed with a space (consistency of the table "look")
        }
        write!(screen, "{}=>{}{total}{}{sign} {}{}{}{}={}{}",
            Goto(self.big_arrow_col, self.running_row),
            Goto(self.running_col, self.running_row),
            Goto(self.modifier_col, self.running_row),
            modifier.abs(),
            Goto(self.sum_col, self.running_row),
            total as i16 + modifier,
            Goto(self.equals_col, self.running_row),
            Goto(self.command_col, self.running_row + 1),
            self.divider
        ).unwrap();

        self.running_row += 1; // sets up to print next line
    }
}