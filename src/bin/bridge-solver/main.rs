//! bridge-solver - Double-dummy solver for PBN files
//!
//! Reads a PBN file containing bridge deals, performs double-dummy analysis,
//! and writes the results as Bridge Composer compatible tags:
//! - DoubleDummyTricks (compact encoding)
//! - OptimumScore (if vulnerability is known)
//! - ParContract (if vulnerability is known)
//! - OptimumResultTable (full table)
//!
//! Boards whose deal is incomplete are passed through untouched.
//!
//! Usage:
//!   bridge-solver -i <file.pbn> -o <file.pbn>   # one file to another
//!   bridge-solver -i <file.pbn>                 # one file to stdout
//!   bridge-solver -w -i <file.pbn> <dir> ...    # annotate in place, recursively

use bridge_solver::{
    par, CutoffCache, DdTricks, Hands, PatternCache, Solver, CLUB, DIAMOND, EAST, HEART, NORTH,
    NOTRUMP, SOUTH, SPADE, WEST,
};
use clap::Parser;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "bridge-solver")]
#[command(about = "Double-dummy solver for PBN files")]
#[command(version)]
struct Args {
    /// Input PBN file(s) or director(ies); directories are searched recursively
    /// for *.pbn. Accepts several, which requires --in-place.
    #[arg(short = 'i', long = "input", required = true, num_args = 1..)]
    input: Vec<String>,

    /// Output PBN file (if not specified, writes to stdout)
    #[arg(short = 'o', long = "output", conflicts_with = "in_place")]
    output: Option<String>,

    /// Rewrite each input file in place. Files whose content is unchanged are
    /// left alone, so a re-run touches nothing and build systems see no churn.
    #[arg(short = 'w', long = "in-place")]
    in_place: bool,

    /// Recompute analysis for boards that already carry it. By default a board
    /// with a [DoubleDummyTricks] tag is left exactly as found, so annotating a
    /// collection only fills in what is missing.
    #[arg(long = "recalculate")]
    recalculate: bool,

    /// Verbose output - show progress
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

/// Vulnerability state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vulnerability {
    None,
    NS,
    EW,
    All,
}

/// Double-dummy results for all 20 combinations
#[derive(Debug, Clone)]
struct DdResults {
    /// results[declarer][denomination] = tricks (declarer: 0=N,1=S,2=E,3=W; denom: 0=NT,1=S,2=H,3=D,4=C)
    tricks: [[u8; 5]; 4],
}

impl DdResults {
    /// Encode as DoubleDummyTricks string (20 hex-like chars)
    /// Format: N(NT,S,H,D,C) + S(NT,S,H,D,C) + E(NT,S,H,D,C) + W(NT,S,H,D,C)
    fn encode_ddt(&self) -> String {
        let mut s = String::with_capacity(20);
        for decl in 0..4 {
            for denom in 0..5 {
                let tricks = self.tricks[decl][denom];
                let ch = if tricks <= 9 {
                    (b'0' + tricks) as char
                } else {
                    (b'a' + (tricks - 10)) as char
                };
                s.push(ch);
            }
        }
        s
    }

    /// Get tricks for a specific declarer and denomination
    fn get(&self, declarer: usize, denom: usize) -> u8 {
        self.tricks[declarer][denom]
    }
}

fn main() {
    let args = Args::parse();

    let files = match collect_inputs(&args.input) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };
    if files.is_empty() {
        eprintln!("Error: no .pbn files found in the given input(s)");
        std::process::exit(1);
    }
    if files.len() > 1 && !args.in_place {
        eprintln!(
            "Error: {} input files matched; use --in-place to annotate them, \
             or name a single file with --output",
            files.len()
        );
        std::process::exit(1);
    }

    let mut changed = 0usize;
    for path in &files {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error reading input file '{}': {}", path.display(), e);
                std::process::exit(1);
            }
        };
        if args.verbose {
            eprintln!("Processing {}...", path.display());
        }
        let result = process_pbn(&content, args.verbose, args.recalculate);

        if args.in_place {
            // Unchanged files are left untouched so a re-run is a true no-op
            // and does not churn mtimes in a build.
            if result == content {
                continue;
            }
            if let Err(e) = write_atomically(path, &result) {
                eprintln!("Error writing '{}': {}", path.display(), e);
                std::process::exit(1);
            }
            changed += 1;
        } else {
            match args.output {
                Some(ref out) => {
                    if let Err(e) = fs::write(out, &result) {
                        eprintln!("Error writing output file '{out}': {e}");
                        std::process::exit(1);
                    }
                    if args.verbose {
                        eprintln!("Output written to {out}");
                    }
                }
                None => {
                    io::stdout().write_all(result.as_bytes()).unwrap();
                }
            }
        }
    }

    if args.in_place && args.verbose {
        eprintln!("{changed} of {} file(s) updated", files.len());
    }
}

