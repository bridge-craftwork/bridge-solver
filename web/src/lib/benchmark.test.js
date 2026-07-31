import { describe, expect, it } from 'vitest'
import {
  REFERENCE_MS,
  SLOW_SCORE,
  expectedTotalMs,
  isSlow,
  probeDeal,
  scoreFrom,
  slowMessage,
} from './benchmark.js'

describe('probeDeal', () => {
  it('is a real deal with a frozen expected table', () => {
    const { deal, ddtricks } = probeDeal()
    expect(deal).toMatch(/^N:/)
    // Four hands, each four dot-separated holdings.
    expect(deal.replace(/^N:/, '').split(' ')).toHaveLength(4)
    // Twenty cells, one character each.
    expect(ddtricks).toHaveLength(20)
    expect(ddtricks).toMatch(/^[0-9a-d]{20}$/)
  })
})

describe('scoreFrom', () => {
  it('scores the reference machine at about 100', () => {
    expect(scoreFrom(REFERENCE_MS)).toBe(100)
  })

  it('halves the score for a device twice as slow', () => {
    expect(scoreFrom(REFERENCE_MS * 2)).toBe(50)
    expect(scoreFrom(REFERENCE_MS * 4)).toBe(25)
  })

  it('scores a faster device above 100', () => {
    expect(scoreFrom(REFERENCE_MS / 2)).toBe(200)
  })

  /*
   * A backgrounded tab or a device that was asleep can report an absurd
   * measurement. A floor keeps that reading as "very slow" rather than as a
   * broken probe.
   */
  it('never scores below one', () => {
    expect(scoreFrom(1_000_000)).toBe(1)
  })

  it('has no score for a measurement that did not happen', () => {
    expect(scoreFrom(0)).toBeNull()
    expect(scoreFrom(null)).toBeNull()
    expect(scoreFrom(-5)).toBeNull()
  })
})

describe('isSlow', () => {
  it('leaves the reference machine alone', () => {
    expect(isSlow(100)).toBe(false)
  })

  it('warns well below the reference', () => {
    expect(isSlow(10)).toBe(true)
  })

  it('does not warn without a measurement', () => {
    // No probe result is not evidence of a slow device, and a warning shown to
    // everyone is a warning nobody reads.
    expect(isSlow(null)).toBe(false)
  })

  it('treats the threshold itself as acceptable', () => {
    expect(isSlow(SLOW_SCORE)).toBe(false)
    expect(isSlow(SLOW_SCORE - 1)).toBe(true)
  })
})

describe('expectedTotalMs', () => {
  it('scales inversely with the score', () => {
    const atReference = expectedTotalMs(100)
    expect(expectedTotalMs(50)).toBeCloseTo(atReference * 2, -1)
    expect(expectedTotalMs(25)).toBeCloseTo(atReference * 4, -1)
  })

  it('has no estimate without a score', () => {
    expect(expectedTotalMs(null)).toBeNull()
  })
})

describe('slowMessage', () => {
  it('says nothing for a device that is fast enough', () => {
    expect(slowMessage(100)).toBeNull()
    expect(slowMessage(null)).toBeNull()
  })

  it('quotes a range and promises progressive results', () => {
    const message = slowMessage(5)
    expect(message).toMatch(/\d+–\d+ seconds/)
    expect(message).toMatch(/appear as they are found/)
  })

  it('gets slower as the score falls', () => {
    const seconds = (s) => Number(slowMessage(s).match(/roughly (\d+)/)[1])
    expect(seconds(5)).toBeGreaterThan(seconds(20))
  })

  /*
   * A range whose bottom rounds to zero or one reads as "instant", which is the
   * opposite of what this is for.
   */
  it('never promises less than two seconds', () => {
    expect(slowMessage(SLOW_SCORE - 1)).toMatch(/roughly [2-9]/)
  })
})
