//! solver-bench - performance harness for the double-dummy solver.
//!
//! Answers three questions, deliberately kept apart because they move for
//! different reasons and collapsing them into one headline hides regressions:
//!
//! * **Single-threaded cost.** How efficient the search itself is. One core
//!   doing one board's work. This is the stable number — a single-threaded run
//!   gets a full core even on a busy machine, so repeats land within a percent
//!   or two — and it is the one that matters for the browser, where extra
//!   cores may not be available at all.
//! * **Scaling.** How much of the machine we can actually use, measured as a
//!   thread sweep over *equal work per thread*. Never one board per thread:
//!   board difficulty spans an order of magnitude, so that measures load
//!   imbalance and reports a number that looks like scaling and is not.
//! * **Agreement.** Every board carries its double-dummy table, so a run that
//!   is fast and wrong says so.
//!
//! Run on demand, not in CI — a full sweep takes minutes and the numbers are
//! only comparable on an otherwise-idle machine.
//!
//! ```text
//! ./dev-build.sh --ci run --release --features bench --bin solver-bench -- run
//! ./dev-build.sh --ci run --release --features bench --bin solver-bench -- sweep
//! ./dev-build.sh --ci run --release --features bench --bin solver-bench -- verify
//! ./dev-build.sh --ci run --release --features bench --bin solver-bench -- compare A.json B.json
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[cfg(feature = "dds-reference")]
mod dds;

use bridge_solver::solve_dd_table;
use bridge_types::Deal;
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};

/// A run whose samples spread wider than this was measuring the machine, not
/// the code. dealer3's harness uses the same threshold for the same reason.
const SPREAD_WARN: f64 = 0.15;

/// Sweep configurations shorter than this are dominated by thread startup and
/// the final join, and the curve that comes out is noise rather than scaling.
const DEFAULT_MIN_SWEEP_SECONDS: f64 = 1.0;

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------

/// One benchmark board: the deal, and the answer it must produce.
#[derive(Debug, Clone, Deserialize)]
struct Board {
    board: usize,
    pbn: String,
    /// Seat-major N,S,E,W over strains NT,S,H,D,C, in hex.
    ddtricks: String,
    contract: String,
}

/// The committed benchmark corpus.
#[derive(Debug, Deserialize)]
struct Corpus {
    version: String,
    boards: Vec<Board>,
}

