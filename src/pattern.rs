//! Shape-based pattern cache for transposition table
//!
//! This implements the C++ solver's pattern cache which:
//! - Keys by hand shapes (suit lengths) rather than actual cards
//! - Stores relative hands (cards normalized within each suit)
//! - Uses a tree structure to store patterns with bounds
//!
//! This allows matching positions that are equivalent due to card equivalence.

use super::cards::{mask_of, suit_of, Cards};
use super::hands::Hands;
use super::types::*;

/// Pack bits: extract bits from source where mask has 1s, compress them to low bits
/// Example: PackBits(0b10100, 0b11100) = 0b101 (extracts bits 2,3,4 and packs to 0,1,2)
#[inline]
// `m & m.wrapping_neg()` is the classic isolate-lowest-set-bit idiom. Clippy
// (since the toolchain that stabilized it) wants `m.isolate_lowest_one()`, but
// that method is still unstable on older compilers, so adopting it would raise
// this crate's minimum Rust version for no benefit in a hot path. `unknown_lints`
// keeps compilers that predate the lint quiet about the allow itself.
#[allow(unknown_lints, clippy::manual_isolate_lowest_one)]
pub fn pack_bits(source: u64, mask: u64) -> u64 {
    #[cfg(target_feature = "bmi2")]
    {
        // Use PEXT instruction if available
        unsafe { core::arch::x86_64::_pext_u64(source, mask) }
    }
    #[cfg(not(target_feature = "bmi2"))]
    {
        if source == 0 {
            return 0;
        }
        let mut packed = 0u64;
        let mut bit = 1u64;
        let mut m = mask;
        while m != 0 {
            let lowest = m & m.wrapping_neg(); // isolate lowest bit
            if source & lowest != 0 {
                packed |= bit;
            }
            bit <<= 1;
            m &= m - 1; // clear lowest bit
        }
        packed
    }
}

/// Unpack bits: scatter source bits to positions where mask has 1s
/// Example: UnpackBits(0b101, 0b11100) = 0b10100 (scatters bits 0,1,2 to positions 2,3,4)
#[inline]
// `m & m.wrapping_neg()` is the classic isolate-lowest-set-bit idiom. Clippy
// (since the toolchain that stabilized it) wants `m.isolate_lowest_one()`, but
// that method is still unstable on older compilers, so adopting it would raise
// this crate's minimum Rust version for no benefit in a hot path. `unknown_lints`
// keeps compilers that predate the lint quiet about the allow itself.
#[allow(unknown_lints, clippy::manual_isolate_lowest_one)]
pub fn unpack_bits(source: u64, mask: u64) -> u64 {
    #[cfg(target_feature = "bmi2")]
    {
        // Use PDEP instruction if available
        unsafe { core::arch::x86_64::_pdep_u64(source, mask) }
    }
    #[cfg(not(target_feature = "bmi2"))]
    {
        if source == 0 {
            return 0;
        }
        let mut unpacked = 0u64;
        let mut bit = 1u64;
        let mut src = source;
        let mut m = mask;
        while src != 0 && m != 0 {
            if src & bit != 0 {
                unpacked |= m & m.wrapping_neg();
                src &= !bit;
            }
            bit <<= 1;
            m &= m - 1;
        }
        unpacked
    }
}

/// Shape encodes the suit lengths for all 4 hands in 64 bits
/// Each hand-suit pair gets 4 bits (0-13), arranged as:
/// bits 60-63: West spades, bits 56-59: West hearts, etc.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Shape {
    value: u64,
}

impl Shape {
    pub fn from_hands(hands: &Hands) -> Self {
        let mut value = 0u64;
        for seat in 0..NUM_SEATS {
            for suit in 0..NUM_SUITS {
                let len = hands[seat].suit(suit).size() as u64;
                value |= len << Self::offset(seat, suit);
            }
        }
        Shape { value }
    }

    /// Update shape after a trick is played
    pub fn play_cards(&mut self, seat: Seat, c1: usize, c2: usize, c3: usize, c4: usize) {
        self.value -= 1u64 << Self::offset(seat, suit_of(c1));
        self.value -= 1u64 << Self::offset((seat + 1) % NUM_SEATS, suit_of(c2));
        self.value -= 1u64 << Self::offset((seat + 2) % NUM_SEATS, suit_of(c3));
        self.value -= 1u64 << Self::offset((seat + 3) % NUM_SEATS, suit_of(c4));
    }