/// Expand the input arguments into a sorted, de-duplicated list of PBN files.
/// A directory contributes every `*.pbn` beneath it; a file is taken as given,
/// whatever its extension, so an oddly-named file can still be named directly.
fn collect_inputs(inputs: &[String]) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for raw in inputs {
        let path = PathBuf::from(raw);
        if path.is_dir() {
            collect_pbn_files(&path, &mut files)?;
        } else if path.exists() {
            files.push(path);
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no such file or directory: {raw}"),
            ));
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Recursively gather `*.pbn` under `dir`.
fn collect_pbn_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_pbn_files(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pbn"))
        {
            out.push(path);
        }
    }
    Ok(())
}

/// Write via a sibling temporary file and rename, so an interrupted run cannot
/// leave a half-written lesson file behind.
fn write_atomically(path: &Path, contents: &str) -> io::Result<()> {
    let tmp = path.with_extension("pbn.tmp");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)
}

/// Process a PBN file: find deals, solve them, insert/replace DD tags
fn process_pbn(content: &str, verbose: bool, recalculate: bool) -> String {
    // Split into deal blocks (separated by blank lines outside of brace comments)
    let mut result = String::new();
    let mut deal_count = 0;

    // Process the file block by block
    // A block is a sequence of lines until a blank line outside of {} comments
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        // Skip leading blank lines, but preserve them
        while i < lines.len() && lines[i].trim().is_empty() {
            result.push_str(lines[i]);
            result.push('\n');
            i += 1;
        }

        if i >= lines.len() {
            break;
        }

        // Collect a deal block (all lines until next blank line outside of {} comments)
        let block_start = i;
        let mut in_brace_comment = false;

        while i < lines.len() {
            let line = lines[i];

            // Track brace comment state
            // Note: braces don't nest per PBN spec
            for ch in line.chars() {
                if ch == '{' {
                    in_brace_comment = true;
                } else if ch == '}' {
                    in_brace_comment = false;
                }
            }

            i += 1;

            // Check if next line would be a blank line outside of comment
            if i < lines.len() && lines[i].trim().is_empty() && !in_brace_comment {
                break;
            }
        }
        let block_end = i;

        // Process this block
        let block_lines = &lines[block_start..block_end];
        let processed = process_deal_block(block_lines, &mut deal_count, verbose, recalculate);
        result.push_str(&processed);
    }

    if verbose {
        eprintln!("Processed {} deal(s)", deal_count);
    }

    result
}