impl Corpus {
    /// Load the corpus, and parse every deal up front so a malformed board is
    /// a startup error rather than a mid-run panic.
    fn load(path: &Path) -> Result<(Self, Vec<Deal>), String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read corpus {}: {e}", path.display()))?;
        let corpus: Corpus =
            serde_json::from_str(&text).map_err(|e| format!("corpus is not valid JSON: {e}"))?;
        if corpus.boards.is_empty() {
            return Err("corpus contains no boards".into());
        }
        let deals = corpus
            .boards
            .iter()
            .map(|b| {
                Deal::from_pbn(&b.pbn)
                    .ok_or_else(|| format!("board {} has an unparseable deal", b.board))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((corpus, deals))
    }
}

/// Encode a solved table the way the corpus pins it.
fn encode_ddtricks(tricks: &[[u8; 5]; 4]) -> String {
    let mut out = String::with_capacity(20);
    // par.rs indexes [direction][strain] as N,E,S,W over C,D,H,S,NT; the
    // corpus is seat-major N,S,E,W over NT,S,H,D,C.
    for dir in [0usize, 2, 1, 3] {
        for strain in [4usize, 3, 2, 1, 0] {
            match std::char::from_digit(tricks[dir][strain] as u32, 16) {
                Some(c) => out.push(c),
                None => out.push('?'),
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------

/// Wall clock and CPU time for one measured run.
///
/// Both, always. Wall clock alone cannot tell "we used four cores well" from
/// "we used one core faster", and reading a threaded result without the CPU
/// figure beside it is how a parallelism win gets mistaken for a per-core one.
#[derive(Debug, Clone, Copy, Serialize)]
struct Sample {
    wall_ms: f64,
    cpu_ms: f64,
}

impl Sample {
    /// Cores busy: CPU divided by wall. 1.0 is single-threaded, 4.0 means four
    /// cores were saturated for the whole run.
    fn cores_busy(&self) -> f64 {
        if self.wall_ms > 0.0 {
            self.cpu_ms / self.wall_ms
        } else {
            0.0
        }
    }
}

/// CPU time consumed by this process so far, across all its threads.
#[cfg(unix)]
fn cpu_time() -> Duration {
    // SAFETY: `getrusage` writes a plain POD struct through the pointer we
    // give it and reads nothing else. The struct is fully initialised by the
    // zeroed value before the call, and the call cannot fail for RUSAGE_SELF.
    let usage = unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        usage
    };
    let secs = |t: libc::timeval| {
        Duration::from_secs(t.tv_sec as u64) + Duration::from_micros(t.tv_usec as u64)
    };
    secs(usage.ru_utime) + secs(usage.ru_stime)
}

/// CPU time is unavailable off unix; wall clock stands in, and "cores busy"
/// will read 1.0 whatever actually happened.
#[cfg(not(unix))]
fn cpu_time() -> Duration {
    Duration::ZERO
}

/// Time one closure, wall and CPU together.
fn measure<T>(f: impl FnOnce() -> T) -> (T, Sample) {
    let cpu0 = cpu_time();
    let wall0 = Instant::now();
    let out = f();
    let wall = wall0.elapsed();
    let cpu = cpu_time().saturating_sub(cpu0);
    let cpu_ms = if cfg!(unix) {
        cpu.as_secs_f64() * 1000.0
    } else {
        wall.as_secs_f64() * 1000.0
    };
    (
        out,
        Sample {
            wall_ms: wall.as_secs_f64() * 1000.0,
            cpu_ms,
        },
    )
}

/// Best-of-N, with the spread kept alongside.
///
/// The fastest run is the least contaminated by whatever else the machine was
/// doing, so it is the headline. The spread is what says whether to believe it.
#[derive(Debug, Clone, Serialize)]
struct Measured {
    best: Sample,
    median_wall_ms: f64,
    /// `(slowest - fastest) / fastest` over the samples.
    spread: f64,
    runs: usize,
}

impl Measured {
    fn from(samples: &mut [Sample]) -> Self {
        samples.sort_by(|a, b| a.wall_ms.total_cmp(&b.wall_ms));
        let best = samples[0];
        let worst = samples[samples.len() - 1];
        let median = samples[samples.len() / 2].wall_ms;
        Measured {
            best,
            median_wall_ms: median,
            spread: if best.wall_ms > 0.0 {
                (worst.wall_ms - best.wall_ms) / best.wall_ms
            } else {
                0.0
            },
            runs: samples.len(),
        }
    }

    /// Marker for a sample set too noisy to read.
    fn flag(&self) -> &'static str {
        if self.spread > SPREAD_WARN {
            " !"
        } else {
            ""
        }
    }
}

/// Repeat `f` `runs` times and reduce to a [`Measured`].
fn best_of(runs: usize, mut f: impl FnMut()) -> Measured {
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs.max(1) {
        let (_, s) = measure(&mut f);
        samples.push(s);
    }
    Measured::from(&mut samples)
}

// ---------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------

/// Everything one invocation measured, keyed to what it measured it on.
#[derive(Debug, Serialize)]
struct Results {
    corpus_version: String,
    revision: String,
    machine: String,
    logical_cpus: usize,
    runs: usize,
    /// Per board, single-threaded.
    boards: BTreeMap<String, Measured>,
    /// Thread count to equal-work scaling, when a sweep ran.
    sweep: Option<Sweep>,
}

/// A thread sweep over equal work.
#[derive(Debug, Serialize)]
struct Sweep {
    /// The board the sweep ran on, and how many solves each thread did.
    board: usize,
    solves_per_thread: usize,
    /// Thread count to measurement, in ascending order of thread count.
    points: Vec<SweepPoint>,
}

/// One configuration of the sweep.
#[derive(Debug, Serialize)]
struct SweepPoint {
    threads: usize,
    measured: Measured,
    /// Throughput relative to the single-threaded point.
    speedup: f64,
}

/// Short git description of the working tree, or `unknown` outside a checkout.
fn revision() -> String {
    Command::new("git")
        .args(["describe", "--always", "--dirty", "--tags"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

/// Human-readable CPU description, for the results header.
fn machine() -> String {
    #[cfg(target_os = "macos")]
    let probe = Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output();
    #[cfg(not(target_os = "macos"))]
    let probe = Command::new("uname").arg("-m").output();

    probe
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

/// Logical CPU count, defaulting to 1 where it cannot be determined.
fn logical_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "solver-bench")]
#[command(about = "Performance harness for the double-dummy solver")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Measure each board single-threaded, and optionally sweep thread counts.
    Run(RunArgs),
    /// Sweep thread counts over equal work, without the per-board pass.
    Sweep(RunArgs),
    /// Check every board still solves to its pinned double-dummy table.
    Verify(CorpusArgs),
    /// Measure DDS on the same corpus, with the same timing code.
    #[cfg(feature = "dds-reference")]
    Reference(ReferenceArgs),
    /// Compare two results files.
    Compare {
        /// The earlier results file (the baseline).
        baseline: PathBuf,
        /// The later results file.
        current: PathBuf,
    },
}

#[derive(Args, Clone)]
struct CorpusArgs {
    /// Corpus file to read.
    #[arg(long, default_value = "bench/corpus.json")]
    corpus: PathBuf,
}

/// Options for the DDS reference comparison.
#[cfg(feature = "dds-reference")]
#[derive(Args, Clone)]
struct ReferenceArgs {
    #[command(flatten)]
    corpus: CorpusArgs,

    /// Repeats per measurement; the fastest is reported.
    #[arg(long, default_value_t = 3)]
    runs: usize,

    /// Thread count to measure at. Both solvers get the same one.
    ///
    /// Exactly one value: DDS cannot survive a second `SetResources`, so
    /// measuring a curve means one process per thread count.
    #[arg(long, value_delimiter = ',', default_values_t = [1usize])]
    dds_threads: Vec<usize>,

    /// DDS threading backend: 3 = GCD, 5 = STL. GCD is DDS's own default on
    /// macOS and dispatches at background priority, which Apple Silicon
    /// confines to the efficiency cores; STL is the like-for-like choice.
    #[arg(long, default_value_t = 5)]
    dds_threading: usize,

    /// Solve these deals instead of generated ones: a file of PBN deal
    /// strings, one per line. Lets an external solver be measured on exactly
    /// the same deals.
    #[arg(long)]
    throughput_pbn: Option<std::path::PathBuf>,

    /// Distinct deals to generate for the throughput measurement. Ten boards
    /// is not enough work to keep twelve threads busy, and the corpus cannot
    /// simply be repeated -- see `generated_deals`.
    #[arg(long, default_value_t = 120)]
    throughput_deals: usize,
}

#[derive(Args, Clone)]
struct RunArgs {
    #[command(flatten)]
    corpus: CorpusArgs,

    /// Repeats per measurement; the fastest is reported.
    #[arg(long, default_value_t = 3)]
    runs: usize,

    /// Measure only the median-cost board. Deterministic, so two quick runs
    /// are comparable with each other.
    #[arg(long)]
    quick: bool,

    /// Skip the thread sweep (`run` only).
    #[arg(long)]
    no_sweep: bool,

    /// Highest thread count to sweep. Defaults to the logical CPU count.
    #[arg(long)]
    max_threads: Option<usize>,

    /// Grow the sweep workload until even the fastest configuration runs at
    /// least this long. Short sweeps measure thread startup, not scaling.
    #[arg(long, default_value_t = DEFAULT_MIN_SWEEP_SECONDS)]
    min_sweep_seconds: f64,

    /// Write results JSON here. Defaults to `bench/results/<revision>.json`.
    #[arg(long)]
    json: Option<PathBuf>,
}

/// Solve every board once, single-threaded, and report per-board cost.
fn per_board(
    corpus: &Corpus,
    deals: &[Deal],
    which: &[usize],
    runs: usize,
) -> BTreeMap<String, Measured> {
    let mut out = BTreeMap::new();
    println!("board  contract      wall ms     cpu ms   median   spread  runs");
    println!("{}", "-".repeat(68));
    for &i in which {
        let deal = &deals[i];
        let m = best_of(runs, || {
            let _ = solve_dd_table(deal);
        });
        println!(
            "{:>5}  {:<8}  {:>10.1} {:>10.1} {:>8.1} {:>7.1}% {:>5}{}",
            corpus.boards[i].board,
            corpus.boards[i].contract,
            m.best.wall_ms,
            m.best.cpu_ms,
            m.median_wall_ms,
            m.spread * 100.0,
            m.runs,
            m.flag(),
        );
        out.insert(corpus.boards[i].board.to_string(), m);
    }
    out
}

/// Sweep thread counts over *equal work per thread*.
///
/// Every thread solves the same board the same number of times, so the only
/// thing the ratio can reflect is how well concurrent solves coexist. A sweep
/// that hands one board to each thread would instead measure the spread of
/// board difficulty, which here is more than an order of magnitude.
fn sweep(deal: &Deal, board: usize, args: &RunArgs, max_threads: usize) -> Sweep {
    // Size the workload against the *widest* configuration, which is the
    // fastest in wall-clock terms and therefore the one at risk of being all
    // startup. Everything narrower then runs at least as long.
    let mut solves = 1usize;
    loop {
        let (_, s) = measure(|| run_threads(deal, max_threads, solves));
        if s.wall_ms / 1000.0 >= args.min_sweep_seconds || solves >= 4096 {
            break;
        }
        let ratio = (args.min_sweep_seconds * 1000.0 / s.wall_ms.max(0.01)).ceil() as usize;
        solves = (solves * ratio.max(2)).min(4096);
    }

    println!(
        "\nthread sweep: board {board}, {solves} solves per thread, best of {}",
        args.runs
    );
    println!("threads      wall ms     cpu ms   cores   speedup   spread");
    println!("{}", "-".repeat(60));

    let mut points: Vec<SweepPoint> = Vec::new();
    let mut single: Option<f64> = None;
    for threads in 1..=max_threads {
        let m = best_of(args.runs, || run_threads(deal, threads, solves));
        // Throughput per unit time: N threads each doing `solves` solves.
        let work = threads as f64;
        let rate = work / m.best.wall_ms;
        let base = *single.get_or_insert(rate);
        let speedup = rate / base;
        println!(
            "{:>7} {:>12.1} {:>10.1} {:>7.2} {:>9.2}x {:>7.1}%{}",
            threads,
            m.best.wall_ms,
            m.best.cpu_ms,
            m.best.cores_busy(),
            speedup,
            m.spread * 100.0,
            m.flag(),
        );
        points.push(SweepPoint {
            threads,
            measured: m,
            speedup,
        });
    }

    report_curve(&points);

    Sweep {
        board,
        solves_per_thread: solves,
        points,
    }
}

/// Run `threads` threads, each solving `solves` times.
fn run_threads(deal: &Deal, threads: usize, solves: usize) {
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                for _ in 0..solves {
                    let _ = solve_dd_table(deal);
                }
            });
        }
    });
}

