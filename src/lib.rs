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

#[cfg(feature = "play-analysis")]
pub mod analyse_play;
mod bridge_solver;
mod cache;
pub mod cards;
pub mod claim;
pub mod claim_bench;
mod convert;
pub mod dd_bench;
mod hands;
#[cfg(feature = "json")]
pub mod json_api;
pub mod par;
mod pattern;
mod pattern_vec;
mod play;
mod search;
pub mod types;

pub use bridge_solver::{
    get_node_count, order_follows, order_leads, set_no_pruning, set_no_rank_skip, set_no_tt,
    set_show_perf, set_xray_cache_window, set_xray_limit, OrderedCards, PartialTrick, PlayedCard,
    Solver,
};
pub use cards::Cards;
pub use claim::{
    ClaimError, ClaimLimit, ClaimPosition, ClaimResult, ClaimVerdict, DEFAULT_CLAIM_BUDGET,
};
pub use convert::{direction_to_seat, seat_to_direction};
pub use hands::Hands;
pub use par::{
    par, solve_dd_strain, solve_dd_table, solve_dd_table_cells, solve_dd_table_with_nodes,
    ParContract, ParDeclarer, ParResult, Side, TableSolver,
};
// The table type and its axis orders belong to `bridge-types`; re-exported so a
// caller of `solve_dd_table` needs one dependency, not two.
pub use bridge_types::{DdTable, DECLARERS, STRAINS};
pub use pattern::{type_sizes, PatternCache};
pub use pattern_vec::drain_pool;
pub use search::{slow_trump_tricks_opponent, CutoffCache};
pub use types::{Seat, Suit, NOTRUMP, NUM_RANKS, NUM_SEATS, NUM_SUITS, TOTAL_CARDS, TOTAL_TRICKS};
pub use types::{CLUB, DIAMOND, HEART, SPADE};
pub use types::{EAST, NORTH, SOUTH, WEST};

/// Number of threads the command-line tools use when not told otherwise: every
/// logical processor on the machine, or 1 where that cannot be determined.
///
/// On Windows this counts all processor groups, which
/// `std::thread::available_parallelism` does not: it reports one group, so at
/// most 64 threads.
#[cfg(feature = "cli")]
pub fn default_threads() -> usize {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{
            GetActiveProcessorCount, ALL_PROCESSOR_GROUPS,
        };
        // SAFETY: takes a plain integer and touches no caller memory. It reports
        // failure by returning 0, which falls through to the std answer.
        let all_groups = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) };
        if all_groups > 0 {
            return all_groups as usize;
        }
    }
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "cli"))]
mod default_threads_tests {
    #[test]
    fn default_threads_is_at_least_the_std_answer() {
        // Windows may report more than std (other processor groups), never fewer.
        let std_answer = std::thread::available_parallelism().map_or(1, |n| n.get());
        assert!(super::default_threads() >= std_answer);
    }
}
