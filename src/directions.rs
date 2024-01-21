use rand::{Rng, thread_rng};

// used by dice objects to represent their direction of movement

pub enum Direction {
    None,
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

impl Direction {
    pub fn random() -> Direction {
        match thread_rng().gen_range(1..=8) {
            1 => Direction::Up,
            2 => Direction::Down,
            3 => Direction::Left,
            4 => Direction::Right,
            5 => Direction::UpLeft,
            6 => Direction::UpRight,
            7 => Direction::DownLeft,
            8 => Direction::DownRight,
            _ => Direction::None,
        }
    }
}