/// Say what the curve's shape means.
///
/// A curve that flattens past the physical core count is ordinary saturation.
/// A curve that turns *down* — more threads, less throughput — is contention,
/// and that is a bug rather than a limit.
fn report_curve(points: &[SweepPoint]) {
    let Some(peak) = points.iter().max_by(|a, b| a.speedup.total_cmp(&b.speedup)) else {
        return;
    };
    let last = &points[points.len() - 1];
    if peak.threads == last.threads {
        println!("\npeak {:.2}x at {} threads", peak.speedup, peak.threads);
    } else {
        println!(
            "\npeak {:.2}x at {} threads, falling to {:.2}x at {}",
            peak.speedup, peak.threads, last.speedup, last.threads
        );
    }
    let regression = (peak.speedup - last.speedup) / peak.speedup;
    if regression > 0.10 {
        println!(
            "  REGRESSION: throughput falls {:.0}% from its peak by {} threads.\n\
               More workers doing less work is contention on shared state, not\n\
               saturation. Re-run with a different workload size: if the turnover\n\
               moves, the cost is per-run; if it stays put, it is contention.",
            regression * 100.0,
            last.threads
        );
    } else if last.speedup < peak.speedup * 0.95 {
        println!(
            "  Plateau past {} threads — ordinary saturation.",
            peak.threads
        );
    }
}