/// Process a single deal block
fn process_deal_block(
    lines: &[&str],
    deal_count: &mut usize,
    verbose: bool,
    recalculate: bool,
) -> String {
    // Find the Deal tag to extract hands
    let mut deal_str: Option<&str> = None;
    let mut vulnerability: Option<Vulnerability> = None;

    for line in lines {
        if deal_str.is_none() {
            if let Some(d) = extract_deal_tag(line) {
                deal_str = Some(d);
            }
        }
        if vulnerability.is_none() {
            if let Some(v) = extract_vulnerability_tag(line) {
                vulnerability = Some(v);
            }
        }
    }

    // If no Deal tag, just pass through unchanged
    let Some(deal_str) = deal_str else {
        let mut out = String::new();
        for line in lines {
            out.push_str(line);
            out.push('\n');
        }
        return out;
    };

    // Parse the deal. A board with no cards — BridgeComposer writes
    // [Deal "N:... ... ... ..."] for auction-only teaching boards — parses fine
    // into empty hands, so completeness is checked too. Annotating one of those
    // would stamp a fabricated all-zero table and a "Pass" par onto a board that
    // has no deal to analyze.
    let unchanged = || {
        let mut out = String::new();
        for line in lines {
            out.push_str(line);
            out.push('\n');
        }
        out
    };
    // Already analyzed? Leave it alone unless asked to redo the work. The
    // [DoubleDummyTricks] tag is the marker: a board carrying a stray par tag
    // but no DD table has not been analyzed, and still gets filled in.
    if !recalculate
        && lines
            .iter()
            .any(|l| extract_tag_name(l) == Some("DoubleDummyTricks"))
    {
        if verbose {
            eprintln!("Skipping board that already has analysis");
        }
        return unchanged();
    }

    let Some(hands) = Hands::from_pbn(deal_str) else {
        return unchanged();
    };
    if !hands.is_complete() {
        if verbose {
            eprintln!("Skipping board with an incomplete deal");
        }
        return unchanged();
    }

    *deal_count += 1;
    if verbose {
        eprintln!("Processing deal {}...", deal_count);
    }

    // Solve the deal
    let dd_results = solve_deal(&hands);

    // Generate the DD tags
    let dd_tags = generate_dd_tags(&dd_results, vulnerability);

    // Now reconstruct the block:
    // 1. Remove any existing DD tags
    // 2. Insert our new DD tags in the right place

    let mut output_lines: Vec<String> = Vec::new();
    let mut found_dd_tag = false;
    let mut skipping_optimum_data = false;
    let mut insertion_point: Option<usize> = None;

    // Tags we generate (need to remove existing ones)
    let dd_tag_names = [
        "DoubleDummyTricks",
        "OptimumScore",
        "ParContract",
        "OptimumResultTable",
    ];

    for line in lines {
        let trimmed = line.trim();

        // Check if this is one of our DD tags
        if let Some(tag_name) = extract_tag_name(trimmed) {
            if dd_tag_names.contains(&tag_name) {
                if !found_dd_tag {
                    // Remember where to insert (we'll insert our new tags here)
                    insertion_point = Some(output_lines.len());
                    found_dd_tag = true;
                }
                if tag_name == "OptimumResultTable" {
                    skipping_optimum_data = true;
                }
                continue;
            }
        }

        // Skip data lines that follow OptimumResultTable
        if skipping_optimum_data {
            if is_optimum_result_data_line(line) {
                continue;
            } else {
                // Stop skipping when we hit a non-data line
                skipping_optimum_data = false;
            }
        }

        output_lines.push(line.to_string());

        // Track potential insertion points (after Result tag, or alphabetically among supplemental tags)
        if !found_dd_tag {
            if trimmed.starts_with("[Result ") {
                // Insert after Result tag (last mandatory tag)
                insertion_point = Some(output_lines.len());
            } else if trimmed.starts_with('[') {
                if let Some(tag_name) = extract_tag_name(trimmed) {
                    // DoubleDummyTricks comes first alphabetically among our tags
                    if tag_name > "DoubleDummyTricks" && insertion_point.is_none() {
                        // Insert before this tag
                        insertion_point = Some(output_lines.len() - 1);
                    } else if tag_name < "DoubleDummyTricks" {
                        // Insert after this tag
                        insertion_point = Some(output_lines.len());
                    }
                }
            }
        }
    }

    // Build the output
    let mut result = String::new();
    let insert_at = insertion_point.unwrap_or(output_lines.len());

    for (idx, line) in output_lines.iter().enumerate() {
        if idx == insert_at {
            result.push_str(&dd_tags);
        }
        result.push_str(line);
        result.push('\n');
    }

    // If insertion point was at the end
    if insert_at >= output_lines.len() {
        result.push_str(&dd_tags);
    }

    result
}

/// Extract the deal string from a [Deal "..."] tag
fn extract_deal_tag(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if !trimmed.starts_with("[Deal ") {
        return None;
    }

    // Find the quoted content
    let start = trimmed.find('"')? + 1;
    let end = trimmed.rfind('"')?;
    if end <= start {
        return None;
    }

    Some(&trimmed[start..end])
}

/// Extract vulnerability from [Vulnerable "..."] tag
fn extract_vulnerability_tag(line: &str) -> Option<Vulnerability> {
    let trimmed = line.trim();
    if !trimmed.starts_with("[Vulnerable ") {
        return None;
    }

    let start = trimmed.find('"')? + 1;
    let end = trimmed.rfind('"')?;
    if end <= start {
        return None;
    }

    let value = &trimmed[start..end];
    match value.to_uppercase().as_str() {
        "NONE" | "LOVE" | "-" => Some(Vulnerability::None),
        "NS" | "N" => Some(Vulnerability::NS),
        "EW" | "E" => Some(Vulnerability::EW),
        "ALL" | "BOTH" => Some(Vulnerability::All),
        _ => None,
    }
}

