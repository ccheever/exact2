// Ported favicon tests from T3 Code 1e2ecbd975 (MIT, LICENSE-T3).
// Resolver/cache tests below are clone-authored.
import { describe, expect, it } from "bun:test";
import { toolActivityFaviconUrl, toolActivityIconSources, syncToolActivityIcons } from "./timeline-tool-icons";
describe("toolActivityFaviconUrl", () => {
  it("uses the page origin instead of a third-party favicon service", () => {
    expect(toolActivityFaviconUrl({ pageUrl: "https://example.com/docs/page?q=1" }, "light")).toBe(
      "https://example.com/favicon.ico",
    );
    expect(toolActivityFaviconUrl({ pageUrl: "http://localhost:5173/app" }, "light")).toBe(
      "http://localhost:5173/favicon.ico",
    );
  });

  it("selects site-owned light and dark variants without filtering full-color icons", () => {
    expect(toolActivityFaviconUrl({ pageUrl: "https://github.com/openai/codex" }, "light")).toBe(
      "https://github.githubassets.com/favicons/favicon.svg",
    );
    expect(toolActivityFaviconUrl({ pageUrl: "https://github.com/openai/codex" }, "dark")).toBe(
      "https://github.githubassets.com/favicons/favicon-dark.svg",
    );

    const fullColorIcon = {
      pageUrl: "https://example.com/docs",
      faviconUrl: "https://cdn.example.com/full-color.png",
    };
    expect(toolActivityFaviconUrl(fullColorIcon, "light")).toBe(fullColorIcon.faviconUrl);
    expect(toolActivityFaviconUrl(fullColorIcon, "dark")).toBe(fullColorIcon.faviconUrl);
  });

  it("prefers a provider-supplied dark favicon", () => {
    expect(
      toolActivityFaviconUrl(
        {
          pageUrl: "https://example.com/docs",
          faviconUrl: "https://example.com/light.svg",
          faviconUrlDark: "https://example.com/dark.svg",
        },
        "dark",
      ),
    ).toBe("https://example.com/dark.svg");
  });

  it("accepts provider-supplied image URLs but rejects extension URLs", () => {
    expect(
      toolActivityFaviconUrl(
        { pageUrl: "https://example.com/docs", faviconUrl: "https://example.com/icon.png" },
        "light",
      ),
    ).toBe("https://example.com/icon.png");
    expect(
      toolActivityFaviconUrl(
        { pageUrl: "https://example.com/docs", faviconUrl: "chrome-extension://example/_favicon/" },
        "light",
      ),
    ).toBe("https://example.com/favicon.ico");
  });
});

import type { Obj } from './domain';
const native = { available: true, watch() {}, later: async () => ({}) };
function fixture(items: Obj[], rpc: (native: unknown, method: string, payload: Obj) => Promise<Obj> = async () => ({ relativeUrl: '/assets/icon' })) {
  return { generation: 1, environmentId: 'environment-a', origin: 'http://localhost:16001', connection: 'connected',
    projection: { visibleTurnItems: items.map(item => ({ item })) }, rpc };
}
const appIcon = { toolIcon: { _tag: 'native-app', app: { _tag: 'app-id', appId: 'com.apple.Safari' } } };

describe('toolActivityIconSources', () => {
  it('uses toolIcon before source icon and resolves both themes', () => {
    const client = fixture([]);
    const source = { icon: { _tag: 'themed-logo', logoUrl: 'source' } };
    expect(toolActivityIconSources(client, { toolSource: source })).toEqual({ iconLight: 'source', iconDark: 'source' });
    expect(toolActivityIconSources(client, { toolSource: source, toolIcon: { _tag: 'themed-logo', logoUrl: 'light', logoUrlDark: 'dark' } }))
      .toEqual({ iconLight: 'light', iconDark: 'dark' });
    expect(toolActivityIconSources(client, { toolSource: source, tone: 'warning' })).toEqual({ iconLight: '', iconDark: '' });
    expect(toolActivityIconSources(client, { toolSource: source, tone: 'error' })).toEqual({ iconLight: '', iconDark: '' });
  });
});

describe('syncToolActivityIcons', () => {
  it('deduplicates native assets and remints after five minutes', async () => {
    const calls: Obj[] = [];
    const client = fixture([appIcon, appIcon], async (_native, method, payload) => {
      expect(method).toBe('assets.createUrl'); calls.push(payload);
      return { relativeUrl: `/assets/icon?mint=${calls.length}` };
    });
    expect(toolActivityIconSources(client, appIcon).iconLight).toBe('');
    await syncToolActivityIcons(client, native, 1);
    expect(calls).toEqual([{ resource: { _tag: 'native-app-icon', app: { _tag: 'app-id', appId: 'com.apple.Safari' } } }]);
    expect(toolActivityIconSources(client, appIcon).iconLight).toBe('http://localhost:16001/assets/icon?mint=1');
    await syncToolActivityIcons(client, native, 100);
    expect(calls.length).toBe(1);
    await syncToolActivityIcons(client, native, 300_001);
    expect(calls.length).toBe(2);
    expect(toolActivityIconSources(client, appIcon).iconDark).toContain('mint=2');
  });
  it('keeps delayed native icon replies distinct across skipped loop rows', async () => {
    const secondIcon = { toolIcon: { _tag: 'native-app', app: { _tag: 'app-id', appId: 'com.apple.Terminal' } } };
    const replies: Array<(value: Obj) => void> = [];
    const client = fixture([appIcon, {}, secondIcon, {}], async () => new Promise(resolve => replies.push(resolve)));
    const read = syncToolActivityIcons(client, native, 1);
    replies[1]!({ relativeUrl: '/terminal' });
    replies[0]!({ relativeUrl: '/safari' });
    await read;
    expect(toolActivityIconSources(client, appIcon).iconLight).toBe('http://localhost:16001/safari');
    expect(toolActivityIconSources(client, secondIcon).iconLight).toBe('http://localhost:16001/terminal');
  });
  it('keeps old connection replies out of the new connection', async () => {
    let finish: (value: Obj) => void = () => {};
    const client = fixture([appIcon], async () => new Promise<Obj>(resolve => { finish = resolve; }));
    const request = syncToolActivityIcons(client, native, 1);
    client.generation++;
    finish({ relativeUrl: '/old-icon' });
    await request;
    expect(toolActivityIconSources(client, appIcon)).toEqual({ iconLight: '', iconDark: '' });
  });
  it('shows fallback after a failed asset request without retrying every refresh', async () => {
    let calls = 0;
    const client = fixture([appIcon], async () => { calls++; throw new Error('unavailable'); });
    await syncToolActivityIcons(client, native, 1);
    await syncToolActivityIcons(client, native, 2);
    expect(calls).toBe(1);
    expect(toolActivityIconSources(client, appIcon).iconLight).toBe('');
  });
});