/// Check every board against its pinned table.
fn verify(corpus: &Corpus, deals: &[Deal]) -> bool {
    let mut ok = true;
    println!("board  expected              computed              result");
    println!("{}", "-".repeat(62));
    for (board, deal) in corpus.boards.iter().zip(deals) {
        let got = encode_ddtricks(&solve_dd_table(deal).tricks);
        let good = got == board.ddtricks;
        ok &= good;
        println!(
            "{:>5}  {}  {}  {}",
            board.board,
            board.ddtricks,
            got,
            if good { "ok" } else { "MISMATCH" }
        );
    }
    println!(
        "\n{} of {} boards agree with the corpus.",
        corpus
            .boards
            .iter()
            .zip(deals)
            .filter(|(b, d)| { encode_ddtricks(&solve_dd_table(d).tricks) == b.ddtricks })
            .count(),
        corpus.boards.len()
    );
    ok
}

/// Results as re-read for a comparison. Deliberately a separate, permissive
/// shape: a baseline written by an older revision should still be readable.
#[derive(Debug, Deserialize)]
struct StoredResults {
    corpus_version: String,
    revision: String,
    machine: String,
    boards: BTreeMap<String, StoredMeasured>,
    sweep: Option<StoredSweep>,
}

#[derive(Debug, Deserialize)]
struct StoredMeasured {
    best: StoredSample,
    spread: f64,
}