/// Extract the tag name from a tag line like "[TagName ...]"
fn extract_tag_name(line: &str) -> Option<&str> {
    if !line.starts_with('[') {
        return None;
    }
    let rest = &line[1..];
    let end = rest.find([' ', ']'])?;
    Some(&rest[..end])
}

/// Check if a line is OptimumResultTable data (e.g., "N NT  3")
fn is_optimum_result_data_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Must start with a seat letter
    let first_char = trimmed.chars().next().unwrap_or(' ');
    if !['N', 'S', 'E', 'W'].contains(&first_char) {
        return false;
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() != 3 {
        return false;
    }

    // First part should be a seat (N/S/E/W)
    let seat = parts[0];
    if !["N", "S", "E", "W"].contains(&seat) {
        return false;
    }

    // Second part should be a denomination (NT/S/H/D/C)
    let denom = parts[1];
    if !["NT", "S", "H", "D", "C"].contains(&denom) {
        return false;
    }

    // Third part should be a number
    parts[2].parse::<u8>().is_ok()
}

/// Solve a deal and return DD results
fn solve_deal(hands: &Hands) -> DdResults {
    // Solve for each declarer (N, S, E, W) and denomination (NT, S, H, D, C)
    let declarers = [NORTH, SOUTH, EAST, WEST];
    let denominations = [NOTRUMP, SPADE, HEART, DIAMOND, CLUB];

    // Store results: [declarer][denomination] = tricks
    let mut results = [[0u8; 5]; 4];

    // Solve for each denomination (caches are per-trump, shared across leaders)
    for (denom_idx, trump) in denominations.iter().enumerate() {
        // Create fresh caches for each trump contract
        let mut cutoff_cache = CutoffCache::new(16);
        let mut pattern_cache = PatternCache::new(16);

        for (decl_idx, declarer_seat) in declarers.iter().enumerate() {
            // The leader is to the left of declarer
            let leader = (*declarer_seat + 1) % 4;

            let solver = Solver::new(*hands, *trump, leader);
            let ns_tricks = solver.solve_with_caches(&mut cutoff_cache, &mut pattern_cache);

            // Convert to declarer's tricks
            let declarer_tricks = if *declarer_seat == NORTH || *declarer_seat == SOUTH {
                ns_tricks
            } else {
                hands.num_tricks() as u8 - ns_tricks
            };

            results[decl_idx][denom_idx] = declarer_tricks;
        }
    }

    DdResults { tricks: results }
}

/// Generate all DD tags as a string
fn generate_dd_tags(results: &DdResults, vulnerability: Option<Vulnerability>) -> String {
    let mut output = String::new();

    // 1. DoubleDummyTricks
    output.push_str(&format!(
        "[DoubleDummyTricks \"{}\"]\n",
        results.encode_ddt()
    ));

    // 2. Par: OptimumScore + ParContract (needs vulnerability to score).
    if let Some(vul) = vulnerability {
        let (vul_ns, vul_ew) = match vul {
            Vulnerability::None => (false, false),
            Vulnerability::NS => (true, false),
            Vulnerability::EW => (false, true),
            Vulnerability::All => (true, true),
        };
        let p = par(&to_par_table(results), vul_ns, vul_ew);
        output.push_str(&format!("[OptimumScore \"{}\"]\n", p.optimum_score()));
        if let Some(c) = p.contract {
            output.push_str(&format!("[ParContract \"{}\"]\n", c.describe()));
        }
    }

    // 3. OptimumResultTable
    output.push_str("[OptimumResultTable \"Declarer;Denomination\\2R;Result\\2R\"]\n");

    let decl_names = ["N", "S", "E", "W"];
    let denom_names = ["NT", " S", " H", " D", " C"];

    for (decl_idx, decl_name) in decl_names.iter().enumerate() {
        for (denom_idx, denom_name) in denom_names.iter().enumerate() {
            output.push_str(&format!(
                "{} {} {:2}\n",
                decl_name,
                denom_name,
                results.get(decl_idx, denom_idx)
            ));
        }
    }

    output
}

