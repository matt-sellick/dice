use std::io::{stdout, Write};

use dice::input_handling;

// command line dice roller
// trivial change

fn main() {
    print!("\nEnter command (or 'help' / 'quit'):");
    loop {

        // get input
        print!("\nRoll: ");
        stdout().flush().unwrap();
        let input = dice::get_input();
        match &input.trim().to_lowercase()[..] {
            "help" => {
                dice::help();
                continue;
            },
            "quit" | "exit" => break,
            _ => ()
        }
        
        // roll
        match input_handling::generate_dice(input) {
            Ok((code, dice, modifiers, log)) => {
                match dice::throw(code, dice, modifiers, log) {
                    Some(result) => {
                        println!("Result: {result}");
                        continue;
                    },
                    None => break,
                }
            },
            Err(error) => {
                println!("{error}");
                continue;
            },
        }
    }
}
