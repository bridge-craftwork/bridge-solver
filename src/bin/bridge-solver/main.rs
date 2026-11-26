//! bridge-solver - Double-dummy solver for PBN files
//!
//! Reads PBN files and writes double-dummy analysis into them as Bridge
//! Composer compatible tags: `[DoubleDummyTricks]`, `[OptimumScore]`,
//! `[ParContract]` and `[OptimumResultTable]`, and with `--play` a
//! `[DoubleDummyPlay]` section for each recorded play. Which tags, on which
//! boards and where is `bridge_solver::annotate`'s; this binary reads and
//! writes the files and reports.
//!
//! Usage:
//!   bridge-solver -i <file.pbn> -o <file.pbn>   # one file to another
//!   bridge-solver -i <file.pbn>                 # one file to stdout
//!   bridge-solver -w -i <file.pbn> <dir> ...    # annotate in place, recursively

mod bench;

use bridge_encodings::pbn::PbnDocument;
use bridge_solver::annotate::{
    annotate_documents, AnnotateError, AnnotateOptions, DdPlayStyle, Progress, Stage,
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
    #[arg(
        short = 'i',
        long = "input",
        required_unless_present = "bench",
        num_args = 1..
    )]
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

    /// Also analyse the play, card by card: for each board with a [Play]
    /// section, write a [DoubleDummyPlay] section giving the double-dummy
    /// cost of every card that could have been played at each turn. A board
    /// that already has one keeps it unless --recalculate.
    #[arg(long = "play")]
    play: bool,

    /// With --play, spell every [DoubleDummyPlay] entry out rather than
    /// writing `*` for a turn with no wrong card and `*+` for a run of them to
    /// the end. Readers accept both.
    #[arg(long = "play-explicit", requires = "play")]
    play_explicit: bool,

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

    /// Time the solver instead of annotating files: solve the built-in
    /// benchmark deals (`quick`, 20 deals, or `full`, 200) across the threads
    /// and report tricks, nodes and time. The same deals as the benchmark
    /// page at bridge-craftwork.com/bridge-solver/bench, node for node.
    #[arg(
        long = "bench",
        value_name = "PRESET",
        num_args = 0..=1,
        default_missing_value = "full",
        conflicts_with_all = ["input", "output", "in_place"]
    )]
    bench: Option<String>,

    /// With --bench, print the results as JSON.
    #[arg(long = "json", requires = "bench")]
    json: bool,
}

fn main() {
    let args = Args::parse();

    if let Some(preset) = &args.bench {
        if let Err(e) = bench::run(preset, args.threads, args.json) {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return;
    }

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

    let options = AnnotateOptions {
        recalculate: args.recalculate,
        mark_verified: args.mark_verified,
        play: args.play,
        play_style: if args.play_explicit {
            DdPlayStyle::Explicit
        } else {
            DdPlayStyle::Compact
        },
        threads: args
            .threads
            .unwrap_or_else(bridge_solver::default_thread_count)
            .max(1),
    };

    // Read and index every input before solving any of it. The solve is one
    // batch across all of them, so a directory of one-deal files spreads over
    // the threads exactly as well as a single file of many deals does.
    let mut documents = Vec::with_capacity(files.len());
    for path in &files {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error reading input file '{}': {}", path.display(), e);
                std::process::exit(1);
            }
        };
        match PbnDocument::parse(&content) {
            Ok(doc) => documents.push(doc),
            Err(e) => {
                eprintln!("Error reading input file '{}': {}", path.display(), e);
                std::process::exit(1);
            }
        }
    }

    let verbose = args.verbose;
    let report = match annotate_documents(&mut documents, &options, &|p| {
        if verbose {
            report_progress(p, files.len(), options.threads);
        }
        true
    }) {
        Ok(report) => report,
        Err(AnnotateError::Pbn(e)) => {
            eprintln!("Error annotating: {e}");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: {e}; no files were written");
            std::process::exit(1);
        }
    };
    for skip in &report.skipped {
        eprintln!(
            "Skipping board {} in '{}': {}",
            skip.label,
            files[skip.file].display(),
            skip.reason
        );
    }
    if verbose {
        eprintln!(
            "{} table(s) and {} play analysis(es) written",
            report.tables, report.plays
        );
    }

    let mut changed = 0usize;
    for (path, doc) in files.iter().zip(&documents) {
        let result = doc.to_pbn();

        if args.in_place {
            // Unchanged files are left untouched so a re-run is a true no-op
            // and does not churn mtimes in a build.
            if !doc.is_modified() {
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
                    if verbose {
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

    if args.in_place && verbose {
        eprintln!("{changed} of {} file(s) updated", files.len());
    }
}

/// Say what a stage is about to do as it starts, and at each ten percent how
/// far it has got.
///
/// The decile test is true for exactly one value of `done`, so exactly one
/// thread prints each line however many are running and in whatever order they
/// finish.
fn report_progress(p: Progress, files: usize, threads: usize) {
    let unit = match p.stage {
        Stage::Tables => "strains",
        Stage::Play => "plays",
    };
    if p.done == 0 {
        match p.stage {
            Stage::Tables => eprintln!(
                "{} board(s) to analyse in {files} file(s), on up to {threads} thread(s)",
                p.total / 5
            ),
            Stage::Play if p.total > 0 => eprintln!("{} play(s) to analyse", p.total),
            Stage::Play => {}
        }
        return;
    }
    let (n, items) = (p.done, p.total);
    if items > 0 && n * 10 / items != (n - 1) * 10 / items {
        eprintln!("  {}% ({n} of {items} {unit})", n * 100 / items);
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
