import { describe, expect, it } from 'vitest'
import fixture from '../fixtures/url-normalization.json'
import { normalizeUrl } from '../../src/core'
import { normalize } from '../../../extension/src/shared'

// The same fixture is asserted by Rust (db.rs), so all three implementations agree.
describe('URL normalization (shared fixture)', () => {
  it.each(fixture.cases)('desktop normalizes $input', ({ input, expected }) => {
    expect(normalizeUrl(input)).toBe(expected)
  })

  it.each(fixture.cases)('extension normalizes $input', ({ input, expected }) => {
    expect(normalize(input)).toBe(expected)
  })

  it.each(fixture.distinct)('keeps %s and %s distinct', (a, b) => {
    expect(normalizeUrl(a)).not.toBe(normalizeUrl(b))
    expect(normalize(a)).not.toBe(normalize(b))
  })

  it.each(fixture.rejected)('rejects %s in the desktop editor', (input) => {
    expect(() => normalizeUrl(input)).toThrow()
  })
})

describe('desktop URL input conveniences', () => {
  it('adds https to bare hosts, including hosts with a port', () => {
    expect(normalizeUrl('linkedin.com/jobs/')).toBe('https://linkedin.com/jobs')
    expect(normalizeUrl('  example.com  ')).toBe('https://example.com')
    expect(normalizeUrl('localhost:3000')).toBe('https://localhost:3000')
    expect(normalizeUrl('example.com:8080/app')).toBe('https://example.com:8080/app')
  })

  it('requires a value', () => {
    expect(() => normalizeUrl('   ')).toThrow('URL is required.')
  })

  it('extension leaves non-web URLs untouched instead of throwing', () => {
    expect(normalize('chrome://settings')).toBe('chrome://settings')
    expect(normalize('not a url')).toBe('not a url')
  })
})
