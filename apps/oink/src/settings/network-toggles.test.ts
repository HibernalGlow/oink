import { describe, expect, it } from 'vitest';
import { setLanguage } from '../i18n';
import { DEFAULT_PREFS } from './prefs';
import { SECTIONS, searchSections } from './schema';

describe('Network & new feature settings schema', () => {
  it('includes network section with Tailscale and Bluetooth PAN items', () => {
    const network = SECTIONS.find((s) => s.id === 'network');
    expect(network).toBeDefined();
    expect(network?.title).toBeTruthy();
    expect(network?.items.map((i) => i.id)).toEqual([
      'tailscaleDiscovery',
      'bluetoothPanDiscovery',
    ]);
  });

  it('includes extendedMouseButtons in input section', () => {
    const input = SECTIONS.find((s) => s.id === 'input');
    expect(input).toBeDefined();
    expect(input?.items.some((i) => i.id === 'extendedMouseButtons')).toBe(true);
  });

  it('includes destroyWebViewOnClose in general section with only mac', () => {
    const general = SECTIONS.find((s) => s.id === 'general');
    expect(general).toBeDefined();
    const item = general?.items.find((i) => i.id === 'destroyWebViewOnClose');
    expect(item).toBeDefined();
    expect(item?.only).toBe('mac');
  });

  it('defaults new toggles to true in DEFAULT_PREFS', () => {
    expect(DEFAULT_PREFS.tailscaleDiscovery).toBe(true);
    expect(DEFAULT_PREFS.bluetoothPanDiscovery).toBe(true);
    expect(DEFAULT_PREFS.extendedMouseButtons).toBe(true);
    expect(DEFAULT_PREFS.destroyWebViewOnClose).toBe(true);
  });

  it('finds new items via searchSections in both English and Chinese', () => {
    // English search
    setLanguage('en');
    const macHits = searchSections('mac', 'Tailscale');
    expect(macHits.get('network')).toBeDefined();

    const panHits = searchSections('mac', 'BNEP');
    expect(panHits.get('network')).toBeDefined();

    const mouseHits = searchSections('mac', 'buttons');
    expect(mouseHits.get('input')).toBeDefined();

    const webviewHits = searchSections('mac', 'WebView');
    expect(webviewHits.get('general')).toBeDefined();

    // Chinese search
    setLanguage('zh-Hans');
    const zhMacHits = searchSections('mac', '跨网段');
    expect(zhMacHits.get('network')).toBeDefined();

    const zhPanHits = searchSections('mac', '直连');
    expect(zhPanHits.get('network')).toBeDefined();

    const zhMouseHits = searchSections('mac', '侧键');
    expect(zhMouseHits.get('input')).toBeDefined();

    const zhWebviewHits = searchSections('mac', '内存');
    expect(zhWebviewHits.get('general')).toBeDefined();
  });
});

