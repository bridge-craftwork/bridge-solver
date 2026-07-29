//! LIN and BBO handviewer-URL input.
//!
//! Turns the two things a user actually has — a LIN string, or the handviewer
//! URL they copied out of BBO — into the [`PlayRequest`](crate::PlayRequest)
//! that [`Analyzer::dd_play`](crate::Analyzer::dd_play) consumes, plus the
//! surrounding detail a UI wants to show (player names, the auction, the
//! claim).
//!
//! Everything here is local: no network, no URL shortener resolution. A
//! `tinyurl.com/...` link has to be expanded by whoever pasted it, because
//! following it would defeat the point of analysing in the browser.

use bridge_encodings::lin::{self, LinData};
use bridge_types::{AnnotatedCall, Auction, Call, Direction, Strain};
use serde::Serialize;

use crate::PlayRequest;

/// A LIN record turned into an analysable request plus its presentation detail.
#[derive(Debug, Serialize)]
pub struct LinInput {
    /// Ready to hand straight to `dd_play` / `dd_play_node`.
    pub request: PlayRequest,
    /// The contract the auction settled on.
    pub contract: ContractInfo,
    /// Seat names in `N, E, S, W` order — reordered from LIN's `S, W, N, E`.
    pub player_names: SeatNames,
    /// Dealer, as `N|E|S|W`.
    pub dealer: String,
    /// Vulnerability, as `None|NS|EW|All`.
    pub vulnerability: String,
    /// The `ah|` board header, e.g. `"Board 6"`.
    pub board: Option<String>,
    /// Tricks claimed via `mc|`, if the hand was claimed rather than played out.
    pub claim: Option<u8>,
    /// How many cards are in `request.plays`. Fewer than 52 is normal: a claim
    /// ends the record early, and the final trick may be partial.
    pub cards_played: usize,
    /// The auction, in calling order from `dealer`.
    pub auction: Vec<AuctionCall>,
}

/// The contract, decomposed for display and for driving the engine.
#[derive(Debug, Serialize)]
pub struct ContractInfo {
    pub level: u8,
    /// `C|D|H|S|N`.
    pub strain: String,
    pub doubled: bool,
    pub redoubled: bool,
    /// `N|E|S|W`.
    pub declarer: String,
    /// Rendered form, e.g. `"5CX"` or `"3N"`.
    pub description: String,
}

/// Names by seat, in the `N, E, S, W` order the rest of the world uses.
#[derive(Debug, Serialize)]
pub struct SeatNames {
    pub north: String,
    pub east: String,
    pub south: String,
    pub west: String,
}

/// One call, with the alert and explanation BBO carries alongside it.
#[derive(Debug, Serialize)]
pub struct AuctionCall {
    /// PBN spelling: `Pass`, `X`, `XX`, `1C`, `3N`, ...
    pub call: String,
    /// The trailing `!` on the LIN token.
    pub alert: bool,
    /// The following `an|` token.
    pub annotation: Option<String>,
}

/// Parse a LIN string or a BBO handviewer URL into an analysable request.
///
/// Accepts either form; a URL is recognised by its `lin=` query parameter.
///
/// # Errors
///
/// Returns a human-readable message if the LIN cannot be parsed, if a call is
/// unrecognisable, or if the auction was passed out — a passed-out board has no
/// contract, so there is nothing to analyse.
pub fn parse(input: &str) -> Result<LinInput, String> {
    let lin = match lin_from_url(input) {
        Some(extracted) => extracted,
        None => input.trim().to_string(),
    };

    let data = lin::parse_lin(&lin).map_err(|e| format!("could not parse the LIN: {}", e))?;
    from_lin_data(&data)
}

/// Parse a multi-board LIN file, one board per line.
///
/// Boards that cannot be analysed — a passed-out auction, a malformed line —
/// are reported as `Err` in place rather than dropped, so the caller can show
/// eleven good boards and one explained failure instead of silently losing one.
///
/// # Errors
///
/// Returns an error only if the file as a whole cannot be read as LIN.
pub fn parse_file(content: &str) -> Result<Vec<Result<LinInput, String>>, String> {
    let boards =
        lin::parse_lin_file(content).map_err(|e| format!("could not parse the LIN file: {}", e))?;
    Ok(boards.iter().map(from_lin_data).collect())
}

