mod die;
mod table;
mod util;
mod directions;
mod graph;

use crate::die::{Die, D};
use crate::input_handling::Code;
use crate::table::Table;
use crate::util::*;

use std::sync::mpsc::channel;
use std::io::{stdin, Write};
use std::time::Duration;
use std::thread;

use termion::event::Key;
use termion::cursor::Goto; // Goto: (col, row)
use termion::input::TermRead;

// functions that initiate dice rolling or process user input

/*
    Note: some interpret, on percentile throws, that 0 on the 0-9 die represents 10, and therefore 90 + 0 = 100.
    This program interprets 0 on that die as representing 0, and as such:
        0 + 10 = 10
        0 + 00 = 100
*/

pub fn throw(code: Code, dice: Vec<D>, modifiers: Vec<i16>, command_log: Vec<String>) -> Option<String> { // most of the program
    
    // setup
    let mut table = Table::new(code, modifiers, command_log);
    let (tx, rx) = channel();
    table.hide_cursor();

    // display pending throw
    table.print_throw();
    table.clear_screen();
    thread::sleep(Duration::from_millis(200));

    // throw each die on its own thread
    for (id, kind) in dice.iter().enumerate() { // ids will start at zero
        let tx_copy = tx.clone();
        let k = kind.clone(); // not sure this is efficient but ah well
        table.log_kind(id, kind.clone());
        thread::spawn(move || {
            let mut die = Die::new(id, k, tx_copy);
            die.roll();
        });
    }

    // receive rolling
    drop(tx);
    for (id, face, position) in rx {
        table.update(id, face, position); // displays and logs positions/faces up
    }

    table.redraw(); // in case dice on screen have been "erased" (caused by update() and dice overlapping, or a die running over another stationary one)
    table.crit_colour();

    // pause
    let msg = " PRESS ANY KEY ";
    let (_, row) = terminal_centre();
    write!(table.surface, "{}{msg}", Goto(centre(msg), row)).unwrap();
    table.surface.flush().unwrap();
    press_to_continue();

    // display results
    if let Err(error) = table.show_math() {
        table.print_error(error);
    }

    // allow display toggle before exiting
    let input = stdin();
    for key in input.keys() {
        match key.unwrap() {
            Key::Esc => {
                table.show_cursor();
                return None; // exit
            },
            Key::Char('t') => { // toggle between math and table
                match table.graph_on {
                    true => table.redraw(),
                    false => {
                        if table.error_on {
                            table.redraw();
                            thread::sleep(Duration::from_millis(200));
                        }
                        if let Err(error) = table.show_math() {
                            table.print_error(error);
                        }
                    }
                }
            }
            Key::Char('r') => { // return to command line
                table.show_cursor();
                return Some(table.do_math()); // return Some() to signal the user wants to reroll on returning
            },
            _ => (),
        }
    }

    None // returns None if you want program to close upon returning
}

pub fn get_input() -> String {
    let mut input_line = String::new();
    stdin().read_line(&mut input_line).expect("failed to read input");
    input_line
}

pub fn help() {
    let help = "
Enter dice rolls in the format:
'[coefficient]d[die kind]+/-[modifier]'.
Separate roll commands with commas or slashes.

Special rolls --
Advantage roll: 'adv d[dice kind]'.
Disadvantage roll: 'disadv d[dice kind]'.
Percentile roll: 'd100' or 'd%'.

Modifiers may be applied to any roll type,
but you may not add additional dice
to a special roll.

Enter 'quit' or 'exit' to close program.";

    println!("{help}");
}

pub mod input_handling {

    use crate::die::D;

    #[derive(Clone, Copy, PartialEq)]
    pub enum Code {
        Normal,
        Advantage,
        Disadvantage,
        Percentile,
    }
    
    pub fn generate_dice(input: String) -> Result<(Code, Vec<D>, Vec<i16>, Vec<String>), &'static str> { // take input string and convert to command we can use (list of die and a throw code)
    
        // setup
        const DIE_LIMIT: usize = 99;
        const ADV_PREFIX: &'static str = "adv";
        const DISADV_PREFIX: &'static str = "disadv";
        let input = input.trim().to_lowercase();
        let inputs: Vec<&str>  = input.split(&[',', '/'][..]).collect(); // command split-by characters
        let command_count = inputs.len();
    
        // things this function will return
        let mut code = Code::Normal;
        let mut dice: Vec<D> = Vec::new(); // D-types
        let mut modifiers: Vec<i16> = Vec::new(); // note that modifiers don't need to be attached to specific die, just in the right order
        let mut command_log: Vec<String> = Vec::new();
    
