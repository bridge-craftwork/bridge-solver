//! bridge-solver - Double-dummy solver for PBN files
//!
//! Reads a PBN file containing bridge deals, performs double-dummy analysis,
//! and writes the results as Bridge Composer compatible tags, where Bridge
//! Composer itself puts them (see `fixtures/bridge-composer`):
//! - DoubleDummyTricks (compact encoding)
//! - OptimumScore (if vulnerability is known)
//! - ParContract (if vulnerability is known)
//!
//! all three one-line supplemental tag pairs, sorted alphabetically among the
//! board's other supplemental tags; and
//! - OptimumResultTable (full table)
//!
//! a supplemental *section*, below `[Auction]` and `[Play]` and sorted
//! alphabetically among any other sections.
//!
//! Boards whose deal is incomplete are passed through untouched.
//!
//! Usage:
//!   bridge-solver -i <file.pbn> -o <file.pbn>   # one file to another
//!   bridge-solver -i <file.pbn>                 # one file to stdout
//!   bridge-solver -w -i <file.pbn> <dir> ...    # annotate in place, recursively

use bridge_encodings::pbn::{
    dd_table_to_pbn, is_optimum_result_row, optimum_result_table_header, optimum_result_table_rows,
    prevailing_newline, split_lines,
};
use bridge_solver::{par, DdTricks, Hands, TableSolver};
use bridge_types::{DdTable, Direction, Strain, Vulnerability, DECLARERS};
use clap::Parser;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

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

    /// Set the "double-dummy data has been verified" bit (0x00080000) in each
    /// annotated board's [BCFlags], adding the tag if absent. Note this marks
    /// provenance; it does not make Bridge Composer display the DD table.
    #[arg(long = "mark-verified")]
    mark_verified: bool,

    /// Recompute analysis for boards that already carry it. By default a board
    /// with a [DoubleDummyTricks] tag is left exactly as found, so annotating a
    /// collection only fills in what is missing.
    #[arg(long = "recalculate")]
    recalculate: bool,

    /// Verbose output - show progress
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Worker threads used for solving. Defaults to the machine's available
    /// parallelism; `1` solves serially.
    ///
    /// Work is spread over (deal, strain) pairs rather than whole deals. A
    /// strain is the smallest piece of a table that can move to another thread
    /// without changing the search: its four declarers share one pair of caches
    /// and a chain of MTD(f) seeds, and nothing crosses the boundary from one
    /// strain to the next. Deal cost spans roughly tenfold, so the finer unit is
    /// what keeps the last threads busy instead of waiting on the slowest deal.
    ///
    /// Output does not depend on this: tables are assembled by index, not by the
    /// order the work finished, so any thread count produces identical bytes.
    #[arg(short = 'j', long = "threads", value_name = "N")]
    threads: Option<usize>,
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

    let threads = args
        .threads
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        })
        .max(1);

    // Read every input before solving any of it. The solve is one batch across
    // all of them, so a directory of one-deal files spreads over the threads
    // exactly as well as a single file of many deals does.
    let mut contents = Vec::with_capacity(files.len());
    for path in &files {
        match fs::read_to_string(path) {
            Ok(c) => contents.push(c),
            Err(e) => {
                eprintln!("Error reading input file '{}': {}", path.display(), e);
                std::process::exit(1);
            }
        }
    }

    // Pass 1 finds the boards that need analysis without solving any of them.
    // It runs the real processor with a recording stub rather than a separate
    // scanner, so the boards it asks about are exactly the boards pass 2 will
    // ask about — the "already analysed", "incomplete deal" and `--recalculate`
    // decisions are made once, by one piece of code.
    let mut pending: Vec<Hands> = Vec::new();
    for content in &contents {
        let mut collect = |hands: &Hands| {
            pending.push(*hands);
            DdTable::new()
        };
        process_pbn_with(
            content,
            false,
            args.recalculate,
            args.mark_verified,
            &mut collect,
        );
    }

    let items = pending.len() * DENOMINATIONS.len();
    let threads = threads.min(items.max(1));
    if args.verbose {
        eprintln!(
            "{} board(s) to analyse in {} file(s), on {} thread(s)",
            pending.len(),
            files.len(),
            threads
        );
    }

    let solved: Vec<DdTable> = if threads > 1 {
        solve_deals_parallel(&pending, threads, args.verbose)
    } else {
        let done = AtomicUsize::new(0);
        pending
            .iter()
            .map(|hands| {
                let table = solve_deal(hands);
                if args.verbose {
                    for _ in 0..DENOMINATIONS.len() {
                        report_progress(&done, items);
                    }
                }
                table
            })
            .collect()
    };

    // Pass 2 rebuilds each file, taking each board's table from the batch.
    let mut tables = solved.into_iter();
    let mut changed = 0usize;
    for (path, content) in files.iter().zip(&contents) {
        if args.verbose {
            eprintln!("Processing {}...", path.display());
        }
        let mut replay = |_: &Hands| match tables.next() {
            Some(table) => table,
            // Both passes run the same code over the same bytes, so they cannot
            // disagree; if they ever did, stopping beats writing a table of
            // zeroes over someone's file.
            None => {
                eprintln!("Error: internal mismatch between the analysis and writing passes");
                std::process::exit(1);
            }
        };
        let result = process_pbn_with(
            content,
            args.verbose,
            args.recalculate,
            args.mark_verified,
            &mut replay,
        );

        if args.in_place {
            // Unchanged files are left untouched so a re-run is a true no-op
            // and does not churn mtimes in a build.
            if &result == content {
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
                    // A reader that closed early is not a failure. `bridge-solver
                    // -i deals.pbn | head` ends the pipe as soon as head has what
                    // it wants, and a tool in a pipeline exits quietly there —
                    // panicking prints a backtrace over the user's terminal for
                    // something they did on purpose.
                    let mut out = io::stdout().lock();
                    if let Err(e) = out.write_all(result.as_bytes()).and_then(|()| out.flush()) {
                        if e.kind() == io::ErrorKind::BrokenPipe {
                            std::process::exit(0);
                        }
                        eprintln!("Error writing to stdout: {e}");
                        std::process::exit(1);
                    }
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

/// Bridge Composer's [BCFlags] bit meaning "double-dummy data has been
/// verified". Note this records provenance only: no documented BCFlags bit
/// controls whether the DD table is *displayed*, and Bridge Composer does not
/// set this one itself when it computes a table.
const BC_FLAG_DD_VERIFIED: u64 = 0x0008_0000;

/// Return a `[BCFlags]` line with the verified bit set, preserving every other
/// bit. An unparsable value is replaced rather than propagated, since the tag
/// is meaningless if it is not hex.
fn with_verified_bit(line: &str) -> String {
    let current = line
        .split('"')
        .nth(1)
        .and_then(|v| u64::from_str_radix(v.trim(), 16).ok())
        .unwrap_or(0);
    format!("[BCFlags \"{:x}\"]", current | BC_FLAG_DD_VERIFIED)
}

/// Process a PBN file: find deals, solve them, insert/replace DD tags.
///
/// Solves each deal inline, on the calling thread. `main` goes through
/// [`process_pbn_with`] in every case, so this is the tests' way in.
#[cfg(test)]
fn process_pbn(content: &str, verbose: bool, recalculate: bool, mark_verified: bool) -> String {
    process_pbn_with(
        content,
        verbose,
        recalculate,
        mark_verified,
        &mut solve_deal,
    )
}

/// [`process_pbn`], with each deal's table supplied by the caller.
///
/// `solve` is called once per board that needs analysis, in file order, and its
/// return value becomes that board's table. Threading is built on this: a first
/// pass records the hands it is asked about, and a second pass — running the
/// same code, so it makes the same decisions about which boards to skip — hands
/// back the tables solved in between.
///
/// # Why this is still hand-rolled
///
/// `bridge_encodings::pbn::PbnDocument` exists to retire exactly this, and it
/// was measured against the file below rather than argued about. It keeps every
/// guarantee this code was written for: an untouched document round-trips
/// byte-for-byte, `%` directives and `;` comments and `{...}` commentary all
/// survive, `is_modified()` is false after re-annotating an annotated file so a
/// re-run churns no mtimes, a board whose `[Deal]` will not parse comes back as
/// an ordinary board to be skipped rather than failing the file, and an
/// existing `OptimumResultTable` is replaced header and rows together.
///
/// What it cannot do is rank a *section* as a section. `set_tag` and
/// `set_section` share one order — mandatory tags in the standard's order,
/// then everything else alphabetically, with only `Auction`, `Play` and `Note`
/// held back — so `OptimumResultTable` lands among the one-line tags, above the
/// auction, where Bridge Composer puts it below the play. The three one-liners
/// it would place correctly; the twenty-row table it would not, and that is the
/// tag whose placement `fixtures/bridge-composer` is about. Tracked as
/// bridge-craftwork/bridge-encodings#13; when a `*Table` name sorts with the
/// sections there, this goes.
///
/// # Line endings
///
/// Lines are split by [`split_lines`], which keeps each terminator, rather than
/// by `str::lines`, which discards it. Bridge Composer writes CRLF throughout,
/// so rejoining with `\n` rewrote every line of every real-world file — which
/// made "annotating only ever adds lines" and "a re-run touches nothing" false
/// for exactly the files this tool exists to annotate.
fn process_pbn_with(
    content: &str,
    verbose: bool,
    recalculate: bool,
    mark_verified: bool,
    solve: &mut dyn FnMut(&Hands) -> DdTable,
) -> String {
    // Split into deal blocks (separated by blank lines outside of brace comments)
    let mut result = String::new();
    let mut deal_count = 0;

    // Process the file block by block
    // A block is a sequence of lines until a blank line outside of {} comments
    // Each line keeps the ending it was written with; lines this pass inserts
    // have no neighbour to copy from, so they take the file's prevailing one.
    let lines = split_lines(content);
    let newline = prevailing_newline(content);
    let mut i = 0;

    while i < lines.len() {
        // Skip leading blank lines, but preserve them
        while i < lines.len() && lines[i].0.trim().is_empty() {
            result.push_str(lines[i].0);
            result.push_str(terminator(lines[i].1, newline));
            i += 1;
        }

        if i >= lines.len() {
            break;
        }

        // Collect a deal block (all lines until next blank line outside of {} comments)
        let block_start = i;
        let mut in_brace_comment = false;

        while i < lines.len() {
            let line = lines[i].0;

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
            if i < lines.len() && lines[i].0.trim().is_empty() && !in_brace_comment {
                break;
            }
        }
        let block_end = i;

        // Process this block
        let block_lines = &lines[block_start..block_end];
        let processed = process_deal_block(
            block_lines,
            newline,
            &mut deal_count,
            verbose,
            recalculate,
            mark_verified,
            solve,
        );
        result.push_str(&processed);
    }

    // Every line above was written with a real terminator, the file's last one
    // included, so that a tag appended after it starts on its own line. Take the
    // one that was supplied back off: a file that ended without a newline still
    // ends without one.
    if !content.ends_with('\n') {
        let trimmed = result
            .strip_suffix(newline)
            .or_else(|| result.strip_suffix('\n'))
            .map(str::len);
        if let Some(len) = trimmed {
            result.truncate(len);
        }
    }

    if verbose {
        eprintln!("Processed {} deal(s)", deal_count);
    }

    result
}

/// A line's ending, with `newline` standing in for the empty one the final line
/// carries when the file ends without a newline.
///
/// Writing that line with no ending at all would run the next line — a tag this
/// pass appends — straight into it. [`process_pbn_with`] takes the final newline
/// back off the finished output instead, once, so the "no trailing newline"
/// property belongs to the file rather than to whichever line happens to be
/// last after the edit.
fn terminator<'a>(term: &'a str, newline: &'a str) -> &'a str {
    if term.is_empty() {
        newline
    } else {
        term
    }
}

/// Process a single deal block
///
/// `lines` are `(content, terminator)` pairs from [`split_lines`], so every line
/// this block passes through keeps the ending it was written with. `newline` is
/// the ending given to lines that are inserted.
fn process_deal_block(
    lines: &[(&str, &str)],
    newline: &str,
    deal_count: &mut usize,
    verbose: bool,
    recalculate: bool,
    mark_verified: bool,
    solve: &mut dyn FnMut(&Hands) -> DdTable,
) -> String {
    // Find the Deal tag to extract hands
    let mut deal_str: Option<&str> = None;
    let mut vulnerability: Option<Vulnerability> = None;

    for (line, _) in lines {
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

    // A block reproduced exactly as it was read, terminators included.
    let unchanged = || {
        let mut out = String::new();
        for (line, term) in lines {
            out.push_str(line);
            out.push_str(terminator(term, newline));
        }
        out
    };

    // If no Deal tag, just pass through unchanged
    let Some(deal_str) = deal_str else {
        return unchanged();
    };

    // Already analyzed? Leave it alone unless asked to redo the work. The
    // [DoubleDummyTricks] tag is the marker: a board carrying a stray par tag
    // but no DD table has not been analyzed, and still gets filled in.
    if !recalculate
        && lines
            .iter()
            .any(|(l, _)| extract_tag_name(l) == Some("DoubleDummyTricks"))
    {
        if verbose {
            eprintln!("Skipping board that already has analysis");
        }
        return unchanged();
    }

    // Parse the deal. A board with no cards — BridgeComposer writes
    // [Deal "N:... ... ... ..."] for auction-only teaching boards — parses fine
    // into empty hands, so completeness is checked too. Annotating one of those
    // would stamp a fabricated all-zero table and a "Pass" par onto a board that
    // has no deal to analyze.
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
    let dd_results = solve(&hands);

    // Rebuild the block: strip whatever analysis it carried, then place ours.
    let mut output_lines: Vec<(String, &str)> = Vec::new();
    let mut saw_bcflags = false;
    let mut skipping_optimum_data = false;

    // Tags we generate: strip whatever the board carried, so that ours are
    // placed rather than replaced in situ. A board annotated by an older build
    // has them all above the auction; leaving them there would keep them there.
    let dd_tag_names = [
        "DoubleDummyTricks",
        "OptimumScore",
        "ParContract",
        "OptimumResultTable",
    ];

    for (line, term) in lines {
        let trimmed = line.trim();
        let term = terminator(term, newline);

        // Fold the verified bit into an existing [BCFlags], keeping every other
        // bit the board already carried.
        if mark_verified && extract_tag_name(trimmed) == Some("BCFlags") {
            saw_bcflags = true;
            output_lines.push((with_verified_bit(trimmed), term));
            continue;
        }

        if let Some(tag_name) = extract_tag_name(trimmed) {
            if dd_tag_names.contains(&tag_name) {
                // The twenty rows below an [OptimumResultTable] header belong to
                // it and go with it.
                skipping_optimum_data = tag_name == "OptimumResultTable";
                continue;
            }
        }

        if skipping_optimum_data {
            if is_optimum_result_row(line) {
                continue;
            }
            skipping_optimum_data = false;
        }

        output_lines.push((line.to_string(), term));
    }

    // Place what we write the way Bridge Composer does: the one-line tags among
    // the identification tags, sorted alphabetically with whatever supplemental
    // tags the board already has; the table below [Auction] and [Play], sorted
    // alphabetically among any other sections. See `fixtures/bridge-composer`.
    let mut one_liners: Vec<(&str, String)> = Vec::new();
    // A board with no [BCFlags] of its own still needs one to carry the bit, and
    // it sorts with the rest: `BCFlags` < `DoubleDummyTricks`.
    if mark_verified && !saw_bcflags {
        one_liners.push((
            "BCFlags",
            format!("[BCFlags \"{:x}\"]{newline}", BC_FLAG_DD_VERIFIED),
        ));
    }
    one_liners.extend(dd_tag_pairs(&dd_results, vulnerability, newline));

    let identification_end = first_section(&output_lines);
    let mut insertions: Vec<(usize, String)> = one_liners
        .into_iter()
        .map(|(name, text)| {
            (
                tag_pair_position(&output_lines, identification_end, name),
                text,
            )
        })
        .collect();
    insertions.push((
        optimum_result_table_position(&output_lines, identification_end),
        optimum_result_section(&dd_results, newline),
    ));
    // Stable, so tags landing at the same line keep the alphabetical order they
    // were built in, and the table stays last of them.
    insertions.sort_by_key(|(at, _)| *at);

    let mut result = String::new();
    let mut next = 0;
    for (idx, (line, term)) in output_lines.iter().enumerate() {
        while insertions.get(next).is_some_and(|(at, _)| *at == idx) {
            result.push_str(&insertions[next].1);
            next += 1;
        }
        result.push_str(line);
        result.push_str(term);
    }
    // Anything ranked past the last line goes on the end.
    for (_, text) in &insertions[next..] {
        result.push_str(text);
    }

    result
}

/// The 15 tag pairs PBN 2.1 §3.4 requires of every game.
///
/// A copy of the list `bridge_encodings::pbn` keeps privately, needed here for
/// the same reason it needs it: a supplemental tag is ranked alphabetically
/// among the *supplemental* tags, and `Result` — mandatory, and the last of
/// them — would otherwise sort above `OptimumScore` and take the analysis with
/// it. Order within the list does not matter here, only membership: this
/// binary places tags relative to the mandatory block without reordering it.
const MANDATORY_TAGS: [&str; 15] = [
    "Event",
    "Site",
    "Date",
    "Board",
    "West",
    "North",
    "East",
    "South",
    "Dealer",
    "Vulnerable",
    "Deal",
    "Scoring",
    "Declarer",
    "Contract",
    "Result",
];

/// The end of the board's identification section: the first line that opens a
/// section, or the end of the block if it has none.
///
/// Everything above this is tag pairs (with any commentary between them);
/// everything below belongs to a section. A supplemental tag pair goes above
/// it, since a section owns every line beneath its header.
fn first_section(lines: &[(String, &str)]) -> usize {
    lines
        .iter()
        .position(|(line, _)| extract_tag_name(line.trim()).is_some_and(starts_a_section))
        .unwrap_or(lines.len())
}

/// Where a supplemental tag pair named `name` goes: after every mandatory tag
/// and every supplemental tag sorting above it, and above the rest.
///
/// Mandatory tags always rank first, wherever the file happens to put them, so
/// a board whose mandatory block is out of order still gets the analysis below
/// it rather than wedged into the middle. Nothing already in the block is
/// moved: we insert into the order the board has, we do not impose one.
fn tag_pair_position(lines: &[(String, &str)], identification_end: usize, name: &str) -> usize {
    let mut at = 0;
    for (idx, (line, _)) in lines[..identification_end].iter().enumerate() {
        if let Some(tag) = extract_tag_name(line.trim()) {
            if MANDATORY_TAGS.contains(&tag) || tag < name {
                at = idx + 1;
            }
        }
    }
    at
}

/// The sections of a block, as `(name, header index, end index)`.
///
/// A section runs from its header to the next tag pair of any kind — that is
/// what ends one, per PBN 2.1 §5.5 — so its data lines, and any commentary
/// among them, travel with it.
fn sections<'a>(lines: &'a [(String, &str)], from: usize) -> Vec<(&'a str, usize, usize)> {
    let mut found = Vec::new();
    let mut idx = from;
    while idx < lines.len() {
        let name = extract_tag_name(lines[idx].0.trim()).filter(|n| starts_a_section(n));
        let Some(name) = name else {
            idx += 1;
            continue;
        };
        let mut end = idx + 1;
        while end < lines.len() && extract_tag_name(lines[end].0.trim()).is_none() {
            end += 1;
        }
        found.push((name, idx, end));
        idx = end;
    }
    found
}

/// Where the `[OptimumResultTable]` section goes: below `[Auction]` and
/// `[Play]`, and alphabetically among the board's other sections.
///
/// Bridge Composer writes the game record first and the supplemental sections
/// after it, sorted among themselves — board 8 of `fixtures/bridge-composer`
/// carries a custom `AAATable`, written *above* the auction, and comes back
/// below the auction and above `OptimumResultTable`. Alphabetical order yields
/// to that: a section sorting above ours but written below the auction keeps
/// the table beneath it, because the game record wins.
fn optimum_result_table_position(lines: &[(String, &str)], identification_end: usize) -> usize {
    const NAME: &str = "OptimumResultTable";
    let sections = sections(lines, identification_end);

    // The earliest line the table may take.
    let mut earliest = identification_end;
    for (name, _, end) in &sections {
        if matches!(*name, "Auction" | "Play") || *name < NAME {
            earliest = earliest.max(*end);
        }
    }

    // ...and then above the first section that sorts below it.
    sections
        .iter()
        .find(|(name, start, _)| *start >= earliest && *name > NAME)
        .map_or(lines.len(), |(_, start, _)| *start)
}

/// Whether `[TagName ...]` opens a section whose data lines follow it.
///
/// PBN 2.1 gives `[Auction]` (§5.5) and `[Play]` (§5.6) the lines beneath them
/// until the next tag pair, and §7 does the same for every table tag — a name
/// ending in `Table`. Nothing may be inserted between such a header and its
/// rows: a reader following the standard would see an auction with no calls,
/// and then a run of stray call tokens after whatever was wedged in.
///
/// This is the boundary the whole placement is built on: everything above the
/// first section is the identification block, where the one-line tags are
/// ranked alphabetically, and everything below is game record, where the table
/// goes. Ranking the table alphabetically instead — `"Auction" <
/// "DoubleDummyTricks"` — is how it once came to be written into the middle of
/// the auction.
fn starts_a_section(tag_name: &str) -> bool {
    tag_name == "Auction" || tag_name == "Play" || tag_name.ends_with("Table")
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

    // `bridge_types` owns the spelling table, and is what `bridge-encodings`
    // and `pbn-to-pdf` already parse this tag with. Keeping a private copy here
    // is how this binary came to accept `"N"` and `"E"` — which PBN 2.1 §3.4.10
    // does not define — while rejecting the `"N-S"` and `"E-W"` that everything
    // else in the family accepts.
    Vulnerability::from_pbn(&trimmed[start..end])
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

/// The five strains, in the order this binary hands them out as work.
///
/// A strain is the unit of parallel work, and the smallest one that does not
/// change the search: its four declarers share one pair of caches and a chain
/// of MTD(f) seeds, and nothing crosses the boundary from one strain to the
/// next. [`TableSolver::solve_strain_hands`] is that unit, so this array is
/// only a work order — any order gives the same table.
const DENOMINATIONS: [Strain; 5] = [
    Strain::NoTrump,
    Strain::Spades,
    Strain::Hearts,
    Strain::Diamonds,
    Strain::Clubs,
];

/// Place one strain's four solved cells into `table`.
///
/// [`TableSolver::solve_strain_hands`] returns its column in `N, E, S, W`
/// order — [`Direction::to_index`], which is the order
/// [`bridge_types::DECLARERS`] lists — so the two are zipped rather than
/// transposed by hand. `solve_deal_matches_the_library_table` is the guard
/// that they still agree.
fn place_column(table: &mut DdTable, strain: Strain, column: [u8; 4]) {
    for (declarer, tricks) in DECLARERS.into_iter().zip(column) {
        table.set(declarer, strain, tricks);
    }
}

/// Note one finished work item and, at each ten percent, say so.
///
/// The decile test is true for exactly one value of `n`, so exactly one thread
/// prints each line however many are running and in whatever order they finish.
fn report_progress(done: &AtomicUsize, items: usize) {
    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
    if items > 0 && n * 10 / items != (n - 1) * 10 / items {
        eprintln!("  {}% ({n} of {items} strains)", n * 100 / items);
    }
}

/// Solve a deal and return its DD table, on the calling thread.
fn solve_deal(hands: &Hands) -> DdTable {
    let mut solver = TableSolver::new();
    let mut table = DdTable::new();
    for strain in DENOMINATIONS {
        place_column(
            &mut table,
            strain,
            solver.solve_strain_hands(*hands, strain),
        );
    }
    table
}

/// Solve every deal in `deals` across `threads` workers, returning one table per
/// deal in the order given.
///
/// Work is handed out one (deal, strain) pair at a time from a shared counter,
/// so a thread that draws a cheap strain comes straight back for another. That
/// matters because deal cost spans roughly tenfold: scheduling whole deals ends
/// the run when the slowest deal ends, with most threads long since idle, which
/// is the load-imbalance signature `bench/comparison/RESULTS.md` measured.
///
/// Each worker accumulates its own results and they are merged after the join,
/// so the only state shared between threads is the counter, and the merge is by
/// index — the output is identical whatever order the work completed in.
///
/// A worker keeps one [`TableSolver`] for its whole run rather than building a
/// pair of caches per strain. `CutoffCache::new(16)` alone is a megabyte, and
/// both caches then double their way up to whatever the deal wants; a solver
/// held across items keeps the grown capacity and resets the entries, which is
/// what the C++ reference does with its process globals.
fn solve_deals_parallel(deals: &[Hands], threads: usize, verbose: bool) -> Vec<DdTable> {
    let items = deals.len() * DENOMINATIONS.len();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);

    let harvest = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut solver = TableSolver::new();
                    let mut mine: Vec<(usize, usize, [u8; 4])> = Vec::new();
                    loop {
                        let item = next.fetch_add(1, Ordering::Relaxed);
                        if item >= items {
                            break;
                        }
                        let (deal_idx, denom_idx) =
                            (item / DENOMINATIONS.len(), item % DENOMINATIONS.len());
                        let column =
                            solver.solve_strain_hands(deals[deal_idx], DENOMINATIONS[denom_idx]);
                        mine.push((deal_idx, denom_idx, column));
                        if verbose {
                            report_progress(&done, items);
                        }
                    }
                    mine
                })
            })
            .collect();

        workers.into_iter().map(|w| w.join()).collect::<Vec<_>>()
    });

    let mut tables = vec![DdTable::new(); deals.len()];
    for worker in harvest {
        // A worker only ends by panicking, which has already printed its own
        // message; carrying on would silently write a table of zeroes.
        let Ok(found) = worker else {
            eprintln!("Error: a solver thread panicked; no files were written");
            std::process::exit(1);
        };
        for (deal_idx, denom_idx, column) in found {
            place_column(&mut tables[deal_idx], DENOMINATIONS[denom_idx], column);
        }
    }
    tables
}

