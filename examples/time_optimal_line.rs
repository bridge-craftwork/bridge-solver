//! Time `optimal_line` over a spread of deals.
//!
//! Exists to check one claim: that seeking cards in a natural order and stopping
//! at the first free one is not slower than pricing every legal card. Run it on
//! either side of a change and compare.
//!
//! ```text
//! cargo run --release --example time_optimal_line --features play-analysis
//! ```

use std::time::Instant;

use bridge_solver::analyse_play::{optimal_line, PlayInput};
use bridge_solver::types::{SOUTH, SPADE, WEST};
use bridge_solver::Hands;

/// A spread rather than one deal: the seek order's value depends on how often
/// the first card tried is already free, which varies with the shape.
const DEALS: &[&str] = &[
    "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72",
    "N:AQ6.K95.J984.Q85 987.842.AT763.KT KJT5.AQ63.K.J642 432.JT7.Q52.A973",
    "N:AK5.AK5.AK5.AK54 QJT.QJT.QJT.QJT6 987.987.987.9873 6432.6432.6432.2",
    "N:A32.A32.A32.A432 K54.K54.K54.K765 Q76.Q76.Q76.QJT8 JT98.JT98.JT98.9",
];

fn main() {
    let mut total = 0u128;
    let mut tricks = Vec::new();

    for (i, deal) in DEALS.iter().enumerate() {
        let hands = match Hands::from_pbn(deal) {
            Some(h) => h,
            None => {
                eprintln!("deal {i} did not parse");
                continue;
            }
        };
        let input = PlayInput {
            hands,
            trump: SPADE,
            declarer: SOUTH,
            leader: WEST,
            plays: Vec::new(),
        };

        let started = Instant::now();
        let line = match optimal_line(&input, 0) {
            Ok(line) => line,
            Err(e) => {
                eprintln!("deal {i} did not solve: {e:?}");
                continue;
            }
        };
        let micros = started.elapsed().as_micros();
        total += micros;
        tricks.push(line.declaring_tricks);

        println!(
            "deal {i}: {:>8} us  {} tricks  {}",
            micros,
            line.declaring_tricks,
            line.cards.join(" ")
        );
    }

    // Printed so a comparison run can confirm the lines are still worth the same
    // thing — a faster search that changed the answer would be a regression, not
    // an improvement.
    println!("\ntotal {total} us   declaring tricks {tricks:?}");
}
