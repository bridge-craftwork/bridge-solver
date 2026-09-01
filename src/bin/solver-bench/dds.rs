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

extern "C" {
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
    // SAFETY: `SetResources` takes two integers by value and stores them in
    // DDS's own globals. It allocates and cannot fail; there is no pointer or
    // lifetime involved.
    unsafe { SetResources(0, threads as c_int) }
}

/// Solve one deal's full twenty-entry table.
///
/// Returns the table in this crate's own layout — `[direction][strain]` over
/// N,E,S,W and C,D,H,S,NT — so it can be compared with `DdTricks` directly.
pub fn solve_table(pbn: &str) -> Result<[[u8; 5]; 4], String> {
    // DDS reads a fixed 80-byte buffer and wants a NUL, so refuse anything
    // that would not round-trip rather than silently truncating a deal.
    if pbn.len() >= 80 {
        return Err(format!("deal string is {} bytes, DDS allows 79", pbn.len()));
    }
    if pbn.contains('\0') {
        return Err("deal string contains a NUL".into());
    }

    // These structs run to hundreds of kilobytes, so they are boxed rather
    // than built on the stack.
    let mut deals: Box<DdTableDealsPbn> = Box::new(unsafe { std::mem::zeroed() });
    let mut res: Box<DdTablesRes> = Box::new(unsafe { std::mem::zeroed() });
    let mut par: Box<AllParResults> = Box::new(unsafe { std::mem::zeroed() });

    deals.no_of_tables = 1;
    for (slot, byte) in deals.deals[0].cards.iter_mut().zip(pbn.bytes()) {
        *slot = byte as c_char;
    }

    // 0 means "compute this strain"; all five are wanted.
    let mut trump_filter = [0 as c_int; DDS_STRAINS];

    // SAFETY: every pointer is to a live, fully zeroed, correctly laid out
    // struct that outlives the call, and `trump_filter` is the five-element
    // array the signature requires. DDS writes only through `resp` and
    // `presp`, and the deal buffer is NUL-terminated because it was zeroed
    // and the string is shorter than the buffer.
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
    let mut out = [[0u8; 5]; 4];
    for (hand, row) in out.iter_mut().enumerate() {
        for (strain, cell) in row.iter_mut().enumerate() {
            let tricks = res.results[0].res_table[DDS_STRAIN_FOR[strain]][hand];
            *cell = u8::try_from(tricks)
                .map_err(|_| format!("DDS returned {tricks} tricks, which is not 0..=13"))?;
        }
    }
    Ok(out)
}