    pub fn value(&self) -> u64 {
        self.value
    }

    #[inline]
    fn offset(seat: Seat, suit: Suit) -> u32 {
        (60 - (seat * NUM_SUITS + suit) * 4) as u32
    }
}

/// Bounds represent the proven range of tricks for a position
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Bounds {
    pub lower: i8,
    pub upper: i8,
}

impl Bounds {
    pub fn new(lower: i8, upper: i8) -> Self {
        Bounds { lower, upper }
    }

    pub fn is_empty(&self) -> bool {
        self.upper < self.lower
    }

    pub fn intersect(&self, other: Bounds) -> Bounds {
        Bounds {
            lower: self.lower.max(other.lower),
            upper: self.upper.min(other.upper),
        }
    }

    /// Returns true if this bound causes a cutoff at the given beta
    pub fn cutoff(&self, beta: i8) -> bool {
        self.lower >= beta || self.upper < beta
    }
}

/// A Pattern stores hands with relative cards and bounds
/// Patterns form a tree where more specific patterns are children of more general ones
#[derive(Clone)]
pub struct Pattern {
    pub hands: Hands,
    pub bounds: Bounds,
    pub children: Vec<Pattern>,
}

impl Default for Pattern {
    fn default() -> Self {
        Pattern {
            hands: Hands::default(),
            bounds: Bounds::new(0, TOTAL_TRICKS as i8),
            children: Vec::new(),
        }
    }
}