/// Convert this bin's `DdResults` (declarer N,S,E,W × denom NT,S,H,D,C) into the
/// library `DdTricks` (seat N,E,S,W × strain C,D,H,S,NT) expected by `par`.
fn to_par_table(results: &DdResults) -> DdTricks {
    const DECL_TO_DIR: [usize; 4] = [0, 2, 1, 3]; // N,S,E,W -> N,S,E,W indices
    const DENOM_TO_STRAIN: [usize; 5] = [4, 3, 2, 1, 0]; // NT,S,H,D,C -> C..NT
    let mut tricks = [[0u8; 5]; 4];
    for d in 0..4 {
        for n in 0..5 {
            tricks[DECL_TO_DIR[d]][DENOM_TO_STRAIN[n]] = results.get(d, n);
        }
    }
    DdTricks { tricks }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_deal_tag() {
        let line = r#"[Deal "N:AK.QJ.T9.8765 432.A.K.QJT94 QJT.KT9.QJ.AK3 9876.8765.A8765.2"]"#;
        let deal = extract_deal_tag(line).unwrap();
        assert!(deal.starts_with("N:"));
    }

    #[test]
    fn test_extract_deal_tag_no_match() {
        assert!(extract_deal_tag("[Event \"Test\"]").is_none());
        assert!(extract_deal_tag("N NT 3").is_none());
    }

    #[test]
    fn test_extract_vulnerability() {
        assert_eq!(
            extract_vulnerability_tag("[Vulnerable \"None\"]"),
            Some(Vulnerability::None)
        );
        assert_eq!(
            extract_vulnerability_tag("[Vulnerable \"NS\"]"),
            Some(Vulnerability::NS)
        );
        assert_eq!(
            extract_vulnerability_tag("[Vulnerable \"EW\"]"),
            Some(Vulnerability::EW)
        );
        assert_eq!(
            extract_vulnerability_tag("[Vulnerable \"All\"]"),
            Some(Vulnerability::All)
        );
        assert_eq!(
            extract_vulnerability_tag("[Vulnerable \"Both\"]"),
            Some(Vulnerability::All)
        );
    }

    #[test]
    fn test_extract_tag_name() {
        assert_eq!(extract_tag_name("[Event \"Test\"]"), Some("Event"));
        assert_eq!(
            extract_tag_name("[OptimumResultTable \"...\"]"),
            Some("OptimumResultTable")
        );
        assert_eq!(extract_tag_name("[Deal \"N:...\"]"), Some("Deal"));
        assert_eq!(extract_tag_name("N NT 3"), None);
    }

    #[test]
    fn test_is_optimum_result_data_line() {
        assert!(is_optimum_result_data_line("N NT  3"));
        assert!(is_optimum_result_data_line("S  S 10"));
        assert!(is_optimum_result_data_line("E  H  7"));
        assert!(!is_optimum_result_data_line("[Deal \"...\"]"));
        assert!(!is_optimum_result_data_line(""));
        assert!(!is_optimum_result_data_line("[OptimumResultTable \"...\"]"));
    }

    #[test]
    fn test_encode_ddt() {
        // Test the encoding: 0-9 -> '0'-'9', 10-13 -> 'a'-'d'
        // From Bridge Composer: "32691326914a74a4a74a"
        // Format: N(NT,S,H,D,C) S(NT,S,H,D,C) E(NT,S,H,D,C) W(NT,S,H,D,C)
        let results = DdResults {
            tricks: [
                [3, 2, 6, 9, 1],   // N: NT=3, S=2, H=6, D=9, C=1 -> "32691"
                [3, 2, 6, 9, 1],   // S: same -> "32691"
                [4, 10, 7, 4, 10], // E: NT=4, S=10, H=7, D=4, C=10 -> "4a74a"
                [4, 10, 7, 4, 10], // W: same -> "4a74a"
            ],
        };
        assert_eq!(results.encode_ddt(), "32691326914a74a4a74a");
    }

    #[test]
    fn test_process_simple_pbn() {
        // Use a real 52-card deal from Bridge Composer reference
        let pbn = r#"[Event "Test"]
[Vulnerable "None"]
[Deal "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72"]
[Dealer "N"]
"#;
        let result = process_pbn(pbn, false, false);
        assert!(result.contains("[DoubleDummyTricks"));
        assert!(result.contains("[OptimumResultTable"));
        assert!(result.contains("N NT"));
    }

    #[test]
    fn test_replaces_existing_dd_tags() {
        let pbn = r#"[Event "Test"]
[Vulnerable "None"]
[Deal "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72"]
[DoubleDummyTricks "00000000000000000000"]
[OptimumScore "NS 0"]
[ParContract "NS Pass"]
[OptimumResultTable "Declarer;Denomination\2R;Result\2R"]
N NT  0
N  S  0
N  H  0
N  D  0
N  C  0
S NT  0
S  S  0
S  H  0
S  D  0
S  C  0
E NT  0
E  S  0
E  H  0
E  D  0
E  C  0
W NT  0
W  S  0
W  H  0
W  D  0
W  C  0
[Dealer "N"]
"#;
        let result = process_pbn(pbn, false, true);
        // Each tag we generate must appear exactly once: the stale copy is
        // stripped and replaced, not duplicated. `Vulnerable` is present, so
        // the par tags are generated too.
        assert_eq!(result.matches("[DoubleDummyTricks").count(), 1);
        assert_eq!(result.matches("[OptimumResultTable").count(), 1);
        assert_eq!(result.matches("[OptimumScore").count(), 1);
        assert_eq!(result.matches("[ParContract").count(), 1);
        // ...and carry recomputed values, not the placeholders from the input.
        assert!(!result.contains("\"00000000000000000000\""));
        assert!(result.contains(r#"[DoubleDummyTricks "9a8789a8784346543465"]"#));
        assert!(result.contains(r#"[OptimumScore "NS 420"]"#));
        assert!(result.contains(r#"[ParContract "NS 4S="]"#));
    }

    /// The default is to fill in only what is missing: a board that already
    /// carries a DD table is passed through byte-for-byte, however stale its
    /// values, so a builder can point the tool at a whole collection safely.
    #[test]
    fn test_existing_analysis_is_kept_unless_recalculating() {
        let pbn = r#"[Event "Test"]
[Vulnerable "None"]
[Deal "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72"]
[DoubleDummyTricks "00000000000000000000"]
[Dealer "N"]
"#;
        let kept = process_pbn(pbn, false, false);
        assert_eq!(kept, pbn, "default must not touch an analyzed board");

        let redone = process_pbn(pbn, false, true);
        assert!(redone.contains(r#"[DoubleDummyTricks "9a8789a8784346543465"]"#));
    }

    /// A board holding a par tag but no DD table has not been analyzed, so the
    /// default still fills it in — and replaces the orphaned par value.
    #[test]
    fn test_par_tag_alone_does_not_count_as_analyzed() {
        let pbn = r#"[Event "Test"]
[Vulnerable "None"]
[Deal "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72"]
[OptimumScore "NS 9999"]
[Dealer "N"]
"#;
        let result = process_pbn(pbn, false, false);
        assert_eq!(result.matches("[DoubleDummyTricks").count(), 1);
        assert!(!result.contains("NS 9999"));
    }

    /// A board with no cards must be left exactly as found. BridgeComposer
    /// writes `[Deal "N:... ... ... ..."]` for auction-only teaching boards;
    /// those parse into empty hands, and annotating one would stamp a
    /// fabricated all-zero table and a "Pass" par onto a board with no deal.
    #[test]
    fn test_placeholder_deals_pass_through_untouched() {
        let pbn = r#"[Event "Test"]
[Board "1"]
[Vulnerable "None"]
[Deal "N:... ... ... ..."]
[Auction "N"]
1S Pass 2S AP
"#;
        let result = process_pbn(pbn, false, false);
        assert_eq!(result, pbn, "placeholder board must be byte-identical");
        assert!(!result.contains("DoubleDummyTricks"));
        assert!(!result.contains("OptimumScore"));
    }

    /// A file mixing real and placeholder boards annotates only the real ones.
    #[test]
    fn test_annotates_only_complete_deals_in_mixed_file() {
        let pbn = r#"[Event "Test"]
[Board "1"]
[Vulnerable "None"]
[Deal "N:... ... ... ..."]

[Event "Test"]
[Board "2"]
[Vulnerable "None"]
[Deal "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72"]
"#;
        let result = process_pbn(pbn, false, false);
        assert_eq!(result.matches("[DoubleDummyTricks").count(), 1);
        assert!(!result.contains("\"00000000000000000000\""));
    }

    /// Without a `Vulnerable` tag par cannot be scored, so the par tags are
    /// omitted — and a stale copy in the input is still stripped.
    #[test]
    fn test_strips_par_tags_when_vulnerability_unknown() {
        let pbn = r#"[Event "Test"]
[Deal "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72"]
[OptimumScore "NS 0"]
[ParContract "NS Pass"]
[Dealer "N"]
"#;
        let result = process_pbn(pbn, false, false);
        assert_eq!(result.matches("[DoubleDummyTricks").count(), 1);
        assert_eq!(result.matches("[OptimumScore").count(), 0);
        assert_eq!(result.matches("[ParContract").count(), 0);
    }
}
