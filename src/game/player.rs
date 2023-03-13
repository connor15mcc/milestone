use crate::ai::heuristics::{Weights, NUM_HEURISTICS};
use crate::ai::tree::SearchLimit;
use crate::game::board::Move;

use crate::ai::tree::get_best_move;

use super::gamestate::State;
use core::fmt::Debug;

use std::io;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Person {
    name: String,
}

impl Person {
    pub fn new(name: String) -> Person {
        Person { name }
    }
}

impl Player for Person {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn one_turn(&self, state: &mut State) {
        println!("Input your move:");

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                match handle_move_input(state, input.trim()) {
                    Ok(_) => (),
                    Err(e) => {
                        println!(
                            "Couldn't process that move ({e}). Please try again"
                        );
                        self.one_turn(state);
                    }
                };
            }
            Err(e) => println!("Oops. Something went wrong ({e})"),
        }

        println!("{state}");
    }
}

fn handle_move_input(
    game: &mut State,
    input: &str,
) -> Result<(), &'static str> {
    match input.split('-').collect::<Vec<&str>>()[..] {
        [a, b] => {
            let from = a.parse::<usize>();
            let to = b.parse::<usize>();
            match (from, to) {
                (Ok(origin), Ok(dest)) => game.move_piece(origin, dest, true),
                _ => Err("couldn't parse your move"),
            }
        }
        _ => Err("improperly formatted move"),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AI {
    name: String,
    weights: Weights,
    limit: SearchLimit,
}

impl Default for AI {
    fn default() -> Self {
        AI {
            name: String::default(),
            weights: [1.0; NUM_HEURISTICS],
            limit: SearchLimit::default(),
        }
    }
}

impl AI {
    pub fn from_name(name: String) -> AI {
        AI {
            name,
            weights: [1.0; NUM_HEURISTICS],
            limit: SearchLimit::default(),
        }
    }

    pub fn new(name: String, weights: Weights, limit: SearchLimit) -> AI {
        AI {
            name,
            weights,
            limit,
        }
    }
}

impl Player for AI {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn one_turn(&self, state: &mut State) {
        let sugg_move = get_best_move(state, &self.limit, &self.weights);

        let (Move::Diagonal(origin, dest) | Move::Straight(origin, dest)) =
            sugg_move.suggestion;

        state
            .move_piece(origin, dest, true)
            .expect("could not play the AI-suggested move");

        println!("{sugg_move:#?}");

        println!("{state}");
    }
}

pub trait Player {
    fn one_turn(&self, state: &mut State);

    fn name(&self) -> String;
}

#[derive(Clone, Debug, PartialEq)]
pub enum PossiblePlayer {
    Person(Person),
    AI(AI),
}

impl Default for PossiblePlayer {
    fn default() -> Self {
        PossiblePlayer::Person(Person::default())
    }
}

impl Player for PossiblePlayer {
    fn name(&self) -> String {
        match self {
            PossiblePlayer::Person(p) => p.name(),
            PossiblePlayer::AI(a) => a.name(),
        }
    }

    fn one_turn(&self, state: &mut State) {
        match self {
            PossiblePlayer::Person(p) => p.one_turn(state),
            PossiblePlayer::AI(a) => a.one_turn(state),
        }
    }
}