/// Build a request from already-parsed LIN.
fn from_lin_data(data: &LinData) -> Result<LinInput, String> {
    let auction = build_auction(data)?;
    let contract = resolve_contract(&auction)
        .ok_or("the auction was passed out, so there is no contract to analyse")?;

    // Anchor the PBN on North regardless of who dealt. The engine's position
    // cache keys on the deal string with only whitespace and case normalised,
    // so the same deal written from a different seat would key differently and
    // re-solve from scratch.
    let dealstr = data.deal.to_pbn(Direction::North);

    let plays = data
        .play
        .iter()
        .map(|c| format!("{}{}", c.suit.to_char(), c.rank.to_char()))
        .collect::<Vec<_>>();

    let request = PlayRequest {
        dealstr,
        trump: contract.strain.to_char().to_string(),
        declarer: contract.declarer.to_char().to_string(),
        // Opening leader is declarer's LHO, and `next` is clockwise.
        leader: contract.declarer.next().to_char().to_string(),
        plays,
    };

    let [south, west, north, east] = &data.player_names;

    Ok(LinInput {
        cards_played: request.plays.len(),
        request,
        contract: ContractInfo {
            level: contract.level,
            strain: contract.strain.to_char().to_string(),
            doubled: contract.doubled,
            redoubled: contract.redoubled,
            declarer: contract.declarer.to_char().to_string(),
            description: format!(
                "{}{}{}",
                contract.level,
                contract.strain.to_char(),
                if contract.redoubled {
                    "XX"
                } else if contract.doubled {
                    "X"
                } else {
                    ""
                }
            ),
        },
        player_names: SeatNames {
            north: north.clone(),
            east: east.clone(),
            south: south.clone(),
            west: west.clone(),
        },
        dealer: data.dealer.to_char().to_string(),
        // `to_pbn` rather than matching the variants: it yields exactly the
        // `None|NS|EW|All` spelling wanted here, and does not break when the
        // variant names change upstream.
        vulnerability: data.vulnerability.to_pbn().to_string(),
        board: data.board_header.clone(),
        claim: data.claim,
        auction: data
            .auction
            .iter()
            .zip(auction.calls.iter())
            .map(|(raw, parsed)| AuctionCall {
                call: parsed.call.to_pbn(),
                alert: raw.alert,
                annotation: raw.annotation.clone(),
            })
            .collect(),
    })
}

/// The contract an auction settled on.
struct ResolvedContract {
    level: u8,
    strain: Strain,
    doubled: bool,
    redoubled: bool,
    declarer: Direction,
}

/// Work out the contract, and in particular *who declares*.
///
/// Deliberately not `bridge_types::Auction::final_contract`, which credits the
/// player who made the last bid. Declarer is the first player of the contract
/// side to have named the final strain — so over `1S - Pass - 4S`, North
/// declares, not the South who bid game. Getting this wrong inverts declarer on
/// most ordinary auctions, which swaps the opening leader and makes every
/// downstream trick count meaningless.
///
/// Returns `None` for a passed-out auction, which has no contract.
fn resolve_contract(auction: &Auction) -> Option<ResolvedContract> {
    /// Index a strain without requiring `Hash`/`Ord` on it upstream.
    fn strain_index(strain: Strain) -> usize {
        match strain {
            Strain::Clubs => 0,
            Strain::Diamonds => 1,
            Strain::Hearts => 2,
            Strain::Spades => 3,
            Strain::NoTrump => 4,
        }
    }
    fn is_ns(seat: Direction) -> bool {
        matches!(seat, Direction::North | Direction::South)
    }

    let mut seat = auction.dealer;
    let mut last_bid: Option<(u8, Strain, Direction)> = None;
    let mut doubled = false;
    let mut redoubled = false;
    // [side][strain] -> the first seat of that side to name that strain.
    let mut first_named: [[Option<Direction>; 5]; 2] = [[None; 5], [None; 5]];

    for annotated in &auction.calls {
        match annotated.call {
            Call::Bid { level, strain } => {
                let side = usize::from(is_ns(seat));
                first_named[side][strain_index(strain)].get_or_insert(seat);
                last_bid = Some((level, strain, seat));
                // A new bid wipes any double standing against the previous one.
                doubled = false;
                redoubled = false;
            }
            Call::Double => doubled = true,
            Call::Redouble => redoubled = true,
            // Pass, and the teaching placeholders, leave the contract alone.
            _ => {}
        }
        seat = seat.next();
    }

    let (level, strain, bidder) = last_bid?;
    let declarer = first_named[usize::from(is_ns(bidder))][strain_index(strain)]?;

    Some(ResolvedContract {
        level,
        strain,
        doubled,
        redoubled,
        declarer,
    })
}