impl Pattern {
    pub fn new(hands: Hands, bounds: Bounds) -> Self {
        Pattern {
            hands,
            bounds,
            children: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.hands = Hands::default();
        self.bounds = Bounds::new(0, TOTAL_TRICKS as i8);
        self.children.clear();
    }

    /// Look up a pattern that matches and causes a cutoff at beta
    pub fn lookup(&self, new_pattern: &Pattern, beta: i8) -> Option<&Pattern> {
        for child in &self.children {
            if !new_pattern.is_subset_of(child) {
                continue;
            }
            if child.bounds.cutoff(beta) {
                return Some(child);
            }
            if let Some(detail) = child.lookup(new_pattern, beta) {
                return Some(detail);
            }
        }
        None
    }

    /// Update the pattern tree with a new pattern
    pub fn update(&mut self, mut new_pattern: Pattern) {
        for i in 0..self.children.len() {
            let child = &mut self.children[i];

            if new_pattern.hands_equal(child) {
                // Same pattern - update bounds
                child.update_bounds(new_pattern.bounds);
                return;
            } else if new_pattern.is_subset_of(child) {
                // New pattern is more specific - add under existing
                new_pattern.bounds = new_pattern.bounds.intersect(child.bounds);
                if !new_pattern.bounds.is_empty() && new_pattern.bounds != child.bounds {
                    child.update(new_pattern);
                }
                return;
            } else if child.is_subset_of(&new_pattern) {
                // New pattern is more general - absorb child
                child.update_bounds(new_pattern.bounds);
                if child.bounds != new_pattern.bounds {
                    new_pattern.children.push(self.children.swap_remove(i));
                } else {
                    // Transfer children
                    let mut old_children = Vec::new();
                    std::mem::swap(&mut old_children, &mut self.children[i].children);
                    new_pattern.children.append(&mut old_children);
                    self.children.swap_remove(i);
                }
                // Check remaining children
                let mut j = i;
                while j < self.children.len() {
                    if self.children[j].is_subset_of(&new_pattern) {
                        self.children[j].update_bounds(new_pattern.bounds);
                        if self.children[j].bounds != new_pattern.bounds {
                            new_pattern.children.push(self.children.swap_remove(j));
                        } else if new_pattern.children.is_empty() {
                            let mut old_children = Vec::new();
                            std::mem::swap(&mut old_children, &mut self.children[j].children);
                            new_pattern.children = old_children;
                            self.children.swap_remove(j);
                        } else {
                            let removed = self.children.swap_remove(j);
                            new_pattern.children.extend(removed.children);
                        }
                    } else {
                        j += 1;
                    }
                }
                self.children.push(new_pattern);
                return;
            }
        }
        // No relationship - add as new child
        self.children.push(new_pattern);
    }

    /// Update bounds and propagate to children
    fn update_bounds(&mut self, new_bounds: Bounds) {
        let old_bounds = self.bounds;
        self.bounds = self.bounds.intersect(new_bounds);
        if self.bounds.is_empty() || self.bounds == old_bounds {
            return;
        }
        let mut i = 0;
        while i < self.children.len() {
            self.children[i].update_bounds(self.bounds);
            if self.children[i].bounds != self.bounds {
                i += 1;
            } else {
                // Child bounds now match parent - flatten
                let removed = self.children.swap_remove(i);
                self.children.extend(removed.children);
            }
        }
    }

    /// Check if this pattern is a subset of (more specific than) another
    /// A pattern is more specific if each hand includes (is subset of) the other
    fn is_subset_of(&self, other: &Pattern) -> bool {
        self.hands[WEST].include(other.hands[WEST])
            && self.hands[NORTH].include(other.hands[NORTH])
            && self.hands[EAST].include(other.hands[EAST])
            && self.hands[SOUTH].include(other.hands[SOUTH])
    }

    /// Check if hands are equal
    fn hands_equal(&self, other: &Pattern) -> bool {
        self.hands[WEST] == other.hands[WEST]
            && self.hands[NORTH] == other.hands[NORTH]
            && self.hands[EAST] == other.hands[EAST]
            && self.hands[SOUTH] == other.hands[SOUTH]
    }

    /// Convert relative rank winners back to actual cards
    pub fn get_rank_winners(&self, all_cards: Cards) -> Cards {
        let relative_rank_winners = self.hands.all_cards();
        let mut rank_winners = Cards::new();
        for suit in 0..NUM_SUITS {
            let rel_suit = relative_rank_winners.suit(suit);
            if rel_suit.is_empty() {
                continue;
            }
            let packed = rel_suit.value() >> (suit * NUM_RANKS);
            let unpacked = unpack_bits(packed, all_cards.suit(suit).value());
            rank_winners = rank_winners.union(Cards::from_bits(unpacked));
        }
        rank_winners
    }
}

/// ShapeEntry is what gets stored in the cache
#[derive(Default)]
pub struct ShapeEntry {
    pub hash: u64,
    pub pattern: Pattern,
}

impl ShapeEntry {
    pub fn reset(&mut self, hash: u64) {
        self.hash = hash;
        self.pattern.reset();
    }

    /// Look up a matching pattern that causes a cutoff.
    ///
    /// Takes `&mut self` because a hit on a child is *promoted* into the root
    /// slot, matching the C++ reference's `ShapeEntry::Lookup`. That promotion
    /// is not merely a most-recently-used speed-up: `Pattern::update` compares
    /// candidates against the root's hands and bounds, so a root left stale
    /// grows a differently-shaped tree, and the generalisations it then stores
    /// are not the reference's. Omitting it is what produced the wrong tables
    /// in #14.
    ///
    /// Returns the matched hands by value; the caller only reads them, and
    /// borrowing would conflict with the promotion.
    pub fn lookup(&mut self, new_pattern: &Pattern, beta: i8) -> Option<(Hands, Bounds)> {
        // Root fast path.
        if self.pattern.bounds.cutoff(beta) && new_pattern.is_subset_of(&self.pattern) {
            return Some((self.pattern.hands, self.pattern.bounds));
        }
        // Otherwise search the children, and promote whatever matched.
        let found = self
            .pattern
            .lookup(new_pattern, beta)
            .map(|matched| (matched.hands, matched.bounds));
        if let Some((hands, bounds)) = found {
            self.pattern.hands = hands;
            self.pattern.bounds = bounds;
        }
        found
    }
}

/// The common bounds cache - hash table of ShapeEntries
pub struct PatternCache {
    entries: Box<[ShapeEntry]>,
    mask: usize,
}

impl PatternCache {
    pub fn new(bits: usize) -> Self {
        let size = 1 << bits;
        let mut entries = Vec::with_capacity(size);
        for _ in 0..size {
            entries.push(ShapeEntry::default());
        }
        PatternCache {
            entries: entries.into_boxed_slice(),
            mask: size - 1,
        }
    }

