//! Minimal FFI onto DDS 2.9, for the reference comparison.
//!
//! Deliberately narrow: `SetResources` and `CalcAllTablesPBN`, which is the
//! entry point that computes a whole twenty-entry table and is the one DDS
//! parallelises. `CalcDDtablePBN` computes the same table on one thread, so
//! benchmarking against it would flatter us.
//!
//! Linking is arranged by `build.rs` and is only attempted under the
//! `dds-reference` feature, so a normal build never needs DDS present.

use std::ffi::c_char;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicIsize, Ordering};

const DDS_STRAINS: usize = 5;
const DDS_HANDS: usize = 4;
const MAXNOOFTABLES: usize = 40;

/// Success, from DDS's `RETURN_NO_FAULT`.
const RETURN_NO_FAULT: c_int = 1;

/// One deal, as a PBN string in DDS's fixed-width buffer.
#[repr(C)]
#[derive(Clone, Copy)]
struct DdTableDealPbn {
    cards: [c_char; 80],
}

#[repr(C)]
struct DdTableDealsPbn {
    no_of_tables: c_int,
    deals: [DdTableDealPbn; MAXNOOFTABLES * DDS_STRAINS],
}

/// Tricks indexed `[strain][hand]`, strains S,H,D,C,NT and hands N,E,S,W.
#[repr(C)]
#[derive(Clone, Copy)]
struct DdTableResults {
    res_table: [[c_int; DDS_HANDS]; DDS_STRAINS],
}

