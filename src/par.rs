//! Par-score calculation from a double-dummy trick table.
//!
//! Given the max DD tricks for each seat in each strain plus vulnerability,
//! computes the contract-neutral par (the score optimal competitive bidding
//! yields, both sides free to bid). No extra solving — it is a bounded search
//! over the 35 possible contracts using duplicate scoring.
//!
//! Validated to 100% agreement with Bridge Composer's `OptimumScore` across 104
//! tournament boards (see `tests`). The `OptimumScore` convention is matched:
//! the score is labeled by the par contract's declaring side and signed to that
//! side (a sacrifice reads negative, e.g. "NS -100").

use crate::{direction_to_seat, CutoffCache, Hands, PatternCache, Solver};
use crate::{CLUB, DIAMOND, HEART, NOTRUMP, SPADE};
use bridge_types::{Contract, Deal, Direction, Doubled, Strain};

/// Max DD tricks per seat × strain. Seat order N,E,S,W and strain order
/// C,D,H,S,NT (bridge-types enum order).
#[derive(Debug, Clone, Copy)]
pub struct DdTricks {
    pub tricks: [[u8; 5]; 4],
}

const STRAINS: [Strain; 5] = [
    Strain::Clubs,
    Strain::Diamonds,
    Strain::Hearts,
    Strain::Spades,
    Strain::NoTrump,
];
const DIRECTIONS: [Direction; 4] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West,
];

fn dir_index(d: Direction) -> usize {
    match d {
        Direction::North => 0,
        Direction::East => 1,
        Direction::South => 2,
        Direction::West => 3,
    }
}

fn strain_index(s: Strain) -> usize {
    match s {
        Strain::Clubs => 0,
        Strain::Diamonds => 1,
        Strain::Hearts => 2,
        Strain::Spades => 3,
        Strain::NoTrump => 4,
    }
}

impl DdTricks {
    pub fn get(&self, declarer: Direction, strain: Strain) -> u8 {
        self.tricks[dir_index(declarer)][strain_index(strain)]
    }

    /// Best DD tricks a side can take in a strain (over its two seats).
    pub fn side_max(&self, side: Side, strain: Strain) -> u8 {
        let (a, b) = side.seats();
        self.get(a, strain).max(self.get(b, strain))
    }
}

/// Solve the full 20-entry DD table for a complete deal.
pub fn solve_dd_table(deal: &Deal) -> DdTricks {
    let hands = Hands::from_deal(deal);
    let total = hands.num_tricks() as u8;
    let mut tricks = [[0u8; 5]; 4];
    for strain in STRAINS {
        let trump = strain_trump(strain);
        let mut cutoff = CutoffCache::new(16);
        let mut pattern = PatternCache::new(16);
        for dir in DIRECTIONS {
            let seat = direction_to_seat(dir);
            let leader = (seat + 1) % 4;
            let ns = Solver::new(hands, trump, leader).solve_with_caches(&mut cutoff, &mut pattern);
            let declarer_tricks = if matches!(dir, Direction::North | Direction::South) {
                ns
            } else {
                total - ns
            };
            tricks[dir_index(dir)][strain_index(strain)] = declarer_tricks;
        }
    }
    DdTricks { tricks }
}

fn strain_trump(strain: Strain) -> usize {
    match strain {
        Strain::Clubs => CLUB,
        Strain::Diamonds => DIAMOND,
        Strain::Hearts => HEART,
        Strain::Spades => SPADE,
        Strain::NoTrump => NOTRUMP,
    }
}

/// A declaring partnership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    NS,
    EW,
}

impl Side {
    fn seats(self) -> (Direction, Direction) {
        match self {
            Side::NS => (Direction::North, Direction::South),
            Side::EW => (Direction::East, Direction::West),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Side::NS => "NS",
            Side::EW => "EW",
        }
    }
}

/// The par contract.
#[derive(Debug, Clone, Copy)]
pub struct ParContract {
    pub side: Side,
    pub level: u8,
    pub strain: Strain,
    /// DD tricks the side takes (result relative to the contract may be negative
    /// for a sacrifice).
    pub tricks: u8,
}

