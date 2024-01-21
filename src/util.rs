use std::io::stdin;

use termion::input::TermRead;
use termion::terminal_size;

use crate::die::D;

// utility functions

pub fn press_to_continue() { // suspends program while waiting for user to press a key
    let input = stdin();
    for key in input.keys() {
        match key.unwrap() {
            _ => break,
        }
    }
}

pub fn centre(msg: &str) -> u16 { // returns a column value that will make a message centred in the terminal
    let (col, _) = terminal_centre();
    col.checked_sub(msg.len() as u16 / 2).unwrap_or(1)
}

pub fn terminal_centre() -> (u16, u16) {
    let (width, height) = terminal_size().unwrap();
    let col = width / 2;
    let row = height / 2;
    (col, row)
}

pub fn is_two_digits(face: u16, kind: D) -> bool {
    if face >= 10 || kind == D::PercentTens {
        true
    } else {
        false
    }
}