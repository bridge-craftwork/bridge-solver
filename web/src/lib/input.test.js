import { describe, expect, it } from 'vitest'
import { INPUT_KINDS, detectKind } from './input.js'

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
