pub mod board;
pub mod gamestate;
pub mod pieces;
pub mod player;

use board::{Board, Hole};
use gamestate::State;
use pieces::Piece;
use player::Player;

pub fn new_game() -> State {
    let players = Player::new_players("Connor".to_owned(), "Corban".to_owned());
    State::new(players)
}
