import { describe, expect, it } from 'vitest'
import { normalizeUrl } from '../../src/core'

// BUG-032 (Low): "localhost:3000" was parsed as a URL with scheme "localhost:" and rejected.
describe('BUG-032 host:port input is accepted', () => {
  it('treats host:port as a host, while real schemes are still validated', () => {
    expect(normalizeUrl('localhost:3000')).toBe('https://localhost:3000')
    expect(() => normalizeUrl('javascript:alert(1)')).toThrow()
    expect(() => normalizeUrl('mailto:me@example.com')).toThrow()
  })
})
