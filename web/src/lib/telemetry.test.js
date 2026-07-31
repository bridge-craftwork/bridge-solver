import { describe, expect, it } from 'vitest'
import { bucket, embedOrigin } from './telemetry.js'

describe('bucket', () => {
  /*
   * A precise millisecond timing is a high-entropy number tied to one device's
   * exact performance — a good fingerprint. Rounding keeps "is this device
   * slow, and how slow" answerable while discarding the resolution that would
   * make two records distinguishable.
   */
  it('rounds to the nearest 100ms', () => {
    expect(bucket(0)).toBe(0)
    expect(bucket(49)).toBe(0)
    expect(bucket(50)).toBe(100)
    expect(bucket(377)).toBe(400)
    expect(bucket(1234)).toBe(1200)
  })

  it('never emits a negative or non-finite bucket', () => {
    expect(bucket(-5)).toBe(0)
    expect(bucket(NaN)).toBe(0)
    expect(bucket(Infinity)).toBe(0)
    expect(bucket(undefined)).toBe(0)
    expect(bucket('nonsense')).toBe(0)
  })

  it('reads a numeric string, since timings arrive from several places', () => {
    expect(bucket('250')).toBe(300)
  })
})

describe('embedOrigin', () => {
  const win = (overrides) => ({
    location: {},
    document: { referrer: '' },
    ...overrides,
  })

  it('is empty when the page is top-level', () => {
    const w = win()
    w.self = w
    w.top = w
    expect(embedOrigin(w)).toBe('')
  })

  it('prefers ancestorOrigins, and takes only the origin', () => {
    // The host's full URL is none of our business and could itself carry a hand
    // in its query string.
    const w = win({ location: { ancestorOrigins: ['https://host.example/lesson?hand=SECRET'] } })
    expect(embedOrigin(w)).toBe('https://host.example')
  })

  it('falls back to the referrer where ancestorOrigins is absent (Firefox)', () => {
    const w = win({ document: { referrer: 'https://host.example/some/page' } })
    w.self = w
    w.top = {}
    expect(embedOrigin(w)).toBe('https://host.example')
  })

  /*
   * "Framed by someone we cannot name" and "not framed" are different facts.
   * Collapsing them to '' would overstate the top-level count.
   */
  it('reports unknown when framed with the referrer suppressed', () => {
    const w = win()
    w.self = w
    w.top = {}
    expect(embedOrigin(w)).toBe('unknown')
  })

  it('reports unknown when reading window.top throws (cross-origin)', () => {
    const w = win()
    w.self = w
    Object.defineProperty(w, 'top', {
      get() {
        throw new Error('cross-origin')
      },
    })
    expect(embedOrigin(w)).toBe('unknown')
  })

  it('survives a malformed referrer rather than throwing into the caller', () => {
    const w = win({ document: { referrer: 'not a url' } })
    w.self = w
    w.top = {}
    expect(() => embedOrigin(w)).not.toThrow()
  })

  it('is empty with no window at all, so it is safe under a test runner', () => {
    expect(embedOrigin(null)).toBe('')
  })
})
