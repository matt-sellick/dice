use crate::directions::Direction;
use crate::util::*;

use std::sync::mpsc::Sender;
use std::time::Duration;
use std::thread;

use termion::terminal_size;

use rand::{Rng, thread_rng};

// structs representing dice objects, their types, and their behaviour

pub struct Die {
    id: usize,
    kind: D,
    face_up: u16,
    tx: Sender<(usize, u16, (u16, u16))>, // id, face up, and position
    position: (u16, u16), // (col, row)
    speed: i16,
    direction: Direction,
}

impl Die {
    pub fn new(id: usize, kind: D, tx: Sender<(usize, u16, (u16, u16))>) -> Die {
        const MAX_INIT_SPEED: i16 = 120; // in flips (position shifs) per second
        const MIN_INIT_SPEED: i16 = 60;
        Die {
            id,
            kind,
            face_up: kind.flip(),
            tx,
            position: Die::spawn_point(),
            speed: thread_rng().gen_range(MIN_INIT_SPEED..=MAX_INIT_SPEED),
            direction: Direction::random(),
        }
    }

    fn spawn_point() -> (u16, u16) {
        let (h, v) = terminal_size().unwrap();
        let h_radius = h / 8; // return a spawn poing somewhere within the central quarter of the window
        let v_radius = v / 8;
        let centre = terminal_centre();
        let col = thread_rng().gen_range(centre.0 - h_radius ..= centre.0 + h_radius);
        let row = thread_rng().gen_range(centre.1 - v_radius ..= centre.1 + v_radius);
        (col, row)
    }

    pub fn roll(&mut self) {
        const STOP_SPEED: i16 = 0; // seems to strike a good balance of slowing but not hanging
        while self.speed > STOP_SPEED {
            self.face_up = self.kind.flip();
            self.detect_wall(); // detects walls and changes direction if necessary
            self.movement(); // changes position
            // self._bounds_check(); // may not be necessary -> uncomment if wall bounces get buggy
            self.tx.send((self.id, self.face_up, self.position)).unwrap();
            thread::sleep(Duration::from_millis(self.flip_time()));
            self.friction(); // needs to go after sleep in order for some rolls not to hang
        }
    }

    fn movement(&mut self) { // moves the die one square along its current trajectory
        let (col, row) = self.position;
        self.position = match self.direction { // move along its direction
            Direction::None => (col, row),
            Direction::Up => (col, row - 1),
            Direction::Down => (col, row + 1),
            Direction::Left => (col - 1, row),
            Direction::Right => (col + 1, row),
            Direction::UpLeft => (col - 1, row - 1),
            Direction::UpRight => (col + 1, row - 1),
            Direction::DownLeft => (col - 1, row + 1),
            Direction::DownRight => (col + 1, row + 1),
        };
    }

    fn friction(&mut self) { // call to slow down according to resistance value
        self.speed += self.kind.acceleration()
    }

    fn detect_wall(&mut self) {
        let (l_wall, ceiling): (u16, u16) = (1, 1); // because Goto is 1-based
        let (mut r_wall, floor) = terminal_size().unwrap();
        if is_two_digits(self.face_up, self.kind) {
            r_wall -= 1; // helps prevent overflow of 2-digit die
        }
        
        // is the die about to collide with a wall given its current position and direction?
        match self.position {
            // corner bounces -> bounce diagonally outward no matter the incidence
            (c, r) if (c <= l_wall) && (r <= ceiling) => if self.will_collide(Direction::UpLeft) {
                self.direction = Direction::DownRight;
            },
            (c, r) if (c >= r_wall) && (r <= ceiling) => if self.will_collide(Direction::UpRight) {
                self.direction = Direction::DownLeft;
            },
            (c, r) if (c <= l_wall) && (r >= floor) => if self.will_collide(Direction::DownLeft) {
                self.direction = Direction::UpRight;
            },
            (c, r) if (c >= r_wall) && (r >= floor) => if self.will_collide(Direction::DownRight) {
                self.direction = Direction::UpLeft;
            },
            // collisions with single surface
            (c, _) if c <= l_wall => if self.will_collide(Direction::Left) {
                self.bounce(true);
            },
            (c, _) if c >= r_wall => if self.will_collide(Direction::Right) {
                self.bounce(true);
            },
            (_, r) if r >= floor => if self.will_collide(Direction::Down) {
                self.bounce(false);
            },
            (_, r) if r <= ceiling => if self.will_collide(Direction::Up) {
                self.bounce(false);
            },
            (_, _) => (),
        }
    }