        for command in inputs {
            let mut command = command.trim().to_string();
    
            // identify advantage/disadvantage roll (& remove the prefixes if you find them)
            if command.starts_with(DISADV_PREFIX) {
                code = Code::Disadvantage;
                command = command.strip_prefix(DISADV_PREFIX).unwrap().trim().to_string();
            } else if command.starts_with(ADV_PREFIX) {
                code = Code::Advantage;
                command = command.strip_prefix(ADV_PREFIX).unwrap().trim().to_string();
            }
    
            // get and validate command
            let (coefficient, kind, modifier) = get_command_values(&command)?;
            if kind == D::PercentTens {
                code = Code::Percentile;
            }
            validate(code, coefficient, kind, modifier, command_count)?;
    
            // log commands
            let mut command_string = String::new();
            command_string.push_str(&format!("{coefficient}d{}", kind.as_number()));
            if modifier > 0 {
                command_string.push_str(&format!("+{}", modifier));
            } else if modifier < 0 {
                command_string.push_str(&format!("{}", modifier));
            }
            command_log.push(command_string);
    
            // load dice and modifiers in vectors
            modifiers.push(modifier);
            match code {
                Code::Normal => {
                    for _ in 1..=coefficient {
                        dice.push(kind);
                    }
                },
                Code::Advantage | Code::Disadvantage => {
                    dice.push(kind);
                    dice.push(kind);
                },
                Code::Percentile => {
                    dice.push(D::PercentTens); // could also just say "kind" here
                    dice.push(D::PercentOnes); // extra d10 (manual add)
                },
            }
        }
    
        // limit check
        if dice.len() > DIE_LIMIT {
            return Err("Cannot roll this many die");
        }
    
        Ok((code, dice, modifiers, command_log))
    }
    
    pub fn get_command_values(input: &String) -> Result<(u16, D, i16), &'static str> { // gets all command values in one go. Accepts "CdK+M" format
        let coefficient = match get_coefficient(&input) {
            Some(c) => c,
            None => return Err("Coefficient error"),
        };
        let kind = match get_kind(&input) {
            Some(k) => k,
            None => return Err("Die type error"),
        };
        let modifier = match get_modifier(&input) {
            Some(m) => m,
            None => return Err("Modifier error"),
        };
        Ok((coefficient, kind, modifier))
    }
    
    fn get_coefficient(input: &String) -> Option<u16> { // analyzes a slice for a coefficient. must be first thing in input, besides whitespace
        if !input.contains('d') { // safety: rejects if there's no 'd'
            return None;
        }
    
        // attempt to parse what comes before 'd'
        let coeff_str = input.trim().split('d').next()?;
        if coeff_str.is_empty() {
            return Some(1);
        }
        if let Ok(coefficient) = coeff_str.trim().parse::<u16>() {
            return Some(coefficient);
        }
        None // if you get to this point, something wasn't specified correctly
    }
    
    pub fn get_kind(input: &String) -> Option<D> { // analyzes a slice for die type
        if !input.contains('d') { // safety: rejects if there's no 'd'
            return None;
        }
    
        // attempt to parse what comes between the 'd' and modifier operator
        let d_str = input.split(|op| op == '+' || op == '-').next()?.split('d').last()?;
        if d_str.trim() == "%" { // this might need an escape to work
            return Some(D::PercentTens);
        }
        let die = match d_str.trim().parse::<u16>() {
            Ok(2) => D::Two,
            Ok(4) => D::Four,
            Ok(6) => D::Six,
            Ok(10) => D::Ten,
            Ok(12) => D::Twelve,
            Ok(20) => D::Twenty,
            Ok(100) => D::PercentTens, // when this is returned, the dice generator manually tosses a PercentOnes as well
            _ => return None,
        };
        Some(die)
    }
    
    fn get_modifier(input: &String) -> Option<i16> { // analyzes a slice for a modifier
        let mut operations = input.clone(); // safety: rejects input if it contains more than one modifier attempt
        operations.retain(|c| c == '+' || c == '-');
        if operations.chars().count() > 1 {
            return None; // this really is more of an "error", but going for concision here
        }
    
        // find operator and parse what comes after it. modifier must be last thing in slice for this to work
        if input.contains('+') {
            match input.split('+').last()?.trim().parse::<i16>() {
                Ok(modifier) => return Some(modifier),
                Err(_) => return None, // returns None if whatever follows the operator is not parseable
            }
        }
        if input.contains('-') {
            match input.split('-').last()?.trim().parse::<i16>() {
                Ok(value) => return Some(0 - value),
                Err(_) => return None,
            }
        }
        Some(0) // if no operator is found, modifier is zero
    }
    
    fn validate(code: Code, coefficient: u16, kind: D, modifier: i16, command_count: usize) -> Result<(), &'static str> { // validates pending commands
        
        const COEFFICIENT_LIMIT: usize = 99;
        const MODIFIER_LIMIT: usize = 99; // absolute value
    
        if coefficient == 0 {
            return Err("Coefficient cannot be zero");
        }
        if coefficient as usize > COEFFICIENT_LIMIT {
            return Err("Coefficient limit exceeded");
        }
        if modifier.abs() as usize > MODIFIER_LIMIT {
            return Err("Modifier limit exceeded");
        }
        if code != Code::Normal && coefficient != 1 {
            return Err("You cannot have a coefficient on this roll");
        }
        if (code == Code::Advantage || code == Code::Disadvantage) && kind == D::PercentTens {
            return Err("You cannot roll advantage/disadvantage on a d100"); // really it should maybe be "anything but d20"?
        }
        if code != Code::Normal && command_count != 1 {
            return Err("You cannot throw extra die on advantage, disadvantage, and percentile rolls"); // pass in vector.len() for count
        }
        Ok(())
    }
}