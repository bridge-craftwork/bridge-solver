//! Bridge Double-Dummy Solver - Port of macroxue/bridge-solver
//!
//! This is a faithful Rust port of the C++ solver from:
//! https://github.com/macroxue/bridge-solver
//!
//! The algorithm uses:
//! - Alpha-beta search with MTD(f) driver
//! - Pattern-based transposition table with hierarchical bounds caching
//! - Move ordering heuristics for efficient pruning
//! - Fast trick estimation for early cutoffs
//!
//! # Example using bridge-types
//!
//! ```
//! use bridge_solver::{Hands, Solver, CutoffCache, PatternCache, NOTRUMP, WEST};
//! use bridge_types::Deal;
//!
//! let deal = Deal::from_pbn("N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72").unwrap();
//! let hands = Hands::from_deal(&deal);
//!
//! let mut cutoff = CutoffCache::new(16);
//! let mut pattern = PatternCache::new(16);
//! let solver = Solver::new(hands, NOTRUMP, WEST);
//! let ns_tricks = solver.solve_with_caches(&mut cutoff, &mut pattern);
//! ```

mod bridge_solver;
mod cache;
pub mod cards;
mod convert;
mod hands;
pub mod par;
mod pattern;
mod play;
mod search;
pub mod types;

pub use bridge_solver::{
    get_node_count, order_follows, order_leads, set_no_pruning, set_no_rank_skip, set_no_tt,
    set_show_perf, set_xray_limit, OrderedCards, PartialTrick, PlayedCard, Solver,
};
pub use cards::Cards;
pub use convert::{direction_to_seat, seat_to_direction};
pub use hands::Hands;
pub use par::{par, solve_dd_table, DdTricks, ParContract, ParResult, Side};
pub use pattern::PatternCache;
pub use search::{slow_trump_tricks_opponent, CutoffCache};
pub use types::{Seat, Suit, NOTRUMP, NUM_RANKS, NUM_SEATS, NUM_SUITS, TOTAL_CARDS, TOTAL_TRICKS};
pub use types::{CLUB, DIAMOND, HEART, SPADE};
pub use types::{EAST, NORTH, SOUTH, WEST};

#[cfg(test)]
mod tests;