#[repr(C)]
struct DdTablesRes {
    no_of_boards: c_int,
    results: [DdTableResults; MAXNOOFTABLES * DDS_STRAINS],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ParResults {
    par_score: [[c_char; 16]; 2],
    par_contracts_string: [[c_char; 128]; 2],
}

#[repr(C)]
struct AllParResults {
    presults: [ParResults; MAXNOOFTABLES],
}

/// DDS's own account of how it configured itself. Only the tail is of
/// interest here; the leading fields are version and platform strings.
#[repr(C)]
struct DdsInfo {
    major: c_int,
    minor: c_int,
    patch: c_int,
    version_string: [c_char; 10],
    system: c_int,
    num_bits: c_int,
    compiler: c_int,
    constructor: c_int,
    num_cores: c_int,
    threading: c_int,
    no_of_threads: c_int,
    thread_sizes: [c_char; 128],
    system_string: [c_char; 1024],
}

extern "C" {
    fn GetDDSInfo(info: *mut DdsInfo);
    fn SetThreading(code: c_int) -> c_int;
    fn SetResources(max_memory_mb: c_int, max_threads: c_int);
    fn CalcAllTablesPBN(
        dealsp: *mut DdTableDealsPbn,
        mode: c_int,
        trump_filter: *mut c_int,
        resp: *mut DdTablesRes,
        presp: *mut AllParResults,
    ) -> c_int;
}

/// Set DDS's thread count. `0` lets DDS choose.
///
/// Process-global in DDS, so call it once per configuration and never from
/// two threads at a time.
pub fn set_threads(threads: usize) {
    // Re-setting the same value is not a no-op inside DDS, and not a harmless
    // one: `SetResources` tears the thread memory down with
    // `memory.Resize(0, ...)` before building it up again, and on this build
    // the rebuild does not always happen -- the next solve then hits
    // `Memory::GetPtr: 0 vs. 0` and DDS calls `exit(1)`, taking the harness
    // with it. Skipping the redundant call avoids the window entirely.
    static LAST: AtomicIsize = AtomicIsize::new(-1);
    if LAST.swap(threads as isize, Ordering::Relaxed) == threads as isize {
        return;
    }
    // SAFETY: `SetResources` takes two integers by value and stores them in
    // DDS's own globals. It allocates and cannot fail; there is no pointer or
    // lifetime involved.
    unsafe { SetResources(0, threads as c_int) }
}

/// The most deals one `CalcAllTablesPBN` call accepts.
///
/// DDS checks `count * noOfTables <= MAXNOOFTABLES * DDS_STRAINS`, where
/// `count` is the number of strains actually requested. Wanting all five caps
/// the batch at forty; a single-strain filter would allow two hundred.
pub const MAX_TABLES_PER_CALL: usize = MAXNOOFTABLES;

/// Solve one deal's full twenty-entry table.
///
/// Returns the table in this crate's own layout — `[direction][strain]` over
/// N,E,S,W and C,D,H,S,NT — so it can be compared with `DdTricks` directly.
pub fn solve_table(pbn: &str) -> Result<[[u8; 5]; 4], String> {
    Ok(solve_tables(&[pbn])?[0])
}

/// Solve a batch of deals' tables in as few DDS calls as possible.
///
/// The batch size is not an efficiency detail, it is what decides how much
/// work DDS's threads have to share. `CalcAllTables` flattens the request into
/// one work item per (deal, strain) pair and hands the flat list to its
/// scheduler, so a single deal offers five items however many threads are
/// configured — most of them idle, and the wall time set by the slowest
/// strain. Forty deals offer two hundred. Anything measuring DDS's threading
/// on one deal at a time is measuring load imbalance.
pub fn solve_tables(pbns: &[&str]) -> Result<Vec<[[u8; 5]; 4]>, String> {
    let mut out = Vec::with_capacity(pbns.len());
    for chunk in pbns.chunks(MAX_TABLES_PER_CALL) {
        solve_chunk(chunk, &mut out)?;
    }
    Ok(out)
}

fn solve_chunk(pbns: &[&str], out: &mut Vec<[[u8; 5]; 4]>) -> Result<(), String> {
    // These structs run to hundreds of kilobytes, so they are boxed rather
    // than built on the stack.
    let mut deals: Box<DdTableDealsPbn> = Box::new(unsafe { std::mem::zeroed() });
    let mut res: Box<DdTablesRes> = Box::new(unsafe { std::mem::zeroed() });
    let mut par: Box<AllParResults> = Box::new(unsafe { std::mem::zeroed() });

    for (slot, pbn) in deals.deals.iter_mut().zip(pbns) {
        // DDS reads a fixed 80-byte buffer and wants a NUL, so refuse anything
        // that would not round-trip rather than silently truncating a deal.
        if pbn.len() >= 80 {
            return Err(format!("deal string is {} bytes, DDS allows 79", pbn.len()));
        }
        if pbn.contains('\0') {
            return Err("deal string contains a NUL".into());
        }
        for (byte_slot, byte) in slot.cards.iter_mut().zip(pbn.bytes()) {
            *byte_slot = byte as c_char;
        }
    }
    deals.no_of_tables = pbns.len() as c_int;

    // 0 means "compute this strain"; all five are wanted.
    let mut trump_filter = [0 as c_int; DDS_STRAINS];

    // SAFETY: every pointer is to a live, fully zeroed, correctly laid out
    // struct that outlives the call, and `trump_filter` is the five-element
    // array the signature requires. DDS writes only through `resp` and
    // `presp`, `no_of_tables` is within the array bounds because the caller
    // chunked at `MAX_TABLES_PER_CALL`, and each deal buffer is
    // NUL-terminated because it was zeroed and the string is shorter than it.
    let rc = unsafe {
        CalcAllTablesPBN(
            deals.as_mut(),
            -1,
            trump_filter.as_mut_ptr(),
            res.as_mut(),
            par.as_mut(),
        )
    };
    if rc != RETURN_NO_FAULT {
        return Err(format!("DDS returned error {rc}"));
    }

    // DDS gives [strain][hand] over S,H,D,C,NT and N,E,S,W. This crate wants
    // [direction][strain] over N,E,S,W and C,D,H,S,NT.
    const DDS_STRAIN_FOR: [usize; 5] = [3, 2, 1, 0, 4]; // C,D,H,S,NT -> DDS index
    for table in 0..pbns.len() {
        let mut one = [[0u8; 5]; 4];
        for (hand, row) in one.iter_mut().enumerate() {
            for (strain, cell) in row.iter_mut().enumerate() {
                let tricks = res.results[table].res_table[DDS_STRAIN_FOR[strain]][hand];
                *cell = u8::try_from(tricks)
                    .map_err(|_| format!("DDS returned {tricks} tricks, which is not 0..=13"))?;
            }
        }
        out.push(one);
    }
    Ok(())
}

/// What DDS says it configured: threading backend, cores seen, threads made.
///
/// Worth asking rather than assuming. `SetResources` derives its thread count
/// from a memory probe and a core count, and silently gives you fewer threads
/// than you requested when either comes out low -- so a scaling curve measured
/// without checking this can be a property of DDS's configuration rather than
/// of its scaling.
pub fn info() -> (String, i32, i32) {
    // SAFETY: `GetDDSInfo` fills a caller-owned struct by pointer and reads
    // nothing from it. The struct is zeroed, correctly laid out, and lives
    // across the call.
    let mut info: Box<DdsInfo> = Box::new(unsafe { std::mem::zeroed() });
    unsafe { GetDDSInfo(info.as_mut()) };
    let backend = match info.threading {
        0 => "none",
        1 => "Windows",
        2 => "OpenMP",
        3 => "GCD",
        4 => "Boost",
        5 => "STL",
        6 => "TBB",
        7 => "STLIMPL",
        8 => "PPLIMPL",
        _ => "?",
    };
    (backend.to_string(), info.num_cores, info.no_of_threads)
}

/// Choose DDS's threading backend: 3 = GCD, 5 = STL. Call before `set_threads`.
///
/// This is not a tuning knob, it is a fairness one on Apple Silicon. DDS's GCD
/// path dispatches to `DISPATCH_QUEUE_PRIORITY_BACKGROUND`, and background QoS
/// on Apple Silicon is confined to the efficiency cores -- four of them on an
/// M4 Pro. DDS therefore pins at four cores however many threads it is given,
/// and a scaling curve measured that way says nothing about DDS and everything
/// about which queue it asked for. Its STL backend uses plain `std::thread` at
/// default QoS and is scheduled across all cores, like ours.
pub fn set_backend(code: i32) -> Result<(), String> {
    // SAFETY: `SetThreading` takes an integer by value, validates it against
    // DDS's table of compiled-in backends, and stores it in a global. No
    // pointers, no lifetimes.
    match unsafe { SetThreading(code as c_int) } {
        RETURN_NO_FAULT => Ok(()),
        rc => Err(format!(
            "DDS refused threading backend {code} (error {rc}); it was probably \
             not compiled into this libdds.a"
        )),
    }
}
