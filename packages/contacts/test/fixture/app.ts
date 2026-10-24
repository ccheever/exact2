import type { Answer, Sources } from './app.contract.d.ts';
import { queryContacts } from '@exact/contacts';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'device.contacts purpose.contacts';

const failed = (e: unknown) => `error ${(e as { code?: string }).code}`;

const sources: Sources = {
  // Every page of a query, `limit` at a time: the ids in order, then how many pages.
  walk: async ([text, limit]) => {
    try {
      const ids: string[] = [];
      let after: string | undefined, pages = 0;
      do {
        const page = await queryContacts({ text: text || undefined, limit, after, properties: ['name', 'email', 'tel'] });
        pages++;
        ids.push(...page.contacts.map((c) => c.id ?? '?'));
        after = page.next ?? undefined;
      } while (after && pages < 50);
      return { text: `${ids.join(',')} ${pages}` };
    } catch (e) {
      return { text: failed(e) };
    }
  },
  // One page, after the cursor the Contract keeps.
  page: async ([limit, after]) => {
    try {
      const page = await queryContacts({ limit, after: after || undefined, properties: ['name'] });
      return { text: page.contacts.map((c) => c.id ?? '?').join(','), next: page.next ?? '' };
    } catch (e) {
      return { text: failed(e), next: '' };
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
