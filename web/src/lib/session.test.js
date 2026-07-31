import { describe, expect, it } from 'vitest'
import { DEFAULTS, readUrl, resolveInitial } from './session.js'

const DEAL = 'N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72'

describe('readUrl', () => {
  it('finds the hand under any of its names', () => {
    for (const key of ['hand', 'lin', 'pbn', 'url', 'board']) {
      expect(readUrl(`?${key}=${encodeURIComponent(DEAL)}`).hand, key).toBe(DEAL)
    }
  })

  it('reads options from the hash as well as the query', () => {
    // A static host that cannot rewrite query strings can still drive it.
    expect(readUrl('', '#x?embed=1&width=390').options).toEqual({ embed: true, width: 390 })
  })

  it('lets the query win over the hash', () => {
    expect(readUrl('?width=800', '#x?width=390').options.width).toBe(800)
  })

  it('treats a bare key as true', () => {
    expect(readUrl('?embed').options.embed).toBe(true)
    expect(readUrl('?embed=').options.embed).toBe(true)
  })

  it('accepts the spellings someone would hand-write', () => {
    for (const v of ['1', 'true', 'yes', 'on', 'TRUE']) {
      expect(readUrl(`?embed=${v}`).options.embed, v).toBe(true)
    }
    for (const v of ['0', 'false', 'no', 'off']) {
      expect(readUrl(`?embed=${v}`).options.embed, v).toBe(false)
    }
  })

  it('takes short aliases for the viewport box', () => {
    expect(readUrl('?w=1180&h=820').options).toEqual({ width: 1180, height: 820 })
    expect(readUrl('?vw=390&vh=844').options).toEqual({ width: 390, height: 844 })
  })

  it('ignores a nonsensical box rather than rendering into it', () => {
    expect(readUrl('?width=0').options.width).toBeUndefined()
    expect(readUrl('?width=-5').options.width).toBeUndefined()
    expect(readUrl('?width=wide').options.width).toBeUndefined()
  })

  /*
   * The bug this pins: filling every option in with its default made "absent"
   * indistinguishable from "explicitly the default", so an absent parameter
   * silently overwrote a stored preference. `declarerSouth` came back on after
   * every reload however you had left it.
   */
  it('reports only the options the URL actually mentions', () => {
    expect(readUrl('').options).toEqual({})
    expect(readUrl('?embed=1').options).toEqual({ embed: true })
    expect(readUrl('?declarerSouth=0').options).toEqual({ declarerSouth: false })
  })

  it('says whether the URL asked for anything', () => {
    expect(readUrl('').present).toBe(false)
    expect(readUrl('?unrelated=1').present).toBe(false)
    expect(readUrl('?embed=1').present).toBe(true)
    expect(readUrl(`?hand=${encodeURIComponent(DEAL)}`).present).toBe(true)
  })

  it('survives an encoded handviewer URL, which carries its own lin parameter', () => {
    const handviewer = 'https://www.bridgebase.com/tools/handviewer.html?lin=pn%7CS%2CW%7C&c=9'
    const parsed = readUrl(`?hand=${encodeURIComponent(handviewer)}`)
    expect(parsed.hand).toBe(handviewer)
  })
})

describe('resolveInitial', () => {
  const url = (search = '', hash = '') => readUrl(search, hash)

  it('opens with nothing when there is neither a URL nor a stored hand', () => {
    const got = resolveInitial(url(), null)
    expect(got).toMatchObject({ hand: null, source: 'none' })
    expect(got.options).toEqual(DEFAULTS)
  })

  it('prefers the URL over what was stored', () => {
    const got = resolveInitial(url(`?hand=${encodeURIComponent(DEAL)}`), {
      hand: 'something else',
      options: {},
    })
    expect(got.hand).toBe(DEAL)
    expect(got.source).toBe('url')
  })

  it('falls back to the stored hand', () => {
    const got = resolveInitial(url(), { hand: DEAL, options: {}, savedAt: 'then' })
    expect(got).toMatchObject({ hand: DEAL, source: 'storage', savedAt: 'then' })
  })

  it('restores a stored preference that differs from the default', () => {
    const got = resolveInitial(url(), { hand: DEAL, options: { declarerSouth: false } })
    expect(got.options.declarerSouth).toBe(false)
  })

  it('still lets the URL override a stored preference', () => {
    const got = resolveInitial(url('?declarerSouth=1'), {
      hand: DEAL,
      options: { declarerSouth: false },
    })
    expect(got.options.declarerSouth).toBe(true)
  })

  /*
   * The box and the embed flag describe the frame the page is in, not the hand you
   * were looking at. Carrying them over from storage would strand someone in a
   * 390px embedded box with no way out of it.
   */
  it('does not restore the viewport box or embed flag from storage', () => {
    const got = resolveInitial(url(), {
      hand: DEAL,
      options: { declarerSouth: false, embed: true, width: 390, height: 844 },
    })
    expect(got.options.embed).toBe(false)
    expect(got.options.width).toBeNull()
    expect(got.options.height).toBeNull()
    // The preference that *is* about the hand still comes back.
    expect(got.options.declarerSouth).toBe(false)
  })
})