    /// Hash function matching C++ Cache template
    fn hash(shape: u64, seat_to_play: Seat) -> u64 {
        const HASH_RAND: [u64; 2] = [0x9b8b4567327b23c7, 0x643c986966334873];
        let key0 = shape.wrapping_add(HASH_RAND[0]);
        let key1 = (seat_to_play as u64).wrapping_add(HASH_RAND[1]);
        key0.wrapping_mul(key1)
    }

    #[inline]
    fn index(&self, hash: u64) -> usize {
        // Use top bits for index (like C++)
        (hash >> (64 - (self.mask + 1).trailing_zeros())) as usize & self.mask
    }

    /// Look up a shape entry.
    ///
    /// Mutable because a lookup promotes the matched pattern into the entry's
    /// root slot; see [`ShapeEntry::lookup`].
    pub fn lookup(&mut self, shape: u64, seat_to_play: Seat) -> Option<&mut ShapeEntry> {
        let hash = Self::hash(shape, seat_to_play);
        let idx = self.index(hash);
        let entry = &mut self.entries[idx];
        if entry.hash == hash {
            Some(entry)
        } else {
            None
        }
    }

    /// Get or create a shape entry for update
    pub fn get_or_create(&mut self, shape: u64, seat_to_play: Seat) -> &mut ShapeEntry {
        let hash = Self::hash(shape, seat_to_play);
        let idx = self.index(hash);
        if self.entries[idx].hash != hash {
            self.entries[idx].reset(hash);
        }
        &mut self.entries[idx]
    }
}

/// RelativeHands computation - converts actual cards to relative cards
/// Relative cards are packed so that card ranks are relative to remaining cards
#[derive(Clone, Copy, Default)]
pub struct RelativeHands {
    pub hands: Hands,
}

impl RelativeHands {
    /// Convert a suit to relative cards
    pub fn convert_suit(&mut self, hands: &Hands, suit: Suit, all_suit_cards: Cards) {
        let all_value = all_suit_cards.value();
        for seat in 0..NUM_SEATS {
            let hand_suit = hands[seat].suit(suit);
            let packed = pack_bits(hand_suit.value(), all_value);
            // Clear the suit and add relative cards
            self.hands[seat] = self.hands[seat].clear_suit(suit);
            let relative = Cards::from_bits(packed << (suit * NUM_RANKS));
            self.hands[seat] = self.hands[seat].union(relative);
        }
    }

    /// Compute relative hands for all suits
    pub fn compute(&mut self, hands: &Hands, all_cards: Cards) {
        for suit in 0..NUM_SUITS {
            self.convert_suit(hands, suit, all_cards.suit(suit));
        }
    }