#[derive(Debug, Deserialize)]
struct StoredSample {
    wall_ms: f64,
    cpu_ms: f64,
}

#[derive(Debug, Deserialize)]
struct StoredSweep {
    solves_per_thread: usize,
    points: Vec<StoredSweepPoint>,
}

#[derive(Debug, Deserialize)]
struct StoredSweepPoint {
    threads: usize,
    speedup: f64,
}

/// Compare two result files, board by board and point by point.
fn compare(baseline: &Path, current: &Path) -> Result<(), String> {
    let read = |p: &Path| -> Result<StoredResults, String> {
        let text =
            std::fs::read_to_string(p).map_err(|e| format!("cannot read {}: {e}", p.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{} is not results JSON: {e}", p.display()))
    };
    let (a, b) = (read(baseline)?, read(current)?);

    println!("baseline : {} on {}", a.revision, a.machine);
    println!("current  : {} on {}", b.revision, b.machine);
    if a.corpus_version != b.corpus_version {
        println!(
            "\nWARNING: corpus differs ({} vs {}). The two runs measured different\n\
             work, and the per-board numbers below are not comparable.",
            a.corpus_version, b.corpus_version
        );
    }
    if a.machine != b.machine {
        println!("\nWARNING: different machines. Read the ratios, not the absolute times.");
    }

    println!("\nboard      base ms      now ms    change   base cpu    now cpu    note");
    println!("{}", "-".repeat(78));
    let mut ratios = Vec::new();
    let mut cpu_ratios = Vec::new();
    for (board, base) in &a.boards {
        let Some(now) = b.boards.get(board) else {
            println!(
                "{board:>5}  {:>11.1}           -         -          -          -    (dropped)",
                base.best.wall_ms
            );
            continue;
        };
        let change = now.best.wall_ms / base.best.wall_ms;
        ratios.push(change);
        if base.best.cpu_ms > 0.0 {
            cpu_ratios.push(now.best.cpu_ms / base.best.cpu_ms);
        }
        // A change smaller than either run's own spread is not a change.
        let noise = base.spread.max(now.spread);
        let note = if (change - 1.0).abs() <= noise {
            "within noise"
        } else if change < 1.0 {
            "faster"
        } else {
            "SLOWER"
        };
        println!(
            "{board:>5}  {:>11.1} {:>11.1} {:>8.1}% {:>10.1} {:>10.1}    {note}",
            base.best.wall_ms,
            now.best.wall_ms,
            (change - 1.0) * 100.0,
            base.best.cpu_ms,
            now.best.cpu_ms,
        );
    }
    if !ratios.is_empty() {
        // Geometric mean: these are ratios, and ratios do not average.
        let logs: f64 = ratios.iter().map(|r| r.ln()).sum();
        let geo = (logs / ratios.len() as f64).exp();
        println!(
            "\noverall wall: {:.3}x ({:+.1}%), geometric mean over {} boards",
            geo,
            (geo - 1.0) * 100.0,
            ratios.len()
        );
        if !cpu_ratios.is_empty() {
            let logs: f64 = cpu_ratios.iter().map(|r| r.ln()).sum();
            let geo_cpu = (logs / cpu_ratios.len() as f64).exp();
            println!(
                "overall cpu : {:.3}x ({:+.1}%) — this is the per-core figure, and it moves\n\
                 {:14}only when the search itself got cheaper or dearer.",
                geo_cpu,
                (geo_cpu - 1.0) * 100.0,
                ""
            );
        }
    }

    if let (Some(sa), Some(sb)) = (&a.sweep, &b.sweep) {
        if sa.solves_per_thread != sb.solves_per_thread {
            println!(
                "\nNote: the sweeps sized their workload differently ({} vs {} solves per\n\
                 thread), because each run grows it until the widest configuration is long\n\
                 enough to measure. The speedup curves below are still comparable — each is\n\
                 normalised to its own single-threaded point — but absolute sweep times are\n\
                 not.",
                sa.solves_per_thread, sb.solves_per_thread
            );
        }
        println!("\nthreads   base speedup   now speedup       change");
        println!("{}", "-".repeat(52));
        for pa in &sa.points {
            if let Some(pb) = sb.points.iter().find(|p| p.threads == pa.threads) {
                println!(
                    "{:>7} {:>14.2}x {:>13.2}x {:>11.2}x",
                    pa.threads,
                    pa.speedup,
                    pb.speedup,
                    pb.speedup / pa.speedup
                );
            }
        }
    }
    Ok(())
}

/// Measure DDS beside this solver, board for board.
///
/// Both are called from this process by the same timing code on the same
/// corpus, which is a cleaner comparison than running two programs and
/// subtracting their startup.
///
/// The two numbers are not the same *kind* of number, and the output says so.
/// DDS parallelises the twenty entries *within* one table; this solver has no
/// intra-solve parallelism and instead scales across independent deals. So
/// DDS's threaded column is its best time for one board, while ours is a
/// single core's — and a caller with many boards to solve should compare
/// DDS's threaded figure against our single-threaded one divided by the
/// scaling in `sweep`.
#[cfg(feature = "dds-reference")]
fn reference(args: &ReferenceArgs) -> Result<(), String> {
    let (corpus, deals) = Corpus::load(&args.corpus.corpus)?;

    println!(
        "corpus   : {} ({} boards)",
        corpus.version,
        corpus.boards.len()
    );
    println!("revision : {}", revision());
    println!("machine  : {} ({} logical)", machine(), logical_cpus());
    println!("runs     : best of {}\n", args.runs);

    // DDS allocates its per-thread working memory when resources are set, and
    // faults with "Memory::GetPtr" if a solve is attempted before that. It is
    // process-global state, so it is set here once before any call.
    //
    // Set it to the *first* configuration we will measure rather than to 1, so
    // that a single-valued `--dds-threads` needs only one `SetResources` for
    // the whole run. DDS cannot survive a second one -- it tears its thread
    // memory down before rebuilding it, and the rebuild does not always
    // happen -- so measuring several thread counts means one process each.
    if args.dds_threads.len() > 1 {
        return Err(
            "--dds-threads takes one value. DDS tears its thread memory down on a \n\
             second SetResources and does not always rebuild it, so a thread-count \n\
             curve means one process per point."
                .into(),
        );
    }
    let threads = args.dds_threads.first().copied().unwrap_or(1);
    dds::set_backend(args.dds_threading as i32)?;
    dds::set_threads(threads);
    let (backend, dds_cores, dds_threads) = dds::info();
    println!(
        "dds      : {backend} threading, {dds_cores} cores seen, {dds_threads} threads made\n"
    );

    // Agreement first: a timing comparison between two solvers that disagree
    // is meaningless.
    // Solved as one batch, so this doubles as the check on the batched result
    // indexing that the throughput measurement below depends on.
    let all_pbns: Vec<&str> = corpus.boards.iter().map(|b| b.pbn.as_str()).collect();
    let dds_tables = dds::solve_tables(&all_pbns)?;

    let mut disagreements = 0;
    for ((board, deal), dds_table) in corpus.boards.iter().zip(&deals).zip(&dds_tables) {
        let ours = encode_ddtricks(&solve_dd_table(deal).tricks);
        let theirs = encode_ddtricks(dds_table);
        if ours != theirs {
            println!(
                "board {}: DISAGREEMENT\n  ours {ours}\n  dds  {theirs}",
                board.board
            );
            disagreements += 1;
        }
    }
    if disagreements > 0 {
        return Err(format!(
            "{disagreements} board(s) disagree; the timings below would be meaningless"
        ));
    }
    println!("all {} boards agree with DDS\n", corpus.boards.len());

    let mut header = format!("{:>5}  {:>10}", "board", "ours 1t");
    for t in &args.dds_threads {
        header.push_str(&format!("  {:>8}", format!("dds {t}t")));
    }
    header.push_str(&format!("  {:>9}", "ours/dds1"));
    println!("{header}");
    println!("{}", "-".repeat(header.len()));

    let mut ratios = Vec::new();
    for (board, deal) in corpus.boards.iter().zip(&deals) {
        let ours = best_of(args.runs, || {
            let _ = solve_dd_table(deal);
        });
        let mut line = format!("{:>5}  {:>10.1}", board.board, ours.best.wall_ms);
        let mut first: Option<f64> = None;
        for &threads in &args.dds_threads {
            dds::set_threads(threads);
            let mut err = None;
            let m = best_of(args.runs, || {
                if let Err(e) = dds::solve_table(&board.pbn) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            first.get_or_insert(m.best.wall_ms);
            line.push_str(&format!("  {:>8.1}", m.best.wall_ms));
        }
        if let Some(dds1) = first {
            let ratio = ours.best.wall_ms / dds1;
            ratios.push(ratio);
            line.push_str(&format!("  {ratio:>8.2}x"));
        }
        println!("{line}");
    }

    if !ratios.is_empty() {
        let logs: f64 = ratios.iter().map(|r| r.ln()).sum();
        let geo = (logs / ratios.len() as f64).exp();
        println!(
            "\nPer core we are {geo:.2}x DDS's cost, geometric mean over {} boards.\n\
             That figure is untouched by any threading work — it moves only when\n\
             the search itself gets cheaper.",
            ratios.len()
        );
    }

    throughput(args, threads)
}

/// Deal-level throughput: the whole corpus, both solvers, at the same thread
/// count, over one shared work list.
///
/// This is the comparison someone choosing a solver for their own machine
/// actually cares about, and it is the one the per-board table above cannot
/// answer. It is only honest because DDS gets the deals in a batch: given one
/// deal at a time it has five work items to spread over N threads and the
/// result measures load imbalance rather than scaling.
///
/// Each solver parallelises however it likes. Ours takes deals off a shared
/// cursor, which is work-stealing in all but name and so tolerates the 18x
/// spread in board cost. DDS is handed the list in chunks of forty and
/// schedules the (deal, strain) pairs itself.
#[cfg(feature = "dds-reference")]
fn throughput(args: &ReferenceArgs, threads: usize) -> Result<(), String> {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let generated = match &args.throughput_pbn {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?;
            let mut v = Vec::new();
            for (i, line) in text.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let deal = Deal::from_pbn(line)
                    .ok_or_else(|| format!("{}:{}: unparseable deal", path.display(), i + 1))?;
                v.push((line.to_string(), deal));
            }
            if v.is_empty() {
                return Err(format!("{} held no deals", path.display()));
            }
            v
        }
        None => generated_deals(args.throughput_deals.max(1))?,
    };
    let pbns: Vec<&str> = generated.iter().map(|(p, _)| p.as_str()).collect();
    let work: Vec<&Deal> = generated.iter().map(|(_, d)| d).collect();

    // Check a sample before timing. A generator that emitted well-formed but
    // wrongly ordered deals would still parse, still solve, and still produce
    // plausible timings -- of a workload that is not the one being reported.
    let sample = work.len().min(5);
    for (i, (pbn, deal)) in generated.iter().take(sample).enumerate() {
        let ours = encode_ddtricks(&solve_dd_table(deal).tricks);
        let theirs = encode_ddtricks(&dds::solve_table(pbn)?);
        if ours != theirs {
            return Err(format!(
                "generated deal {i} disagrees, so the throughput workload is not \n\
                 the same work for both solvers\n  ours {ours}\n  dds  {theirs}\n  {pbn}"
            ));
        }
    }

    println!(
        "\ndeal-level throughput: {} deals ({sample} checked), \
         {threads} thread(s), best of {}",
        work.len(),
        args.runs
    );

    let ours = best_of(args.runs, || {
        let cursor = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| loop {
                    let i = cursor.fetch_add(1, Ordering::Relaxed);
                    let Some(deal) = work.get(i) else { break };
                    let _ = solve_dd_table(deal);
                });
            }
        });
    });

    let mut err = None;
    let dds = best_of(args.runs, || {
        if let Err(e) = dds::solve_tables(&pbns) {
            err = Some(e);
        }
    });
    if let Some(e) = err {
        return Err(e);
    }

    println!(
        "{:>8}  {:>10}  {:>10}  {:>7}  {:>11}",
        "", "wall ms", "cpu ms", "cores", "deals/sec"
    );
    for (name, m) in [("ours", &ours), ("dds", &dds)] {
        println!(
            "{:>8}  {:>10.1}  {:>10.1}  {:>7.2}  {:>11.1}",
            name,
            m.best.wall_ms,
            m.best.cpu_ms,
            m.best.cpu_ms / m.best.wall_ms,
            work.len() as f64 * 1000.0 / m.best.wall_ms,
        );
    }
    let ratio = ours.best.wall_ms / dds.best.wall_ms;
    println!("\nOn {threads} thread(s) we take {ratio:.2}x DDS's wall clock for the same deals.");
    Ok(())
}