    fn will_collide(&self, surface: Direction) -> bool { // is the die going in the right direction for a collision?
        match surface { // surface: the "side" on which the object (wall, ceiling, etc.) is positioned relative to the caller. e.g. ceiling is Up.
            Direction::None => false,
            Direction::Up => match self.direction {
                Direction::Up | Direction::UpLeft | Direction::UpRight => true,
                _ => false,
            },
            Direction::Down => match self.direction {
                Direction::Down | Direction::DownLeft | Direction::DownRight => true,
                _ => false,
            },
            Direction::Left => match self.direction {
                Direction::Left | Direction::UpLeft | Direction::DownLeft => true,
                _ => false,
            },
            Direction::Right => match self.direction {
                Direction::Right | Direction::UpRight | Direction::DownRight => true,
                _ => false,
            },
            Direction::UpLeft => match self.direction {
                Direction::Down | Direction::Right | Direction::DownRight => false,
                _ => true,
            },
            Direction::UpRight => match self.direction {
                Direction::Down | Direction::Left | Direction::DownLeft => false,
                _ => true,
            },
            Direction::DownLeft => match self.direction {
                Direction::Up | Direction::Right | Direction::UpRight => false,
                _ => true,
            },
            Direction::DownRight => match self.direction {
                Direction::Up | Direction::Left | Direction::UpLeft => false,
                _ => true,
            },
        }
    }

    fn bounce(&mut self, wall: bool) { // wall: whether the collision is against vertical surface or not
        const REDIRECT_CHANCE: f64 = 5.0; // reciprocal of chance for altered trajectory
        let redirect: bool = thread_rng().gen_bool(1.0 / REDIRECT_CHANCE); // set to zero to remove redirections
        let option: bool = thread_rng().gen_bool(1.0 / 2.0); // coin toss between two possible altered trajectories
        
        self.direction = match self.direction {
            Direction::Up => match (redirect, option) {
                (false, _) => Direction::Down,
                (true, false) => Direction::DownLeft,
                (true, true) => Direction::DownRight,
            },
            Direction::Down => match (redirect, option) {
                (false, _) => Direction::Up,
                (true, false) => Direction::UpLeft,
                (true, true) => Direction::UpRight,
            },
            Direction::Left => match (redirect, option) {
                (false, _) => Direction::Right,
                (true, false) => Direction::UpRight,
                (true, true) => Direction::DownRight,
            },
            Direction::Right => match (redirect, option) {
                (false, _) => Direction::Left,
                (true, false) => Direction::UpLeft,
                (true, true) => Direction::DownLeft,
            },
            Direction::UpLeft => match (wall, redirect) { // diagonals will only ever bouce diagonally or inward
                (false, false) => Direction::DownLeft,
                (true, false) => Direction::UpRight,
                (false, true) => Direction::Down,
                (true, true) => Direction::Right,
            },
            Direction::UpRight => match (wall, redirect) {
                (false, false) => Direction::DownRight,
                (true, false) => Direction::UpLeft,
                (false, true) => Direction::Down,
                (true, true) => Direction::Left,
            },
            Direction::DownLeft => match (wall, redirect) {
                (false, false) => Direction::UpLeft,
                (true, false) => Direction::DownRight,
                (false, true) => Direction::Up,
                (true, true) => Direction::Right,
            },
            Direction::DownRight => match (wall, redirect) {
                (false, false) => Direction::UpRight,
                (true, false) => Direction::DownLeft,
                (false, true) => Direction::Up,
                (true, true) => Direction::Left,
            },
            Direction::None => Direction::None,
        }
    }

    fn flip_time(&self) -> u64 { // calculates time between flips in ms
        (1000.0 / self.speed as f64) as u64
    }

    fn _bounds_check(&mut self) { // in case terminal gets resized smaller
        let (max_col, max_row) = terminal_size().unwrap();
        let (col, row) = self.position;
        if col > max_col {
            self.position.0 = max_col;
        }
        if row > max_row {
            self.position.1 = max_row;
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum D {
    Two,
    Four,
    Six,
    Ten,
    Twelve,
    Twenty,
    PercentTens, // use Tens as the one that the value input parser uses to communicate percentile roll
    PercentOnes,
}

impl D {
    fn flip(&self) -> u16 { // generates a new number to facing up depending on D type
        let value = thread_rng().gen_range(1..=self.value());
        match self {
            D::PercentTens => 10 * (value - 1), // 0-90, mod 10
            D::PercentOnes => value - 1, // 0-9
            _ => value, // all other cases
        }
    }

    fn acceleration(&self) -> i16 { // returns the speed lost per flip for each D type
        match self {
            D::Two => -10,
            D::Four => -7,
            D::Six => -4,
            D::Ten => -3,
            D::Twelve => -2,
            D::Twenty => -1,
            D::PercentTens | D:: PercentOnes => -3,
        }
    }

    fn value(&self) -> u16 { // for setting max_range on generator
        match self {
            D::Two => 2,
            D::Four => 4,
            D::Six => 6,
            D::Ten => 10,
            D::Twelve => 12,
            D::Twenty => 20,
            D::PercentTens => 10,
            D::PercentOnes => 10,
        }
    }

    pub fn as_number(&self) -> u16 { // for displaying as integer, not enum variant. Called by generate_dice()
        match self {
            D::Two => 2,
            D::Four => 4,
            D::Six => 6,
            D::Ten => 10,
            D::Twelve => 12,
            D::Twenty => 20,
            D::PercentTens => 100,
            D::PercentOnes => 100, // not actually needed so don't worry
        }
    }
}