import type { Answer, Sources } from './app.contract.d.ts';
import { calendarEvents, calendars } from '@exact/calendar';

// The suite writes the id `exact new` gave the app, and may replace the grants.
export const appId = '__APP_ID__';
export const grants = 'device.calendar-write-only purpose.calendar\ndevice.calendar purpose.calendar';

/** 2026-09-01T00:00:00Z. */
const SEPTEMBER = Date.UTC(2026, 8, 1), DAY = 86_400_000;
const failed = (e: unknown) => {
  const { code, message } = e as { code?: string; message?: string };
  return `error ${code} ${message ?? ''}`.trim();
};
const key = (e: { id?: string | null; recurrenceId?: string | null }) => `${e.id ?? '?'}${e.recurrenceId ? `@${e.recurrenceId.slice(0, 10)}` : ''}`;

const sources: Sources = {
  calendarList: async () => {
    try {
      return { text: (await calendars({})).calendars.map((c) => `${c.id}${c.writable ? '' : '(ro)'}${c.isDefault ? '*' : ''}`).join(',') };
    } catch (e) {
      return { text: failed(e) };
    }
  },
  // Every occurrence from 1 September for `days` days, `limit` a page: the keys in order, then how many pages.
  walk: async ([days, limit]) => {
    try {
      const keys: string[] = [];
      let after: string | undefined, pages = 0;
      do {
        const page = await calendarEvents({ start: SEPTEMBER, end: SEPTEMBER + days * DAY, limit, after });
        pages++;
        keys.push(...page.events.map(key));
        after = page.next ?? undefined;
      } while (after && pages < 50);
      return { text: `${keys.join(',')} ${pages}` };
    } catch (e) {
      return { text: failed(e) };
    }
  },
  // One page of September and October, after the cursor the Contract keeps.
  page: async ([limit, after]) => {
    try {
      const page = await calendarEvents({ start: SEPTEMBER, end: SEPTEMBER + 61 * DAY, limit, after: after || undefined });
      return { text: page.events.map(key).join(','), next: page.next ?? '' };
    } catch (e) {
      return { text: failed(e), next: '' };
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