/// The one-line supplemental tags this binary writes, in alphabetical order,
/// each a whole line ending with `newline`.
///
/// Both encodings of the table come from `bridge_encodings::pbn`, which is the
/// one place that says how a `DdTable` is written down. What stays here is the
/// choice of which tags to write — the CLI's job. They come out in alphabetical
/// order because that is the order they are placed in, which
/// `dd_tag_pairs_are_alphabetical` holds us to.
///
/// Every line ends with `newline`, which is the ending the rest of the file
/// uses, so an annotated CRLF file stays a CRLF file throughout.
fn dd_tag_pairs(
    table: &DdTable,
    vulnerability: Option<Vulnerability>,
    newline: &str,
) -> Vec<(&'static str, String)> {
    let mut tags = vec![(
        "DoubleDummyTricks",
        format!(
            "[DoubleDummyTricks \"{}\"]{newline}",
            dd_table_to_pbn(table)
        ),
    )];

    // Par needs vulnerability to score; without it the board gets a table and
    // no par, which is also what a board with no [Vulnerable] tag gets.
    if let Some(vul) = vulnerability {
        let p = par(
            &to_par_table(table),
            vul.is_vulnerable(Direction::North),
            vul.is_vulnerable(Direction::East),
        );
        tags.push((
            "OptimumScore",
            format!("[OptimumScore \"{}\"]{newline}", p.optimum_score()),
        ));
        if let Some(contracts) = p.par_contract() {
            tags.push((
                "ParContract",
                format!("[ParContract \"{contracts}\"]{newline}"),
            ));
        }
    }
    tags
}

