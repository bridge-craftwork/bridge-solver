import { beforeEach, describe, expect, it } from 'vitest'
import { poolSize, record, recordSolve, reset, solveHistory, timings, warmupRatio } from './perf.js'

beforeEach(() => reset())

describe('segment timings', () => {
  it('records a segment and reads it back', () => {
    record('wasmFetch', 42)
    expect(timings().wasmFetch).toBe(42)
  })

  it('ignores a name that is not a known segment', () => {
    // Mis-spelling a segment should not quietly invent a field that nothing
    // ever reads, leaving the real one null.
    record('wasmFecth', 42)
    expect(timings()).toEqual({
      wasmFetch: null,
      wasmCompile: null,
      wasmInstantiate: null,
      firstSolve: null,
    })
  })

  it('starts with every segment unmeasured rather than zero', () => {
    // Zero is a legitimate timing; "not measured" has to be distinguishable
    // from "measured as instant", or the panel reports a cold start that never
    // happened.
    expect(Object.values(timings()).every((v) => v === null)).toBe(true)
  })
})

describe('solve history', () => {
  it('takes the first solve as the first_solve segment', () => {
    recordSolve(120)
    recordSolve(30)
    expect(timings().firstSolve).toBe(120)
  })

  it('keeps solves in call order', () => {
    recordSolve(120)
    recordSolve(40)
    recordSolve(30)
    expect(solveHistory()).toEqual([120, 40, 30])
  })

  it('has no warm-up ratio until three solves are in', () => {
    recordSolve(120)
    expect(warmupRatio()).toBeNull()
    recordSolve(40)
    expect(warmupRatio()).toBeNull()
  })

  it('reports the first solve against the third', () => {
    recordSolve(120)
    recordSolve(50)
    recordSolve(30)
    expect(warmupRatio()).toBeCloseTo(4)
  })

  it('does not divide by a zero third solve', () => {
    recordSolve(120)
    recordSolve(50)
    recordSolve(0)
    expect(warmupRatio()).toBeNull()
  })
})

describe('poolSize', () => {
  /*
   * Never `hardwareConcurrency` itself: that count includes efficiency cores,
   * which run a solve 2-3x slower and become the straggler that gates the run.
   */
  it('halves the core count', () => {
    expect(poolSize({ cores: 8, memoryGb: 8 })).toBe(4)
    expect(poolSize({ cores: 6, memoryGb: 8 })).toBe(3)
  })

  it('caps at four however many cores are reported', () => {
    expect(poolSize({ cores: 32, memoryGb: 16 })).toBe(4)
  })

  it('never returns less than one', () => {
    expect(poolSize({ cores: 1, memoryGb: 8 })).toBe(1)
    expect(poolSize({ cores: 0, memoryGb: 0 })).toBe(1)
  })

  it('assumes two cores when the browser will not say', () => {
    expect(poolSize({ cores: null, memoryGb: null })).toBe(1)
  })

  /*
   * Each worker carries its own position cache, so a device that swaps
   * mid-solve is slower than one that never forked.
   */
  it('holds back to two workers on a low-memory device', () => {
    expect(poolSize({ cores: 8, memoryGb: 4 })).toBe(2)
    expect(poolSize({ cores: 16, memoryGb: 2 })).toBe(2)
  })

  it('leaves a device that will not report memory alone', () => {
    expect(poolSize({ cores: 8, memoryGb: null })).toBe(4)
  })
})