    /// Update relative hands after a trick (only recompute changed suits)
    pub fn update(&mut self, hands: &Hands, prev_all_cards: Cards, new_all_cards: Cards) {
        let changed = prev_all_cards.different(new_all_cards);
        let mut remaining = changed;
        while !remaining.is_empty() {
            let card = remaining.top();
            let suit = suit_of(card);
            remaining = remaining.clear_suit(suit);
            self.convert_suit(hands, suit, new_all_cards.suit(suit));
        }
    }
}

/// Compute pattern hands from relative hands and rank winners
pub fn compute_pattern_hands(
    relative_hands: &Hands,
    all_cards: Cards,
    rank_winners: Cards,
) -> (Hands, Cards) {
    let mut relative_rank_winners = Cards::new();
    let mut extended_rank_winners = Cards::new();

    for suit in 0..NUM_SUITS {
        let rw_suit = rank_winners.suit(suit);
        if rw_suit.is_empty() {
            continue;
        }

        // Find the bottom rank winner in relative terms
        let bottom_winner = rw_suit.bottom();
        let rel_bottom = relative_card_in_suit(bottom_winner, all_cards.suit(suit));

        // Find which seat has this relative card
        let mut actual_rel_bottom = rel_bottom;
        for seat in 0..NUM_SEATS {
            let rel_hand = relative_hands[seat].suit(suit);
            if rel_hand.have(rel_bottom) {
                // Extend to lowest equivalent card
                let suit_value = rel_hand.value() >> (suit * NUM_RANKS);
                let shift = rel_bottom - suit * NUM_RANKS + 1;
                if shift < 64 {
                    let above = suit_value >> shift;
                    actual_rel_bottom += above.trailing_ones() as usize;
                }
                // If it's the suit bottom, extend upward: to its highest
                // equivalent card, and then one rank higher.
                //
                // The reference counts the *run* of consecutive cards held at
                // and below this one, stopping at the first gap. Taking the
                // position of the highest card below it instead — which is
                // what this did — over-subtracts whenever that run is broken,
                // producing a pattern that retains cards the reference drops.
                let all_rel = relative_hands.all_cards().suit(suit);
                if actual_rel_bottom == all_rel.bottom() {
                    let idx = actual_rel_bottom - suit * NUM_RANKS;
                    let run = (!(suit_value << (63 - idx))).leading_zeros() as usize;
                    actual_rel_bottom = actual_rel_bottom.saturating_sub(run);
                }
                break;
            }
        }

        // Build relative rank winners for this suit
        let suit_mask = mask_of(suit);
        let rel_winners = Cards::from_bits(suit_mask).slice(0, actual_rel_bottom + 1);
        relative_rank_winners = relative_rank_winners.union(rel_winners);

        // Unpack to actual cards
        let packed = rel_winners.value() >> (suit * NUM_RANKS);
        let unpacked = unpack_bits(packed, all_cards.suit(suit).value());
        extended_rank_winners = extended_rank_winners.union(Cards::from_bits(unpacked));
    }

    // Pattern hands are relative hands intersected with relative rank winners
    let mut pattern_hands = Hands::default();
    for seat in 0..NUM_SEATS {
        pattern_hands[seat] = relative_hands[seat].intersect(relative_rank_winners);
    }

    (pattern_hands, extended_rank_winners)
}

/// Helper: get relative card index in a suit
fn relative_card_in_suit(card: usize, all_suit_cards: Cards) -> usize {
    let suit = suit_of(card);
    let rank = ACE - all_suit_cards.slice(0, card).size();
    card_of(suit, rank)
}

/// Helper: construct card from suit and rank
fn card_of(suit: Suit, rank: usize) -> usize {
    suit * NUM_RANKS + (ACE - rank)
}

const ACE: usize = 12;

/// A content digest of a cache, for finding where two solvers' caches part
/// company.
///
/// Two rules make the value comparable across independent implementations:
///
///  * **Order-independent across slots.** The digest XORs each live slot's
///    contribution, so table size, hash function and probe order -- all of
///    which differ between us and the C++ reference -- cannot affect it.
///  * **Structure-sensitive within a pattern.** A pattern is a tree and its
///    shape decides what `lookup` will match, so the walk is pre-order and
///    mixes each child's index. Two caches holding the same patterns in a
///    different tree shape are genuinely different caches and must not
///    collide.
///
/// `splitmix64`, so a reimplementation has something exact to match.
#[inline]
pub(crate) fn mix_for_digest(x: u64) -> u64 {
    mix(x)
}

#[inline]
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn pattern_digest(p: &Pattern, depth: u64) -> u64 {
    let mut h = mix(depth);
    for seat in 0..NUM_SEATS {
        h = mix(h ^ mix(p.hands[seat].value()));
    }
    h = mix(h ^ mix(p.bounds.lower as i64 as u64));
    h = mix(h ^ mix(p.bounds.upper as i64 as u64));
    for (i, child) in p.children.iter().enumerate() {
        h = mix(h ^ mix(pattern_digest(child, depth + 1) ^ (i as u64)));
    }
    h
}

impl PatternCache {
    /// Live entries, and a digest of their contents.
    pub fn digest(&self) -> (usize, u64) {
        let mut count = 0;
        let mut digest = 0u64;
        for entry in self.entries.iter() {
            if entry.hash == 0 {
                continue;
            }
            count += 1;
            digest ^= mix(entry.hash) ^ pattern_digest(&entry.pattern, 0);
        }
        (count, digest)
    }
}