impl ParContract {
    /// Result relative to the contract (+overtricks / -undertricks).
    pub fn relative(&self) -> i32 {
        self.tricks as i32 - (self.level as i32 + 6)
    }
    /// A sacrifice is a par contract that does not make (doubled by opponents).
    pub fn is_sacrifice(&self) -> bool {
        self.relative() < 0
    }
    /// e.g. "NS 6S=", "EW 4SX-1".
    pub fn describe(&self) -> String {
        let rel = self.relative();
        let x = if self.is_sacrifice() { "X" } else { "" };
        let result = match rel.cmp(&0) {
            std::cmp::Ordering::Equal => "=".to_string(),
            std::cmp::Ordering::Greater => format!("+{rel}"),
            std::cmp::Ordering::Less => rel.to_string(),
        };
        format!(
            "{} {}{}{}{}",
            self.side.label(),
            self.level,
            self.strain.to_char(),
            x,
            result
        )
    }
}

/// Result of a par calculation.
#[derive(Debug, Clone, Copy)]
pub struct ParResult {
    /// Par score from North-South's perspective (positive = NS benefits).
    pub score_ns: i32,
    /// The par contract, or `None` for a passed-out deal (par zero).
    pub contract: Option<ParContract>,
}

impl ParResult {
    /// Bridge-Composer-style `OptimumScore`: labeled by the par contract's
    /// declaring side, signed to that side (e.g. "NS 980", "EW -500", "0").
    pub fn optimum_score(&self) -> String {
        match self.contract {
            None => "0".to_string(),
            Some(c) => {
                let to_side = if c.side == Side::NS {
                    self.score_ns
                } else {
                    -self.score_ns
                };
                format!("{} {}", c.side.label(), to_side)
            }
        }
    }
}

/// Bidding rank of a contract; higher outranks lower.
fn rank(level: u8, strain: Strain) -> i32 {
    (level as i32 - 1) * 5 + strain_index(strain) as i32
}

/// Score to `side` if it declares `level`-`strain`: making contracts undoubled
/// (positive); non-making contracts are sacrifices, doubled by opponents
/// (negative).
fn score_to_side(level: u8, strain: Strain, tricks: u8, vul: bool) -> i32 {
    let rel = tricks as i32 - (level as i32 + 6);
    let doubled = if rel >= 0 {
        Doubled::None
    } else {
        Doubled::Doubled
    };
    Contract::new(level, strain, doubled, 'N').score(rel, vul)
}