/// Deterministic pseudo-random deals for the throughput measurement.
///
/// The corpus cannot be used for this, and neither can repeats of it. DDS
/// de-duplicates identical deals inside a batch -- `DetectCalcDuplicates` and
/// `CopyCalcSingle` -- and copies a result rather than solving again. Eight
/// copies of the ten-board corpus is eighty solves for us and ten for DDS,
/// which measured as a 4.7x win for DDS that was entirely an artefact of the
/// workload. Distinct deals are the only honest comparison; generating them
/// from a fixed seed keeps runs comparable without committing a large fixture.
///
/// Random deals are also the better workload here on their own merits: the
/// corpus was curated to span 16 ms to 300 ms, which is what makes it a good
/// per-board fixture and a poor model of a real file.
#[cfg(feature = "dds-reference")]
fn generated_deals(count: usize) -> Result<Vec<(String, Deal)>, String> {
    // Index 0 is the ace, so sorting ascending yields PBN's descending ranks.
    const RANK_CHARS: [u8; 13] = *b"AKQJT98765432";

    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    let mut rand = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut deck: [usize; 52] = std::array::from_fn(|i| i);
        for i in (1..52).rev() {
            let j = (rand() % (i as u64 + 1)) as usize;
            deck.swap(i, j);
        }

        // Thirteen cards per seat in PBN's N,E,S,W order; card = suit * 13 +
        // rank, with suit 0 spades so the suit loop emits S.H.D.C as PBN wants.
        let mut pbn = String::from("N:");
        for seat in 0..4 {
            if seat > 0 {
                pbn.push(' ');
            }
            let hand = &deck[seat * 13..seat * 13 + 13];
            for suit in 0..4 {
                if suit > 0 {
                    pbn.push('.');
                }
                let mut ranks: Vec<usize> = hand
                    .iter()
                    .filter(|c| *c / 13 == suit)
                    .map(|c| c % 13)
                    .collect();
                ranks.sort_unstable();
                for r in ranks {
                    pbn.push(RANK_CHARS[r] as char);
                }
            }
        }

        let deal =
            Deal::from_pbn(&pbn).ok_or_else(|| format!("generated an unparseable deal: {pbn}"))?;
        out.push((pbn, deal));
    }
    Ok(out)
}

