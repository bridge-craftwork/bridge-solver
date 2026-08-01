import { describe, expect, it } from 'vitest'
import { INPUT_KINDS, detectKind, parseInput } from './input.js'

const DEAL = 'N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72'

const LIN =
  'qx|o6|pn|aam135,usvi,kemistry,jelsma|st||md|4SQH5AD28JKAC257JA,S379KH278QKD69C9T,S26AH369TJD5C38QK,|sv|e|mb|p|mb|1D|pc|HK|mc|12|'

describe('detectKind', () => {
  it('spots a handviewer URL by its lin parameter', () => {
    expect(detectKind('https://www.bridgebase.com/tools/handviewer.html?lin=pn%7CS%2CW%7C')).toBe(
      INPUT_KINDS.URL
    )
    expect(detectKind('handviewer.html?board=1&lin=pn%7CS%7C')).toBe(INPUT_KINDS.URL)
    expect(detectKind('lin=pn%7CS%7C')).toBe(INPUT_KINDS.URL)
  })

  it('spots LIN by its own tokens', () => {
    expect(detectKind(LIN)).toBe(INPUT_KINDS.LIN)
    expect(detectKind('pn|S,W,N,E|md|1SAKHJD876C5432,,,|')).toBe(INPUT_KINDS.LIN)
  })

  it('prefers the URL reading when a URL contains LIN', () => {
    // A handviewer URL's decoded value is LIN, so order of checks matters.
    expect(detectKind('https://x/handviewer.html?lin=pn%7CS%7Cmd%7C1S...')).toBe(INPUT_KINDS.URL)
  })

  it('spots PBN by its tags', () => {
    expect(detectKind(`[Deal "${DEAL}"]`)).toBe(INPUT_KINDS.PBN)
    expect(detectKind('[Event "Something"]\n[Deal "x"]')).toBe(INPUT_KINDS.PBN)
  })

  it('spots a bare deal string', () => {
    expect(detectKind(DEAL)).toBe(INPUT_KINDS.DEAL)
  })

  it('does not mistake a pipe in prose for LIN', () => {
    // A stray pipe is not enough — the token has to be one LIN actually uses.
    expect(detectKind('this | that')).toBe(INPUT_KINDS.UNKNOWN)
  })

  it('reports nothing for empty or unrecognisable input', () => {
    expect(detectKind('')).toBe(INPUT_KINDS.UNKNOWN)
    expect(detectKind('   ')).toBe(INPUT_KINDS.UNKNOWN)
    expect(detectKind('hello')).toBe(INPUT_KINDS.UNKNOWN)
  })
})

describe('parseInput, PBN', () => {
  const board = (extra = '') => `[Board "7"]\n[Dealer "E"]\n[Vulnerable "NS"]\n[Deal "${DEAL}"]\n${extra}`

  it('reads a board with no contract at all', async () => {
    const { kind, boards } = await parseInput(board())
    expect(kind).toBe(INPUT_KINDS.PBN)
    expect(boards).toHaveLength(1)
    expect(boards[0].contract).toBeNull()
    expect(boards[0].leader).toBeNull()
    expect(boards[0].plays).toEqual([])
  })

  /*
   * A board with a contract but no cards played is analysable, and PBN never states
   * an opening leader — so it has to be derived, or the whole analysis is withheld
   * over a field nobody wrote down.
   */
  it('derives the opening leader from the declarer', async () => {
    const { boards } = await parseInput(board('[Declarer "N"]\n[Contract "4S"]'))
    expect(boards[0]).toMatchObject({ contract: '4S', declarer: 'N', leader: 'E', plays: [] })
  })

  it('derives it correctly for every declarer', async () => {
    for (const [declarer, leader] of [['N', 'E'], ['E', 'S'], ['S', 'W'], ['W', 'N']]) {
      const { boards } = await parseInput(board(`[Declarer "${declarer}"]\n[Contract "3NT"]`))
      expect(boards[0].leader, declarer).toBe(leader)
    }
  })

  it('reads a multi-board file', async () => {
    const { boards } = await parseInput(`${board('[Contract "4S"]\n[Declarer "N"]')}\n\n${board()}`)
    expect(boards).toHaveLength(2)
  })

  it('refuses something that is not a hand at all', async () => {
    await expect(parseInput('hello there')).rejects.toThrow(/does not look like/)
  })

  /*
   * "Export handviewer link" in BBO gives a shortened link, so this is what a
   * lot of people will paste first. It cannot be followed — see BBO_SHORT_LINK
   * in input.js — so the least we owe them is a message that says what to do
   * instead of the generic "that does not look like a hand".
   */
  it('explains what to do with a shortened BBO link', async () => {
    for (const link of [
      'https://tinyurl.bridgebase.com/5cyerrh5',
      'http://tinyurl.bridgebase.com/abc123',
      'tinyurl.bridgebase.com/abc123',
      '  https://tinyurl.bridgebase.com/abc123  ',
    ]) {
      await expect(parseInput(link)).rejects.toThrow(/copy the full address/)
    }
  })

  it('still reads the expanded handviewer URL that link resolves to', async () => {
    // The `lin=` parameter is what makes it recognisable, so the expanded form
    // takes the URL path rather than falling through to the short-link message.
    const expanded =
      'https://www.bridgebase.com/tools/handviewer.html?v3b=web&lin=pn%7Ca%2Cb%2Cc%2Cd%7C'
    expect(detectKind(expanded)).toBe(INPUT_KINDS.URL)
  })

  it('says what is wrong with a PBN carrying no deal', async () => {
    await expect(parseInput('[Event "Nothing here"]\n[Dealer "N"]')).rejects.toThrow(/No \[Deal/)
  })
})
