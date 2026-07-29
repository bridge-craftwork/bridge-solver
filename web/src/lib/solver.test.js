import { describe, expect, it } from 'vitest'
import { reactive, ref } from 'vue'
import { playRequest } from './solver.js'
import { parseDealString } from './deal.js'

const DEAL = 'N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72'

describe('playRequest', () => {
  const hands = parseDealString(DEAL)

  it('builds the request the engine expects', () => {
    const req = playRequest({
      hands,
      trump: 'S',
      declarer: 'S',
      leader: 'W',
      plays: ['HK', 'H3'],
    })
    expect(req).toEqual({
      dealstr: DEAL,
      trump: 'S',
      declarer: 'S',
      leader: 'W',
      plays: ['HK', 'H3'],
    })
  })

  it('spells a null trump as NT', () => {
    // trumpFromContract returns null for notrump, and every call site relies on
    // this rather than remembering to map it.
    const req = playRequest({ hands, trump: null, declarer: 'N', leader: 'E', plays: [] })
    expect(req.trump).toBe('NT')
  })

  /*
   * The request crosses into a Web Worker, so it has to survive
   * structuredClone. Vue reactive state is delivered as Proxy objects, which
   * cannot be cloned — postMessage throws DataCloneError rather than returning a
   * wrong answer, so the whole analysis silently disappears.
   */
  it('produces plain data that survives a structured clone', () => {
    const reactiveHands = reactive(parseDealString(DEAL))
    const reactivePlays = ref(['HK', 'H3', 'HA']).value

    const req = playRequest({
      hands: reactiveHands,
      trump: 'H',
      declarer: 'S',
      leader: 'W',
      plays: reactivePlays,
    })

    expect(() => structuredClone(req)).not.toThrow()
    expect(structuredClone(req)).toEqual(req)
  })

  it('copies plays rather than aliasing the caller array', () => {
    const plays = ['HK']
    const req = playRequest({ hands, trump: 'H', declarer: 'S', leader: 'W', plays })
    plays.push('H3')
    expect(req.plays).toEqual(['HK'])
  })
})
