import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { detectSightingSource } from '../pairing/data';

const css = readFileSync(new URL('./md3-expressive.css', import.meta.url), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');

function parseRules(source: string): Map<string, Record<string, string>> {
  const map = new Map<string, Record<string, string>>();
  for (const [, selector, body] of source.matchAll(/([^{}@]+)\{([^{}]*)\}/g)) {
    const decls: Record<string, string> = {};
    for (const d of body.split(';')) {
      const idx = d.indexOf(':');
      if (idx > 0) decls[d.slice(0, idx).trim()] = d.slice(idx + 1).trim();
    }
    const sel = selector.trim();
    map.set(sel, { ...map.get(sel), ...decls });
  }
  return map;
}

const rules = parseRules(css);

describe('MD3 Expressive Tokens', () => {
  const light = rules.get(':root')!;
  const dark = rules.get(":root[data-theme='dark']")!;

  it('defines all required dynamic color roles in light and dark schemes', () => {
    expect(light).toBeDefined();
    expect(dark).toBeDefined();

    const requiredTokens = [
      '--md-primary',
      '--md-on-primary',
      '--md-primary-container',
      '--md-on-primary-container',
      '--md-secondary-container',
      '--md-tertiary-container',
      '--md-surface',
      '--md-surface-low',
      '--md-surface-container',
      '--md-surface-high',
      '--md-surface-highest',
      '--md-outline',
      '--md-outline-variant',
    ];

    for (const token of requiredTokens) {
      expect(light[token], `light ${token}`).toBeDefined();
      expect(dark[token], `dark ${token}`).toBeDefined();
    }
  });

  it('maps Oink UI core variables cleanly to MD3 tokens', () => {
    const oinkCoreVars = [
      '--ok-bg',
      '--ok-surface',
      '--ok-well',
      '--ok-well-hi',
      '--ok-card',
      '--ok-text',
      '--ok-text-2',
      '--ok-text-3',
      '--ok-accent',
      '--ok-line',
      '--ok-line-2',
    ];

    for (const v of oinkCoreVars) {
      expect(light[v], `light ${v}`).toMatch(/^var\(--md-/);
      expect(dark[v], `dark ${v}`).toMatch(/^var\(--md-/);
    }

    expect(light['--ok-accent']).toBe('var(--md-primary)');
    expect(light['--ok-surface']).toBe('var(--md-surface)');
    expect(light['--ok-text']).toBe('var(--md-on-surface)');
  });

  it('defines expressive spatial springs and motion curves', () => {
    expect(light['--md-spatial-fast']).toBe('350ms linear(0, .234 7%, .773 22%, 1.06 40%, 1.039 58%, .994 78%, 1)');
    expect(light['--md-effects']).toBe('180ms cubic-bezier(.2, 0, 0, 1)');
  });

  it('defines expressive squircle and pill shape tokens', () => {
    expect(light['--md-radius-pill']).toBe('999px');
    expect(light['--md-radius-squircle']).toBe('28px 28px 12px 28px');
    expect(light['--md-radius-squircle-alt']).toBe('32px 8px 32px 32px');
    expect(light['--md-radius-connected-first']).toBe('28px 8px 8px 28px');
    expect(light['--md-radius-connected-last']).toBe('8px 28px 28px 8px');
  });

  it('defines connected button groups with morphing radii', () => {
    const connected = rules.get('.md-connected');
    const first = rules.get('.md-connected > *:first-child');
    const last = rules.get('.md-connected > *:last-child');
    expect(connected).toBeDefined();
    expect(first?.['border-radius']).toBe('28px 8px 8px 28px');
    expect(last?.['border-radius']).toBe('8px 28px 28px 8px');
  });

  it('defines expressive typography classes and figure chips', () => {
    const headline = rules.get('.md-headline');
    const title = rules.get('.md-title');
    const figure = rules.get('.md-chip-figure');

    expect(headline?.['letter-spacing']).toBe('-0.06em');
    expect(headline?.['font-weight']).toBe('800');
    expect(title?.['letter-spacing']).toBe('-0.04em');
    expect(title?.['font-weight']).toBe('750');
    expect(figure?.['font-variant-numeric']).toBe('tabular-nums');
  });

  it('defines expressive sighting badges for LAN, Tailscale (M2), and Bluetooth PAN (M3)', () => {
    const badge = rules.get('.sighting-badge');
    const lan = rules.get('.sighting-badge[data-source="lan"]');
    const tailscale = rules.get('.sighting-badge[data-source="tailscale"]');
    const bt = rules.get('.sighting-badge[data-source="bluetooth"]');

    expect(badge?.['border-radius']).toBe('999px');
    expect(badge?.['font-weight']).toBe('750');
    expect(lan?.background).toBe('var(--md-badge-lan-bg)');
    expect(tailscale?.background).toBe('var(--md-badge-tailscale-bg)');
    expect(bt?.background).toBe('var(--md-badge-bluetooth-bg)');
  });
});

describe('Sighting Source Detection', () => {
  it('correctly detects Tailscale (M2) peers by source and CGNAT IP range', () => {
    expect(detectSightingSource(undefined, 3)).toBe('tailscale');
    expect(detectSightingSource('100.64.0.1:47470', 0)).toBe('tailscale');
    expect(detectSightingSource('100.127.255.254')).toBe('tailscale');
    expect(detectSightingSource('100.100.50.25:47470')).toBe('tailscale');
  });

  it('correctly detects Bluetooth PAN (M3) peers by source and PAN IP range', () => {
    expect(detectSightingSource(undefined, 4)).toBe('bluetooth');
    expect(detectSightingSource('172.20.10.2:47470', 0)).toBe('bluetooth');
    expect(detectSightingSource('172.20.10.8')).toBe('bluetooth');
  });

  it('defaults to LAN for standard subnet IP addresses and beacon/mdns sources', () => {
    expect(detectSightingSource(undefined, 0)).toBe('lan');
    expect(detectSightingSource(undefined, 1)).toBe('lan');
    expect(detectSightingSource('192.168.1.100:47470')).toBe('lan');
    expect(detectSightingSource('10.0.0.15')).toBe('lan');
    expect(detectSightingSource('172.16.0.5')).toBe('lan');
  });
});