/// The `[OptimumResultTable]` section: its header and its twenty rows.
///
/// The `Result` column is one character wide when no declarer takes ten tricks
/// and two when one does — header and rows together, both from
/// `bridge_encodings::pbn`, which is what Bridge Composer writes. A fixed
/// `\2R` had every single-digit board's table rewritten the moment someone
/// opened and saved the file there.
fn optimum_result_section(table: &DdTable, newline: &str) -> String {
    let mut output = format!(
        "[OptimumResultTable \"{}\"]{newline}",
        optimum_result_table_header(table)
    );
    for row in optimum_result_table_rows(table) {
        output.push_str(&row);
        output.push_str(newline);
    }
    output
}

/// Copy a [`DdTable`] into the library's `DdTricks`, which [`par`] takes.
///
/// `DdTricks` is indexed positionally — seats N,E,S,W down the rows, strains
/// C,D,H,S,NT across the columns — so the two orders have to meet somewhere,
/// and this is the only place they do. `DdTable` cannot be indexed positionally
/// at all, which is what keeps the transcription honest.
fn to_par_table(table: &DdTable) -> DdTricks {
    const SEATS: [Direction; 4] = [
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
    ];
    let mut tricks = [[0u8; 5]; 4];
    for (row, declarer) in SEATS.iter().enumerate() {
        for (column, strain) in bridge_solver::STRAINS.iter().enumerate() {
            tricks[row][column] = table.tricks(*declarer, *strain);
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

    /// Every spelling PBN 2.1 §3.4.10 defines, and nothing else.
    #[test]
    fn test_extract_vulnerability() {
        use Vulnerability::{Both, EastWest, None as NoneVul, NorthSouth};
        for (value, expected) in [
            ("None", NoneVul),
            ("Love", NoneVul),
            ("-", NoneVul),
            ("NS", NorthSouth),
            ("EW", EastWest),
            ("All", Both),
            ("Both", Both),
        ] {
            assert_eq!(
                extract_vulnerability_tag(&format!("[Vulnerable \"{value}\"]")),
                Some(expected),
                "{value}"
            );
        }
    }

    /// Case is not significant, which the spec's own mixed-case examples imply.
    #[test]
    fn vulnerability_is_case_insensitive() {
        assert_eq!(
            extract_vulnerability_tag("[Vulnerable \"none\"]"),
            Some(Vulnerability::None)
        );
        assert_eq!(
            extract_vulnerability_tag("[Vulnerable \"bOtH\"]"),
            Some(Vulnerability::Both)
        );
    }

    /// Gained by moving to `bridge_types`. Not in the spec, but every other
    /// crate in the family accepts them, and this binary used to be the one
    /// that silently emitted no par contract for a board written this way.
    #[test]
    fn vulnerability_accepts_the_hyphenated_forms() {
        assert_eq!(
            extract_vulnerability_tag("[Vulnerable \"N-S\"]"),
            Some(Vulnerability::NorthSouth)
        );
        assert_eq!(
            extract_vulnerability_tag("[Vulnerable \"E-W\"]"),
            Some(Vulnerability::EastWest)
        );
    }

    /// Lost by moving to `bridge_types`, deliberately: PBN 2.1 §3.4.10 does not
    /// define bare `"N"` or `"E"`, and nothing here ever produced them. An
    /// unrecognised value means "no vulnerability stated", so such a board keeps
    /// its double-dummy table and simply gets no par — the same treatment as a
    /// board with no `[Vulnerable]` tag at all.
    #[test]
    fn vulnerability_rejects_undefined_spellings() {
        for value in ["N", "E", "S", "W", "NorthSouth", ""] {
            assert_eq!(
                extract_vulnerability_tag(&format!("[Vulnerable \"{value}\"]")),
                Option::None,
                "{value}"
            );
        }
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

    /// The rows this binary used to recognise, now recognised by the shared
    /// predicate. Only the rows matter here: what follows an
    /// `[OptimumResultTable]` header is skipped so the stale table can be
    /// replaced, and a line wrongly kept would be duplicated into the output.
    #[test]
    fn test_is_optimum_result_row() {
        assert!(is_optimum_result_row("N NT  3"));
        assert!(is_optimum_result_row("S  S 10"));
        assert!(is_optimum_result_row("E  H  7"));
        assert!(!is_optimum_result_row("[Deal \"...\"]"));
        assert!(!is_optimum_result_row(""));
        assert!(!is_optimum_result_row("[OptimumResultTable \"...\"]"));
    }

    /// A table this binary would have written before the codec moved, checked
    /// against the value Bridge Composer writes for it. This is the guard that
    /// the shared codec's row and column orders are the ones this CLI has
    /// always emitted: `N,S,E,W` by row and `NT,S,H,D,C` by column.
    #[test]
    fn test_encode_ddt() {
        // 0-9 -> '0'-'9', 10-13 -> 'a'-'d'. From Bridge Composer:
        // "32691326914a74a4a74a".
        let rows: [(Direction, [u8; 5]); 4] = [
            (Direction::North, [3, 2, 6, 9, 1]),  // NT=3 S=2 H=6 D=9 C=1
            (Direction::South, [3, 2, 6, 9, 1]),  // same
            (Direction::East, [4, 10, 7, 4, 10]), // NT=4 S=10 H=7 D=4 C=10
            (Direction::West, [4, 10, 7, 4, 10]), // same
        ];
        let mut table = DdTable::new();
        for (declarer, cells) in rows {
            for (strain, tricks) in DENOMINATIONS.iter().zip(cells) {
                table.set(declarer, *strain, tricks);
            }
        }
        assert_eq!(dd_table_to_pbn(&table), "32691326914a74a4a74a");
    }

    #[test]
    fn test_process_simple_pbn() {
        // Use a real 52-card deal from Bridge Composer reference
        let pbn = r#"[Event "Test"]
[Vulnerable "None"]
[Deal "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72"]
[Dealer "N"]
"#;
        let result = process_pbn(pbn, false, false, false);
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
        let result = process_pbn(pbn, false, true, false);
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
        let kept = process_pbn(pbn, false, false, false);
        assert_eq!(kept, pbn, "default must not touch an analyzed board");

        let redone = process_pbn(pbn, false, true, false);
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
        let result = process_pbn(pbn, false, false, false);
        assert_eq!(result.matches("[DoubleDummyTricks").count(), 1);
        assert!(!result.contains("NS 9999"));
    }

    /// --mark-verified folds bit 0x00080000 into the board's existing BCFlags
    /// without disturbing the bits it already carried.
    #[test]
    fn test_mark_verified_preserves_other_bcflags_bits() {
        let pbn = r#"[Event "Test"]
[Vulnerable "None"]
[Deal "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72"]
[BCFlags "40001f"]
"#;
        let result = process_pbn(pbn, false, false, true);
        // 0x40001f | 0x80000 == 0x48001f — every original bit survives.
        assert!(
            result.contains(r#"[BCFlags "48001f"]"#),
            "got:
{result}"
        );
        assert_eq!(result.matches("[BCFlags").count(), 1);

        // Without the flag the tag is left exactly as written.
        let plain = process_pbn(pbn, false, false, false);
        assert!(plain.contains(r#"[BCFlags "40001f"]"#));
    }

    /// A board with no BCFlags of its own gets one carrying just that bit.
    #[test]
    fn test_mark_verified_adds_bcflags_when_absent() {
        let pbn = r#"[Event "Test"]
[Vulnerable "None"]
[Deal "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72"]
"#;
        let result = process_pbn(pbn, false, false, true);
        assert!(
            result.contains(r#"[BCFlags "80000"]"#),
            "got:
{result}"
        );
        assert_eq!(result.matches("[BCFlags").count(), 1);
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
        let result = process_pbn(pbn, false, false, false);
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
        let result = process_pbn(pbn, false, false, false);
        assert_eq!(result.matches("[DoubleDummyTricks").count(), 1);
        assert!(!result.contains("\"00000000000000000000\""));
    }

    /// A complete board with an auction, for the placement tests. `AP` ends the
    /// auction, so the calls are the whole of the `[Auction]` section.
    const AUCTION_BOARD: &str = concat!(
        "[Event \"Rich\"]\n",
        "[Board \"4\"]\n",
        "[Vulnerable \"EW\"]\n",
        "[Deal \"N:Q7432.85.J983.63 J65.64.AKT5.AK98 AK98.AKQJ7.6.QJ7 T.T932.Q742.T542\"]\n",
        "[Auction \"N\"]\n",
        "1S Pass 2S AP\n",
        "[Play \"E\"]\n",
        "HA H2 H3 H4\n",
    );

    /// The index of the first line equal to `wanted`.
    fn line_of(text: &str, wanted: &str) -> usize {
        text.lines()
            .position(|l| l == wanted)
            .unwrap_or_else(|| panic!("no {wanted:?} line in:\n{text}"))
    }

    /// Issue #22, and then the Bridge Composer fixture. `[Auction]` and
    /// `[Play]` own every line below them until the next tag pair, so nothing
    /// may be inserted between a header and its data — ranking the insertion
    /// point alphabetically once put the whole twenty-row table between
    /// `[Auction "N"]` and its calls. The tags then went *above* the auction,
    /// all four of them, until `fixtures/bridge-composer` showed that Bridge
    /// Composer splits them: one-liners above, the table below the game record.
    #[test]
    fn one_liners_go_above_the_auction_and_the_table_below_the_play() {
        let result = process_pbn(AUCTION_BOARD, false, false, false);
        let lines: Vec<&str> = result.lines().collect();

        // Each section header is still followed immediately by its own data.
        let auction = line_of(&result, "[Auction \"N\"]");
        assert_eq!(lines[auction + 1], "1S Pass 2S AP");
        let play = line_of(&result, "[Play \"E\"]");
        assert_eq!(lines[play + 1], "HA H2 H3 H4");

        // The one-line tags sit between the deal and the auction...
        let deal = lines
            .iter()
            .position(|l| l.starts_with("[Deal "))
            .unwrap_or_else(|| panic!("no deal in:\n{result}"));
        for tag in ["[DoubleDummyTricks", "[OptimumScore", "[ParContract"] {
            let at = lines
                .iter()
                .position(|l| l.starts_with(tag))
                .unwrap_or_else(|| panic!("no {tag} in:\n{result}"));
            assert!(
                at > deal && at < auction,
                "{tag} at {at}, deal at {deal}, auction at {auction}"
            );
        }

        // ...and the table, with its twenty rows, below the play.
        let table = line_of(
            &result,
            "[OptimumResultTable \"Declarer;Denomination\\2R;Result\\2R\"]",
        );
        assert!(table > play, "table at {table}, play at {play}");
        assert_eq!(lines.len() - table - 1, 20, "got:\n{result}");
    }

    /// Group 2 of Bridge Composer's layout: supplemental tag *pairs*, sorted
    /// alphabetically among themselves, custom tags included. Board 7 of
    /// `fixtures/bridge-composer` proves it with `AAACustom` and `ZZZCustom`
    /// bracketing the analysis, so ours has to sort into the same place.
    #[test]
    fn the_one_liners_sort_among_the_boards_own_supplemental_tags() {
        let pbn = concat!(
            "[Board \"1\"]\n",
            "[Vulnerable \"None\"]\n",
            "[Deal \"N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72\"]\n",
            "[Result \"\"]\n",
            "[AAACustom \"first\"]\n",
            "[Generator \"between the analysis tags\"]\n",
            "[ZZZCustom \"last\"]\n",
        );
        let result = process_pbn(pbn, false, false, false);
        let names: Vec<&str> = result
            .lines()
            .filter_map(|l| extract_tag_name(l.trim()))
            .filter(|n| !MANDATORY_TAGS.contains(n))
            .collect();
        assert_eq!(
            names,
            [
                "AAACustom",
                "DoubleDummyTricks",
                "Generator",
                "OptimumScore",
                "ParContract",
                "ZZZCustom",
                "OptimumResultTable",
            ],
            "got:\n{result}"
        );
    }

    /// Group 5: supplemental *sections*, below the game record and sorted
    /// alphabetically among themselves. `AAATable` was written above the
    /// auction on board 8 of the fixture and came back below it, so the game
    /// record outranks the sort — and `ZZZTable` keeps ours above it.
    #[test]
    fn the_table_sorts_among_the_boards_own_sections() {
        let pbn = concat!(
            "[Board \"1\"]\n",
            "[Vulnerable \"None\"]\n",
            "[Deal \"N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72\"]\n",
            "[AAATable \"Declarer;Result\\2R\"]\n",
            "N  1\n",
            "[Auction \"N\"]\n",
            "1S Pass 2S AP\n",
            "[ZZZTable \"Declarer;Result\\2R\"]\n",
            "S  2\n",
        );
        let result = process_pbn(pbn, false, false, false);
        let sections: Vec<&str> = result
            .lines()
            .filter_map(|l| extract_tag_name(l.trim()))
            .filter(|n| starts_a_section(n))
            .collect();
        assert_eq!(
            sections,
            ["AAATable", "Auction", "OptimumResultTable", "ZZZTable"],
            "got:\n{result}"
        );
        // Each header still owns its own rows.
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(
            lines[line_of(&result, "N  1") - 1],
            "[AAATable \"Declarer;Result\\2R\"]"
        );
        assert_eq!(
            lines[line_of(&result, "S  2") - 1],
            "[ZZZTable \"Declarer;Result\\2R\"]"
        );
    }

    /// The Bridge Composer oracle, end to end: annotate the fixture ourselves
    /// and compare against the file Bridge Composer 5.118.2 produced from the
    /// same input, board by board.
    ///
    /// Equality of the whole file is not the goal and never will be — Bridge
    /// Composer reorders the mandatory tags, adds `[BCFlags]` and a preamble of
    /// its own settings, and rewrites `;` comments as `{...}` commentary, none
    /// of which we do. What must agree is everything we write: all four tag
    /// values, the header's `Result` width, the twenty cells, and where the
    /// tags sit relative to `[Auction]` and `[Play]`.
    ///
    /// The table is compared byte for byte, header and rows: Bridge Composer
    /// narrows the data rows along with the header, so a `Result\1R` board
    /// reads `N NT 5` and a `\2R` board `N NT  9`, and
    /// `bridge_encodings::pbn` derives both widths from the same table. Four
    /// boards here are narrow and four are wide.
    #[test]
    fn the_bridge_composer_fixture_round_trips() {
        const OURS: &str = include_str!("../../../fixtures/bridge-composer/pbn-order-test.pbn");
        const THEIRS: &str =
            include_str!("../../../fixtures/bridge-composer/pbn-order-test-bc-dd.pbn");

        let annotated = process_pbn(OURS, false, true, false);
        let ours = analysis_of(&annotated);
        let theirs = analysis_of(THEIRS);
        assert_eq!(ours.len(), 8, "expected eight boards, got {}", ours.len());
        assert_eq!(theirs.len(), ours.len());

        for (board, (ours, theirs)) in ours.iter().zip(&theirs).enumerate() {
            let board = board + 1;
            assert_eq!(ours.tags, theirs.tags, "board {board}");
            assert_eq!(ours.rows, theirs.rows, "board {board}");
            // The table is the last section on the board, so it is below both
            // [Auction] and [Play] and below any section sorting above it.
            assert_eq!(
                ours.sections.last(),
                Some(&"OptimumResultTable"),
                "board {board}"
            );
        }

        // Board 8 is the one place the section order differs, and it is not
        // ours: Bridge Composer moved the board's own `AAATable` from above the
        // auction to below it. We do not move tags we did not write, so ours
        // reads `AAATable, Auction, OptimumResultTable` where theirs reads
        // `Auction, AAATable, OptimumResultTable`. The table is last either way.
        assert_eq!(
            ours[7].sections,
            ["AAATable", "Auction", "OptimumResultTable"]
        );
        assert_eq!(
            theirs[7].sections,
            ["Auction", "AAATable", "OptimumResultTable"]
        );
        for (ours, theirs) in ours.iter().zip(&theirs).take(7) {
            assert_eq!(ours.sections, theirs.sections);
        }
    }

    /// One board's analysis: the part of a PBN record this binary is
    /// responsible for.
    #[derive(Debug, PartialEq, Eq)]
    struct Analysis<'a> {
        /// The analysis tag lines, exactly as written.
        tags: Vec<&'a str>,
        /// The table's rows, as written — column widths included, since those
        /// are Bridge Composer's too.
        rows: Vec<&'a str>,
        /// The board's section headers, in file order.
        sections: Vec<&'a str>,
    }

    /// Every board's [`Analysis`], in file order.
    ///
    /// Boards without a dealt hand (Bridge Composer's template record) are
    /// skipped, so the two files line up board for board.
    fn analysis_of(pbn: &str) -> Vec<Analysis<'_>> {
        let mut boards = Vec::new();
        // Blank lines separate records; `str::lines` drops the file's CRLF for
        // us, which is all this needs to read either file.
        let mut blocks: Vec<Vec<&str>> = vec![Vec::new()];
        for line in pbn.lines() {
            if line.trim().is_empty() {
                blocks.push(Vec::new());
            } else if let Some(block) = blocks.last_mut() {
                block.push(line);
            }
        }

        for block in blocks {
            // Bridge Composer writes a template record with an empty [Deal];
            // skipping it lines the two files up board for board.
            if !block
                .iter()
                .any(|l| l.starts_with("[Deal \"") && l.len() > 10)
            {
                continue;
            }
            let mut found = Analysis {
                tags: Vec::new(),
                rows: Vec::new(),
                sections: Vec::new(),
            };
            let mut in_table = false;
            for line in block {
                match extract_tag_name(line.trim()) {
                    Some(name) => {
                        in_table = name == "OptimumResultTable";
                        if ["DoubleDummyTricks", "OptimumScore", "ParContract"].contains(&name)
                            || in_table
                        {
                            found.tags.push(line);
                        }
                        if starts_a_section(name) {
                            found.sections.push(name);
                        }
                    }
                    None if in_table => found.rows.push(line),
                    None => {}
                }
            }
            boards.push(found);
        }
        boards
    }

    /// The one-liners are emitted in the order they are placed in, so a board
    /// with no supplemental tags of its own gets them in one alphabetical run.
    #[test]
    fn dd_tag_pairs_are_alphabetical() {
        let table = solve_deal(
            &Hands::from_pbn(
                "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72",
            )
            .expect("sample deal parses"),
        );
        let names: Vec<&str> = dd_tag_pairs(&table, Some(Vulnerability::None), "\n")
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, ["DoubleDummyTricks", "OptimumScore", "ParContract"]);
        assert!(names.windows(2).all(|w| w[0] < w[1]));
    }

    /// Only the placement moved: the same tags with the same values are written.
    #[test]
    fn moving_the_tags_did_not_change_them() {
        let result = process_pbn(AUCTION_BOARD, false, false, false);
        assert!(result.contains(r#"[DoubleDummyTricks "7a9547a9544248942489"]"#));
        assert!(result.contains(r#"[OptimumScore "NS 420"]"#));
        assert!(result.contains(r#"[ParContract "NS 4S="]"#));
    }

    /// A board an older build corrupted — the tags written inside the auction —
    /// is repaired by `--recalculate` rather than having them replaced in place.
    #[test]
    fn recalculating_lifts_tags_out_of_the_auction() {
        let mut corrupted = String::new();
        for line in AUCTION_BOARD.lines() {
            corrupted.push_str(line);
            corrupted.push('\n');
            if line == "[Auction \"N\"]" {
                corrupted.push_str("[DoubleDummyTricks \"00000000000000000000\"]\n");
                corrupted.push_str("[OptimumScore \"NS 0\"]\n");
                corrupted.push_str("[ParContract \"NS Pass\"]\n");
                corrupted
                    .push_str("[OptimumResultTable \"Declarer;Denomination\\2R;Result\\2R\"]\n");
                for declarer in ["N", "S", "E", "W"] {
                    for strain in ["NT", " S", " H", " D", " C"] {
                        corrupted.push_str(&format!("{declarer} {strain}  0\n"));
                    }
                }
            }
        }

        let result = process_pbn(&corrupted, false, true, false);
        let lines: Vec<&str> = result.lines().collect();
        let auction = line_of(&result, "[Auction \"N\"]");
        assert_eq!(lines[auction + 1], "1S Pass 2S AP");
        assert_eq!(result.matches("[DoubleDummyTricks").count(), 1);
        assert!(!result.contains("\"00000000000000000000\""));
    }

    /// Any `*Table` tag is a section header too, per PBN 2.1 §7, and
    /// `"ActionTable" < "DoubleDummyTricks"` — so without the rule its rows
    /// would have been split from their header the same way the auction was.
    #[test]
    fn table_tags_are_section_headers_too() {
        let pbn = concat!(
            "[Board \"1\"]\n",
            "[Vulnerable \"None\"]\n",
            "[Deal \"N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72\"]\n",
            "[ActionTable \"Player;Action\"]\n",
            "N 1S\n",
        );
        let result = process_pbn(pbn, false, false, false);
        let lines: Vec<&str> = result.lines().collect();
        let header = line_of(&result, "[ActionTable \"Player;Action\"]");
        assert_eq!(lines[header + 1], "N 1S");
        assert!(
            lines[..header]
                .iter()
                .any(|l| l.starts_with("[DoubleDummyTricks")),
            "analysis must sit above the table header:\n{result}"
        );
    }

    /// Issue #24. `str::lines` discards the terminator, so rejoining with `\n`
    /// rewrote every line of every CRLF file. Bridge Composer writes CRLF, so
    /// that was every real-world file.
    #[test]
    fn crlf_files_stay_crlf() {
        let pbn = AUCTION_BOARD.replace('\n', "\r\n");
        let result = process_pbn(&pbn, false, false, false);
        assert!(result.contains("[DoubleDummyTricks"));
        assert_eq!(
            result.matches('\n').count(),
            result.matches("\r\n").count(),
            "a bare LF survived:\n{result:?}"
        );
        // The inserted lines took the file's ending, not the compiled-in one.
        assert!(result.contains("[OptimumScore \"NS 420\"]\r\n"));
        assert!(result.contains("N NT  7\r\n"));
    }

    /// An LF file is not "corrected" to CRLF either: each line keeps what it had.
    #[test]
    fn lf_files_stay_lf() {
        let result = process_pbn(AUCTION_BOARD, false, false, false);
        assert!(result.contains("[DoubleDummyTricks"));
        assert_eq!(
            result.matches('\r').count(),
            0,
            "a CR appeared:\n{result:?}"
        );
    }

    /// The reported symptom: a CRLF file that already carries a complete
    /// analysis has nothing to add, so it must come back byte-for-byte — which
    /// is what `main` compares to decide whether to write the file at all.
    /// Before the fix it reported "1 of 1 file(s) updated" and came back LF.
    #[test]
    fn an_already_annotated_crlf_file_is_not_rewritten() {
        let annotated = process_pbn(&AUCTION_BOARD.replace('\n', "\r\n"), false, false, false);
        let again = process_pbn(&annotated, false, false, false);
        assert_eq!(again, annotated, "re-annotating must be a byte-level no-op");
    }

    /// A file whose lines disagree keeps each one as it was written; only the
    /// inserted lines need a choice made for them, and they take the majority.
    #[test]
    fn mixed_endings_are_kept_line_by_line() {
        let pbn = concat!(
            "[Board \"1\"]\r\n",
            "[Vulnerable \"None\"]\n",
            "[Deal \"N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72\"]\r\n",
        );
        let result = process_pbn(pbn, false, false, false);
        assert!(result.starts_with(pbn), "input lines changed:\n{result:?}");
        // Two CRLF against one LF, so insertions are CRLF.
        assert!(result.contains("[DoubleDummyTricks \"9a8789a8784346543465\"]\r\n"));
    }

    /// A file that ended without a newline still ends without one, whether or
    /// not anything was appended to it.
    #[test]
    fn a_missing_final_newline_is_not_added() {
        let deal =
            "[Deal \"N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72\"]";
        let pbn = format!("[Board \"1\"]\n[Vulnerable \"None\"]\n{deal}");

        // Nothing to add: byte-for-byte, no newline grown.
        let annotated = process_pbn(&pbn, false, false, false);
        assert!(
            !annotated.ends_with('\n'),
            "gained a newline:\n{annotated:?}"
        );
        // The deal line kept a real terminator, so the appended tag is its own
        // line rather than being run onto the end of it.
        assert!(annotated.contains(&format!("{deal}\n[DoubleDummyTricks")));
        assert!(annotated.ends_with("W  C  5"));

        let again = process_pbn(&annotated, false, false, false);
        assert_eq!(again, annotated);
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
        let result = process_pbn(pbn, false, false, false);
        assert_eq!(result.matches("[DoubleDummyTricks").count(), 1);
        assert_eq!(result.matches("[OptimumScore").count(), 0);
        assert_eq!(result.matches("[ParContract").count(), 0);
    }

    /// A few deals of varying shape, including one with a void, since voids are
    /// the case where the caches behave differently.
    const SAMPLE_PBN: [&str; 3] = [
        "N:62.JT765.AKJ5.Q3 KQ85.Q9.Q876.J75 J9743.K84.T2.K84 AT.A32.943.AT962",
        "N:Q7432.85.J983.63 J65.64.AKT5.AK98 AK98.AKQJ7.6.QJ7 T.T932.Q742.T542",
        "N:KJ86.KQ9.T3.JT76 QT53.JT74.87.A93 -.A83.AQJ642.K542 A9742.652.K95.Q8",
    ];

    fn sample_deals() -> Vec<Hands> {
        SAMPLE_PBN
            .iter()
            .filter_map(|d| Hands::from_pbn(d))
            .collect()
    }

    /// The threaded path must agree with the serial one cell for cell. It is the
    /// same search either way — a strain's four declarers stay together on one
    /// thread — so this is an equality, not an approximation.
    #[test]
    fn parallel_solve_matches_serial() {
        let deals = sample_deals();
        assert_eq!(deals.len(), 3);

        let serial: Vec<DdTable> = deals.iter().map(solve_deal).collect();

        for threads in [2, 4, 12] {
            let threaded = solve_deals_parallel(&deals, threads, false);
            assert_eq!(threaded, serial, "disagreement on {threads} threads");
        }
    }

    /// Results are placed by index, so they come back in the order the deals
    /// were given however the work was scheduled.
    #[test]
    fn parallel_solve_keeps_deal_order() {
        let deals = sample_deals();
        let forward = solve_deals_parallel(&deals, 8, false);

        let mut reversed_input = deals.clone();
        reversed_input.reverse();
        let mut reversed = solve_deals_parallel(&reversed_input, 8, false);
        reversed.reverse();

        assert_eq!(forward, reversed);
    }

    /// The table this binary assembles must be the library's own table, cell
    /// for cell. `TableSolver::solve_strain_hands` returns a column in
    /// `Direction::to_index` order and `place_column` places it by seat, so a
    /// transposition here would be silent — every cell would still be a real
    /// double-dummy result, just the wrong one's.
    #[test]
    fn solve_deal_matches_the_library_table() {
        for (hands, pbn) in sample_deals().iter().zip(SAMPLE_PBN) {
            let deal = bridge_types::Deal::from_pbn(pbn).expect("sample deal parses");
            let reference = bridge_solver::solve_dd_table(&deal);
            let ours = solve_deal(hands);
            for (declarer, strain, tricks) in ours.cells() {
                assert_eq!(
                    tricks,
                    reference.get(declarer, strain),
                    "{declarer:?} in {strain:?} of {pbn}"
                );
            }
        }
    }

    /// The deal strings this binary reads are parsed by `Hands::from_pbn`, and
    /// they stay that way: it accepts PBN that `bridge_types::Deal::from_pbn`
    /// does not, so routing the CLI through a `Deal` to reach `TableSolver`
    /// would have quietly stopped annotating boards it annotates today. That is
    /// why `TableSolver::solve_strain_hands` exists.
    #[test]
    fn the_cli_parser_accepts_more_than_the_typed_one() {
        for lenient in [
            // No leading seat: `Hands::from_pbn` defaults to North.
            "AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72",
            // A void written inside the suit rather than as the whole suit.
            "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.-9653.QJ72",
        ] {
            assert!(Hands::from_pbn(lenient).is_some(), "{lenient}");
            assert!(bridge_types::Deal::from_pbn(lenient).is_none(), "{lenient}");
        }
    }

    /// More threads than there is work to do must still terminate and be right.
    #[test]
    fn parallel_solve_with_more_threads_than_work() {
        let deals = sample_deals();
        let serial: Vec<DdTable> = deals.iter().map(solve_deal).collect();
        let threaded = solve_deals_parallel(&deals, 64, false);
        assert_eq!(threaded, serial);
    }
}