/// Index of the median-cost board, chosen deterministically so that two
/// `--quick` runs measure the same thing.
fn median_board(deals: &[Deal]) -> usize {
    let mut costs: Vec<(usize, f64)> = deals
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let (_, s) = measure(|| {
                let _ = solve_dd_table(d);
            });
            (i, s.wall_ms)
        })
        .collect();
    costs.sort_by(|a, b| a.1.total_cmp(&b.1));
    costs[costs.len() / 2].0
}

fn run(args: &RunArgs, do_sweep: bool) -> Result<(), String> {
    let (corpus, deals) = Corpus::load(&args.corpus.corpus)?;
    let cpus = logical_cpus();
    let max_threads = args.max_threads.unwrap_or(cpus).max(1);

    println!(
        "corpus   : {} ({} boards)",
        corpus.version,
        corpus.boards.len()
    );
    println!("revision : {}", revision());
    println!("machine  : {} ({cpus} logical)", machine());
    println!("runs     : best of {}\n", args.runs);

    let chosen = if args.quick {
        vec![median_board(&deals)]
    } else {
        (0..deals.len()).collect()
    };

    let boards = if do_sweep && args.quick {
        BTreeMap::new()
    } else {
        per_board(&corpus, &deals, &chosen, args.runs)
    };

    let sweep_result = if do_sweep {
        let i = if args.quick {
            chosen[0]
        } else {
            median_board(&deals)
        };
        Some(sweep(&deals[i], corpus.boards[i].board, args, max_threads))
    } else {
        None
    };

    let results = Results {
        corpus_version: corpus.version.clone(),
        revision: revision(),
        machine: machine(),
        logical_cpus: cpus,
        runs: args.runs,
        boards,
        sweep: sweep_result,
    };

    let path = args
        .json
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("bench/results/{}.json", results.revision)));
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(&results)
        .map_err(|e| format!("cannot serialise results: {e}"))?;
    std::fs::write(&path, json + "\n")
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    println!("\nwrote {}", path.display());
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let outcome = match &cli.command {
        Cmd::Run(args) => run(args, !args.no_sweep),
        Cmd::Sweep(args) => run(args, true),
        Cmd::Verify(args) => Corpus::load(&args.corpus).and_then(|(c, d)| {
            if verify(&c, &d) {
                Ok(())
            } else {
                Err("corpus disagreement: the solver changed an answer".into())
            }
        }),
        #[cfg(feature = "dds-reference")]
        Cmd::Reference(args) => reference(args),
        Cmd::Compare { baseline, current } => compare(baseline, current),
    };
    if let Err(e) = outcome {
        eprintln!("solver-bench: {e}");
        std::process::exit(1);
    }
}