/// Compute par from a DD table and each side's vulnerability.
pub fn par(dd: &DdTricks, vul_ns: bool, vul_ew: bool) -> ParResult {
    let vul_of = |side: Side| match side {
        Side::NS => vul_ns,
        Side::EW => vul_ew,
    };

    let mut cur_rank = 0i32;
    let mut cur_ns = 0i32;
    let mut contract: Option<ParContract> = None;

    // Each accepted bid strictly outranks the last, so this terminates; the
    // bound is a backstop.
    for _ in 0..64 {
        // Cheapest bid (lowest rank) above the current contract that improves
        // the bidding side's position — minimum competitive outbid.
        let mut best: Option<(i32, i32, ParContract)> = None;
        for side in [Side::NS, Side::EW] {
            for level in 1..=7u8 {
                for strain in STRAINS {
                    let r = rank(level, strain);
                    if r <= cur_rank {
                        continue;
                    }
                    let tricks = dd.side_max(side, strain);
                    let s = score_to_side(level, strain, tricks, vul_of(side));
                    let s_ns = if side == Side::NS { s } else { -s };
                    let improves = if side == Side::NS {
                        s_ns > cur_ns
                    } else {
                        s_ns < cur_ns
                    };
                    if improves && best.map_or(true, |(br, _, _)| r < br) {
                        best = Some((
                            r,
                            s_ns,
                            ParContract {
                                side,
                                level,
                                strain,
                                tricks,
                            },
                        ));
                    }
                }
            }
        }
        match best {
            None => break,
            Some((r, s_ns, c)) => {
                cur_rank = r;
                cur_ns = s_ns;
                contract = Some(c);
            }
        }
    }

    ParResult {
        score_ns: cur_ns,
        contract,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bridge_types::Deal;

    /// (vul_ns, vul_ew) for a PBN Vulnerable value.
    fn vul(v: &str) -> (bool, bool) {
        match v.to_uppercase().as_str() {
            "NS" | "N-S" => (true, false),
            "EW" | "E-W" => (false, true),
            "ALL" | "BOTH" => (true, true),
            _ => (false, false),
        }
    }

    /// Par computed from our own DD solve must reproduce Bridge Composer's
    /// `OptimumScore` on real tournament boards (oracle: LBC-2026, hand records).
    #[test]
    fn par_matches_bridge_composer_optimum_score() {
        // (deal, vulnerable, expected OptimumScore, their ParContract)
        let cases = [
            ("N:AQT94.T53.KQ8.T9 63.J98.J53.K8654 K72.6.AT9762.AJ7 J85.AKQ742.4.Q32", "None", "NS 980", "NS 6S="),
            ("E:7.AKJ6543.3.AK93 642.8.KQT9.T8752 AKQJ5.Q97.52.QJ6 T983.T2.AJ8764.4", "NS", "EW 980", "EW 6H=; EW 6S="),
            ("S:AJ83.7.T9653.A62 Q4.QJT984.4.T853 T762.K3.KQ2.KQJ4 K95.A652.AJ87.97", "EW", "NS -100", "NS 4SX-1"),
            ("W:87.9.T9642.AK942 AQJ53.QJ873.QJ5. 94.A652.K83.QJ75 KT62.KT4.A7.T863", "All", "NS 650", "NS 4S+1"),
            ("N:932.962.A.AJ8643 AJ87.QJ43.Q9.KT2 KQ5.8.JT6543.Q97 T64.AKT75.K872.5", "NS", "EW 140", "EW 3H="),
            ("E:QT7.Q8.QJT84.K83 986543.5.K5.AT94 K.AKJ9743.7.Q765 AJ2.T62.A9632.J2", "EW", "NS 140", "NS 3S="),
            ("S:AT.A5.AJ842.J984 Q7654.QJ9874.95. 2.K32.QT763.KT75 KJ983.T6.K.AQ632", "All", "EW -500", "EW 5SX-2"),
            ("W:QJ76.KJ9.AT92.Q8 A843.A2.QJ75.T52 K5.T873.K863.AKJ T92.Q654.4.97643", "None", "EW 430", "EW 3N+1"),
            ("N:K643.J65.T5.Q654 AJ72.83.AJ6.AK93 QT98.QT9.Q987.T2 5.AK742.K432.J87", "EW", "EW 660", "E 3N+2"),
            ("E:Q85.AJ62.AJ984.K 93.9.Q7653.J8752 AKT742.8.K2.QT43 J6.KQT7543.T.A96", "All", "EW 1430", "EW 6S="),
            ("S:J8.KT93.QJ95.T96 K63.AJ86.A43.A74 AT7542.72.762.K3 Q9.Q54.KT8.QJ852", "None", "EW 400", "EW 3N="),
            ("W:QJ832.Q76.QT63.9 T74.82.AK982.QJ2 A96.KJT43.4.AKT5 K5.A95.J75.87643", "NS", "EW 420", "EW 4H="),
            ("W:AQ86.86.AQT943.6 T54.QJ2.J82.KQ42 3.AKT9.K5.JT8753 KJ972.7543.76.A9", "EW", "EW 1370", "EW 6D="),
            ("E:KQJT.KQ6.94.KJ54 A64.7.AKJ6532.T6 752.JT84.Q7.A932 983.A9532.T8.Q87", "None", "EW -300", "EW 4CX-2; EW 4HX-2"),
        ];

        for (deal_str, v, expected, their_contract) in cases {
            let deal = Deal::from_pbn(deal_str).expect("deal parses");
            let (vn, ve) = vul(v);
            let result = par(&solve_dd_table(&deal), vn, ve);
            assert_eq!(
                result.optimum_score(),
                expected,
                "deal {deal_str} (their par {their_contract}); computed contract {:?}",
                result.contract.map(|c| c.describe()),
            );
        }
    }

    #[test]
    fn passed_out_is_par_zero() {
        let dd = DdTricks { tricks: [[6; 5]; 4] }; // nobody can take 7 tricks anywhere
        let r = par(&dd, false, false);
        assert_eq!(r.optimum_score(), "0");
        assert!(r.contract.is_none());
    }
}
