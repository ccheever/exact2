// browser-surface part 5: a Browser tab's Mute / Unmute row and its audible indicator (MIT reference, see
// LICENSE-T3, T3 Code 1e2ecbd975: apps/web/src/components/RightPanelTabs.tsx `tabMuteMenuItem`, `tabAudioState`,
// the tab's Volume2 / VolumeOff button; previewBridge.setAudioMuted). WebKit has no public page mute: the module
// mutes the page's media elements and hears their playback (T3BrowserAutomation.swift `setMuted`, X1 path B).
import type { T3Client } from './client';
import type { Surface } from './r4-surfaces-panel';
import { parseScopedThreadKey } from './terminal-ui-state';
import { browserHost, effectiveNav } from './browser-surface';
import { tabAudioState, tabMuteMenuItem, type TabAudioState } from './right-panel-tabs';

function overlayOf(client: T3Client, surface: Surface) {
  const ref = surface.browser ? parseScopedThreadKey(surface.browser.threadKey) : null;
  if (!ref || !surface.browser) return null;
  const { tab, snapshot, runtimeId } = effectiveNav(client, ref, surface.browser.tabId);
  const overlay = snapshot ? browserHost(client).store.read(ref).desktopByTabId[snapshot.tabId] ?? null : null;
  return { overlay: tab ? overlay : null, runtimeId };
}
/** The tab menu's Mute row for a Browser tab; disabled until its page exists. */
export function browserTabMute(client: T3Client, surface: Surface): { label: string; disabled: boolean } | null {
  if (surface.kind !== 'browser') return null;
  const resolved = overlayOf(client, surface);
  return tabMuteMenuItem({ overlay: resolved?.overlay ?? null, canResolveRuntimeTabId: !!resolved?.runtimeId });
}
/** The tab's audio button: none, audible (Volume2, "Mute tab") or muted (VolumeOff, "Unmute tab"). */
export function browserTabAudio(client: T3Client, surface: Surface): TabAudioState {
  return surface.kind === 'browser' ? tabAudioState(overlayOf(client, surface)?.overlay ?? null) : 'none';
}