/// Assemble a [`bridge_types::Auction`] from LIN's bid tokens.
///
/// LIN carries bids as loose strings, so this is where they become calls that
/// [`resolve_contract`] can reason about.
fn build_auction(data: &LinData) -> Result<Auction, String> {
    let mut auction = Auction::new(data.dealer);
    for bid in &data.auction {
        let call = parse_lin_call(&bid.bid)
            .ok_or_else(|| format!("unrecognised call \"{}\" in the auction", bid.bid))?;
        auction.calls.push(AnnotatedCall {
            call,
            annotation: bid.annotation.clone(),
        });
    }
    Ok(auction)
}

/// Parse one LIN bid token into a [`Call`].
///
/// BBO writes doubles as `d`/`r`, which `Call::from_pbn` does not accept — it
/// wants PBN's `X`/`XX`. Tools that generate LIN from PBN emit `X`/`XX`
/// instead, so both spellings turn up in real files and both are accepted here.
fn parse_lin_call(bid: &str) -> Option<Call> {
    // A trailing `!` marks an alert; the parser normally strips it, but tokens
    // reaching here by another route may still carry one.
    let token = bid.trim().trim_end_matches('!');
    match token.to_ascii_uppercase().as_str() {
        "P" | "PASS" => Some(Call::Pass),
        "D" | "X" | "DBL" | "DOUBLE" => Some(Call::Double),
        "R" | "XX" | "RDBL" | "REDOUBLE" => Some(Call::Redouble),
        other => Call::from_pbn(other),
    }
}

/// Pull the LIN out of a BBO handviewer URL.
///
/// Returns `None` when the input has no `lin=` parameter, which is how [`parse`]
/// tells a URL from a bare LIN string.
fn lin_from_url(input: &str) -> Option<String> {
    let input = input.trim();
    // Match the parameter, not a bare substring: `lin=` must start the query or
    // follow a separator, so a `...&sourcelin=` would not be mistaken for it.
    let value = ["?lin=", "&lin=", "#lin="]
        .iter()
        .find_map(|p| input.split_once(p).map(|(_, rest)| rest))
        .or_else(|| input.strip_prefix("lin="))?;

    // The parameter ends at the next separator; anything after it is a
    // different parameter (handviewer also uses `&c=` for the trick count).
    let end = value.find(['&', '#']).unwrap_or(value.len());
    Some(percent_decode(&value[..end]))
}

/// Percent-decode a query-string value.
///
/// `+` becomes a space per `application/x-www-form-urlencoded`, which is
/// correct *here* but deliberately not applied to a bare LIN string: LIN uses
/// `+` for spaces in its own annotation and header fields, and the LIN parser
/// already handles those itself.
///
/// Invalid escapes are passed through rather than rejected — a stray `%` in a
/// player's name should not fail the whole board.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                match (hi, lo) {
                    (Some(hi), Some(lo)) => {
                        out.push((hi * 16 + lo) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    // LIN is ASCII, but a player name need not be; salvage what we can.
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Board 6 of a real BBO tournament: a redoubled auction (`mb|d` then
    /// `mb|r`), claimed after 28 cards. Exercises the `d`/`r` spellings, the
    /// three-hand `md|`, and a short play record all at once.
    const REDOUBLED_CLAIMED: &str = "qx|o6|pn|aam135,usvi,kemistry,jelsma|st||md|4SQH5AD28JKAC257JA,S379KH278QKD69C9T,S26AH369TJD5C38QK,|rh||ah|Board 6|sv|e|mb|p|mb|1D|mb|d|mb|r|mb|1S|mb|3C|mb|p|mb|4C|mb|p|mb|4H|an|0 or 3 kc|mb|p|mb|6C|mb|p|mb|p|mb|p|pc|HK|pc|H3|pc|H4|pc|HA|pc|CA|pc|CT|pc|C3|pc|C4|pc|C2|pc|C9|pc|CK|pc|C6|pc|D5|pc|D3|pc|DA|pc|D6|pc|H5|pc|HQ|pc|H6|pc|S4|pc|D9|pc|S2|pc|DQ|pc|DK|pc|DJ|pc|S3|pc|S6|pc|D4|mc|12|zz|10.93|pg||";

    #[test]
    fn parses_a_redoubled_claimed_board() {
        let got = parse(REDOUBLED_CLAIMED).expect("should parse");

        // The `d`/`r` pair applied to 1D, but the auction carried on to 6C, so
        // the final contract is undoubled.
        assert_eq!(got.contract.description, "6C");
        assert_eq!(got.contract.level, 6);
        assert_eq!(got.contract.strain, "C");
        assert!(!got.contract.doubled);
        assert!(!got.contract.redoubled);

        // Dealer is East (`md|4...`), so the calls run E,S,W,N,E,S,... North bid
        // the final 6C, but South named clubs first (3C) — South declares.
        assert_eq!(got.dealer, "E");
        assert_eq!(got.contract.declarer, "S");
        assert_eq!(got.request.declarer, "S");
        assert_eq!(got.request.leader, "W");
        assert_eq!(got.request.trump, "C");

        assert_eq!(got.claim, Some(12));
        assert_eq!(got.cards_played, 28);
        assert_eq!(got.request.plays.len(), 28);
        assert_eq!(got.request.plays[0], "HK");
        assert_eq!(got.request.plays[27], "D4");

        assert_eq!(got.board.as_deref(), Some("Board 6"));
        assert_eq!(got.vulnerability, "EW");

        // LIN lists names S,W,N,E; the output is N,E,S,W.
        assert_eq!(got.player_names.south, "aam135");
        assert_eq!(got.player_names.west, "usvi");
        assert_eq!(got.player_names.north, "kemistry");
        assert_eq!(got.player_names.east, "jelsma");
    }

    #[test]
    fn keeps_the_double_when_it_is_the_last_call() {
        // 5C doubled, claimed at 9 — `mb|d` twice, the second one final.
        let lin = "qx|o9|pn|aam135,ehy,kemistry,~~M32299|st||md|3S238H4JD2568C356A,S57TH56TQKD9TQC9J,S4QAH29DJC2478TQK,|rh||ah|Board 9|sv|e|mb|1C|mb|d|mb|3C|mb|3H|mb|5C|mb|d|mb|p|mb|p|mb|p|pc|DK|pc|D2|pc|D9|pc|DJ|mc|9|";
        let got = parse(lin).expect("should parse");

        assert_eq!(got.contract.description, "5CX");
        assert!(got.contract.doubled);
        assert!(!got.contract.redoubled);
        assert_eq!(got.claim, Some(9));
    }

    #[test]
    fn accepts_pbn_style_x_and_xx_spellings() {
        // Generated LIN uses `X`/`XX` where BBO writes `d`/`r`.
        let bbo = parse_lin_call("d").expect("d is a double");
        let pbn = parse_lin_call("X").expect("X is a double");
        assert_eq!(bbo.to_pbn(), pbn.to_pbn());

        assert_eq!(
            parse_lin_call("r").map(|c| c.to_pbn()).as_deref(),
            Some("XX")
        );
        assert_eq!(
            parse_lin_call("XX").map(|c| c.to_pbn()).as_deref(),
            Some("XX")
        );
        assert_eq!(
            parse_lin_call("p").map(|c| c.to_pbn()).as_deref(),
            Some("Pass")
        );
        assert_eq!(
            parse_lin_call("1N").map(|c| c.to_pbn()).as_deref(),
            Some("1N")
        );
        assert_eq!(
            parse_lin_call("3C!").map(|c| c.to_pbn()).as_deref(),
            Some("3C")
        );
        assert!(parse_lin_call("nonsense").is_none());
    }

    #[test]
    fn extracts_lin_from_a_handviewer_url() {
        let url = "https://www.bridgebase.com/tools/handviewer.html?lin=pn%7CS%2CW%2CN%2CE%7Cmd%7C1SAKHJD876C5432%2C%2C%2C%7Csv%7Co%7Cmb%7C1N%7Cmb%7Cp%7Cmb%7Cp%7Cmb%7Cp%7C";
        let lin = lin_from_url(url).expect("should find the lin parameter");
        assert!(lin.starts_with("pn|S,W,N,E|md|1SAKHJD876C5432,,,|"));

        let got = parse(url).expect("should parse the URL end to end");
        assert_eq!(got.contract.description, "1N");
        assert_eq!(got.dealer, "S");
        assert_eq!(got.player_names.south, "S");
    }

    #[test]
    fn stops_the_lin_parameter_at_the_next_one() {
        // Handviewer appends `&c=` for the trick count; it must not be swallowed.
        let url = "handviewer.html?lin=pn%7CS%2CW%2CN%2CE%7C&c=9";
        assert_eq!(lin_from_url(url).as_deref(), Some("pn|S,W,N,E|"));
    }

    #[test]
    fn does_not_mistake_another_parameter_for_lin() {
        assert_eq!(lin_from_url("https://example.com/?sourcelin=abc"), None);
        // A bare LIN string is not a URL, so `parse` treats it as LIN.
        assert_eq!(lin_from_url("pn|S,W,N,E|md|1SAKHJD876C5432,,,|"), None);
    }

    #[test]
    fn decodes_plus_as_space_only_in_urls() {
        // In a URL, `+` is a space: the board header comes through readable.
        let url = "?lin=pn%7CS%2CW%2CN%2CE%7Cah%7CBoard+46%7C";
        assert_eq!(
            lin_from_url(url).as_deref(),
            Some("pn|S,W,N,E|ah|Board 46|")
        );

        // A bare LIN string keeps its `+`, and the LIN parser resolves it.
        let bare = "pn|S,W,N,E|md|1SAKHJD876C5432,,,|sv|o|ah|Board+46|mb|1N|mb|p|mb|p|mb|p|";
        let got = parse(bare).expect("should parse");
        assert_eq!(got.board.as_deref(), Some("Board 46"));
    }

    #[test]
    fn passes_invalid_escapes_through() {
        assert_eq!(percent_decode("100%25"), "100%");
        assert_eq!(percent_decode("bad%zz"), "bad%zz");
        assert_eq!(percent_decode("trailing%"), "trailing%");
        assert_eq!(percent_decode("short%2"), "short%2");
    }

    #[test]
    fn rejects_a_passed_out_auction() {
        let lin = "pn|S,W,N,E|md|1SAKHJD876C5432,,,|sv|o|mb|p|mb|p|mb|p|mb|p|";
        let err = parse(lin).expect_err("a passed-out board has no contract");
        assert!(err.contains("passed out"), "unexpected message: {}", err);
    }

    /// Declarer is the *first* of the contract side to name the strain, not
    /// whoever bid it last — the case most easily got wrong, and the one
    /// `bridge_types::Auction::final_contract` gets wrong.
    #[test]
    fn attributes_declarer_to_the_first_bidder_of_the_strain() {
        // N opens 1S, S raises to 4S; North declares and East leads.
        let lin = "pn|S,W,N,E|md|3SAKHJD876C5432,S2HQT9DKQ5CKQJT9,SQJT9HA32DAJ2CA8,|sv|o|mb|1S|mb|p|mb|4S|mb|p|mb|p|mb|p|";
        let got = parse(lin).expect("should parse");
        assert_eq!(got.contract.description, "4S");
        assert_eq!(got.contract.declarer, "N");
        assert_eq!(got.request.leader, "E");
    }

    /// Three boards whose declarer and contract were recorded independently, by
    /// the `bridge-bots` toolchain, from the same LIN. Checking against someone
    /// else's answer is the only way to catch a plausible-but-wrong reading of
    /// an auction.
    #[test]
    fn matches_independently_recorded_declarers() {
        // Expected: declarer East, contract 4H. Dealer East (`md|4`): East opens
        // 2H, West raises to 4H — East named hearts first.
        let four_hearts = "pn|Moss,Watson,Bathurst,Feldman|st||md|4S8632H8DA6CK96432,SAK4HKJ62DQJ532C7,SQT95HA93D7CAQJT8,SJ7HQT754DKT984C5|ah|Board 46|sv|o|mb|2H|mb|p|mb|4H|mb|p|mb|p|mb|p|pc|DA|pc|D2|pc|D7|pc|D9|mc|9|";
        let got = parse(four_hearts).expect("should parse");
        assert_eq!(got.contract.description, "4H");
        assert_eq!(got.contract.declarer, "E");
        assert_eq!(got.request.leader, "S");

        // Expected: declarer West, contract 3S (not doubled — the X against 1S
        // was wiped by the 3H that followed). Uses the `X` spelling throughout.
        let three_spades = "pn|Lall,Hampson,Grue,Greco|st||md|3SQ6HQT7DJ98CJ8742,SK9853HA84D43CAT3,SJ7HK6DAKQ765CK96,SAT42HJ9532DT2CQ5|sv|o|mb|1C|mb|p|mb|1D|mb|1S|mb|X|mb|3H|mb|p|mb|3S|mb|p|mb|p|mb|p|pc|DK|pc|D2|pc|D8|pc|D3|mc|8|";
        let got = parse(three_spades).expect("should parse");
        assert_eq!(got.contract.description, "3S");
        assert!(!got.contract.doubled, "the 3H bid cleared the double");
        assert_eq!(got.contract.declarer, "W");
        assert_eq!(got.request.leader, "N");

        // Expected: declarer North, contract 4SX — here the final double stands,
        // because no bid followed it.
        let four_spades_x = "pn|J. Stansby,Greco,Hung,Hampson|st||md|4S8632H8DA6CK96432,SAK4HKJ62DQJ532C7,SQT95HA93D7CAQJT8,SJ7HQT754DKT984C5|ah|Board 46|sv|o|mb|p|mb|p|mb|1D|mb|2C|mb|2H|mb|X|mb|4H|mb|4S|mb|p|mb|p|mb|X|mb|p|mb|p|mb|p|pc|C5|pc|CK|pc|C7|pc|CQ|mc|10|";
        let got = parse(four_spades_x).expect("should parse");
        assert_eq!(got.contract.description, "4SX");
        assert!(got.contract.doubled);
        assert_eq!(got.contract.declarer, "N");
        assert_eq!(got.request.leader, "E");
        assert_eq!(got.claim, Some(10));
    }

    #[test]
    fn a_later_bid_clears_a_standing_double() {
        // 1D doubled, redoubled, then bidding continues to 2N: undoubled.
        let lin = "pn|S,W,N,E|md|3SAKHJD876C5432,S2HQT9DKQ5CKQJT9,SQJT9HA32DAJ2CA8,|sv|o|mb|1D|mb|d|mb|r|mb|1S|mb|2N|mb|p|mb|p|mb|p|";
        let got = parse(lin).expect("should parse");
        assert_eq!(got.contract.description, "2N");
        assert!(!got.contract.doubled);
        assert!(!got.contract.redoubled);
    }

    #[test]
    fn handles_a_board_with_no_cards_played() {
        // Claimed straight out of the auction: legal, and the trace is empty.
        let lin = "pn|vandenb,chesschamp,gurrutia,deepika n|st||md|3S789TQH5KD2C2478T,S2456JAH6TD57TKC6,S3H78JD4689JQC39J,|rh||ah|Board 1|sv|o|mb|p|mb|2C|mb|p|mb|6H|mb|p|mb|p|mb|p|mc|12|";
        let got = parse(lin).expect("should parse");
        assert_eq!(got.cards_played, 0);
        assert!(got.request.plays.is_empty());
        assert_eq!(got.claim, Some(12));
        assert_eq!(got.contract.description, "6H");
    }

    #[test]
    fn reads_lowercase_vugraph_cards() {
        // Vugraph files write ranks and suits in lower case.
        let lin = "pn|S,W,N,E|md|1SAKHJD876C5432,S2HQT9DKQ5CKQJT9,SQJT9HA32DAJ2CA8,|sv|o|mb|1N|mb|p|mb|p|mb|p|pc|d8|pc|dK|pc|dA|pc|d2|";
        let got = parse(lin).expect("should parse");
        assert_eq!(got.request.plays, vec!["D8", "DK", "DA", "D2"]);
    }

    #[test]
    fn anchors_the_deal_string_on_north() {
        // Two boards, same deal, different dealers: the cache key depends on
        // this string, so it must not vary with the dealing seat.
        let east_dealt = "pn|S,W,N,E|md|4SAKHJD876C5432,S2HQT9DKQ5CKQJT9,SQJT9HA32DAJ2CA8,|sv|o|mb|1N|mb|p|mb|p|mb|p|";
        let got = parse(east_dealt).expect("should parse");
        assert!(
            got.request.dealstr.starts_with("N:"),
            "dealstr should be North-anchored, got {}",
            got.request.dealstr
        );
        assert_eq!(got.dealer, "E");
    }

    /// End to end: a parsed LIN record must actually drive the engine.
    ///
    /// Every other test here checks the parse in isolation, which cannot catch a
    /// request that is well-formed but that `running_trace` rejects — and that
    /// is exactly the failure a caller sees as "no analysis" with no reason
    /// given.
    #[test]
    fn a_parsed_board_feeds_the_engine() {
        use bridge_solver::analyse_play::{self, PlayInput};
        use bridge_solver::Hands;
        use bridge_types::Deal;
        use std::collections::HashMap;

        let got = parse(REDOUBLED_CLAIMED).expect("should parse");
        let req = &got.request;

        let deal = Deal::from_pbn(&req.dealstr).expect("the deal string should parse back");
        let input = PlayInput {
            hands: Hands::from_deal(&deal),
            trump: analyse_play::parse_trump(&req.trump).expect("trump"),
            declarer: analyse_play::parse_seat(&req.declarer).expect("declarer"),
            leader: analyse_play::parse_seat(&req.leader).expect("leader"),
            plays: req
                .plays
                .iter()
                .map(|p| analyse_play::parse_card(p).expect("card"))
                .collect(),
        };

        let keys = analyse_play::prefix_keys(&req.dealstr, input.trump, input.leader, &input.plays);
        let output = analyse_play::running_trace(&input, &keys, &HashMap::new())
            .expect("the engine should accept a request built from real LIN");

        assert_eq!(output.trace.len(), 28, "one trace entry per card played");
        // South declares 6C and the double-dummy table gives South 12 tricks in
        // clubs, which is what was claimed.
        assert_eq!(output.contract_tricks, 12);
    }

    /// Checked against Bridge Base's own BSOL analysis of a real board.
    ///
    /// The reference is a bridgewebs BSOL payload carrying both the
    /// double-dummy table and a per-player error count, for a hand claimed after
    /// 41 cards. It pins three separate things at once: the table, the trace
    /// length, and the error attribution.
    ///
    /// It is also the auction that matters most. `1NT - Pass - 2C - Pass -
    /// 2H - Pass - 3NT`: East bid the final 3NT, but West named notrump first,
    /// so **West declares**. `Auction::final_contract` would say East, put the
    /// opening lead in the wrong hand, and produce a confidently wrong answer —
    /// which is why [`resolve_contract`] exists.
    #[test]
    fn matches_bsol_on_a_real_board() {
        use bridge_solver::analyse_play::{self, PlayInput};
        use bridge_solver::Hands;
        use bridge_types::{Deal, Direction, Strain};
        use std::collections::{BTreeMap, HashMap};

        // BSOL's `Deal`, which is in N,E,S,W order — West holds 16 HCP and
        // opened 1NT, which is what identifies the ordering.
        let dealstr = "N:J98.QT83.K6.J853 Q762.J4.QJT5.AT6 KT43.652.A984.94 A5.AK97.732.KQ72";
        let plays = [
            "C3", "CT", "C4", "C2", "DQ", "D4", "D2", "DK", "SJ", "S2", "S3", "SA", "D3", "D6",
            "DJ", "DA", "C9", "C7", "C5", "CA", "DT", "D8", "D7", "S8", "HJ", "H6", "H7", "HQ",
            "S9", "SQ", "SK", "S5", "ST", "H9", "C8", "S6", "D9", "CQ", "CJ", "D5", "H5",
        ];
        assert_eq!(plays.len(), 41, "a claim ended this board early");

        let deal = Deal::from_pbn(dealstr).expect("deal should parse");

        // 1. The table, encoded the way BSOL sends it: seat-major over N,S,E,W
        //    with strains NT,S,H,D,C. Both orders differ from this engine's, so
        //    getting the string out is itself part of the check.
        let table = bridge_solver::par::solve_dd_table(&deal);
        let mut ddtricks = String::new();
        for seat in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            for strain in [
                Strain::NoTrump,
                Strain::Spades,
                Strain::Hearts,
                Strain::Diamonds,
                Strain::Clubs,
            ] {
                let n = table.get(seat, strain);
                ddtricks.push(if n < 10 {
                    (b'0' + n) as char
                } else {
                    (b'a' + n - 10) as char
                });
            }
        }
        assert_eq!(ddtricks, "45544465449789987899");

        // 2. The trace.
        let input = PlayInput {
            hands: Hands::from_deal(&deal),
            trump: analyse_play::parse_trump("NT").expect("NT"),
            declarer: analyse_play::parse_seat("W").expect("W declares"),
            leader: analyse_play::parse_seat("N").expect("North leads"),
            plays: plays
                .iter()
                .map(|p| analyse_play::parse_card(p).expect("card"))
                .collect(),
        };
        let keys = analyse_play::prefix_keys(dealstr, input.trump, input.leader, &input.plays);
        let output = analyse_play::running_trace(&input, &keys, &HashMap::new())
            .expect("trace should solve");

        // West takes 8 in notrump but bid 3NT, so double-dummy is already one
        // short — and the table above agrees, giving W's NT cell as 8. They
        // claimed 7, one fewer still.
        assert_eq!(output.contract_tricks, 8);

        let costed: Vec<_> = output.trace.iter().filter(|e| e.cost > 0).collect();
        assert_eq!(
            costed.len(),
            5,
            "BSOL reports 5 costed errors on this board"
        );

        let mut by_seat: BTreeMap<&str, u32> = BTreeMap::new();
        for e in &costed {
            *by_seat.entry(e.seat.as_str()).or_default() += e.cost as u32;
        }

        // 3. Attribution. This engine credits a card to the seat that holds it;
        //    BSOL credits dummy's cards to declarer, who actually chooses them,
        //    and scores dummy itself as not applicable. West declares, so East is
        //    dummy — and folding East into West reproduces BSOL's per-player
        //    counts exactly: North 1, South 1, West 3.
        assert_eq!(by_seat.get("N").copied().unwrap_or(0), 1);
        assert_eq!(by_seat.get("S").copied().unwrap_or(0), 1);
        assert_eq!(by_seat.get("W").copied().unwrap_or(0), 1);
        assert_eq!(by_seat.get("E").copied().unwrap_or(0), 2);

        let declarer_side = by_seat.get("W").copied().unwrap_or(0) + by_seat["E"];
        assert_eq!(declarer_side, 3, "BSOL scores the declaring side as 3");
    }

    #[test]
    fn parses_a_multi_board_file() {
        let content = format!(
            "{}\n{}\n",
            REDOUBLED_CLAIMED, "pn|S,W,N,E|md|1SAKHJD876C5432,,,|sv|o|mb|p|mb|p|mb|p|mb|p|"
        );
        let boards = parse_file(&content).expect("the file should read");
        assert_eq!(boards.len(), 2);

        let first = boards[0].as_ref().expect("board 1 analysable");
        assert_eq!(first.contract.description, "6C");

        // The passed-out board is reported in place, not dropped.
        assert!(boards[1].is_err());
    }
}
