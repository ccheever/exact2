import type { Answer, Sources } from './app.contract.d.ts';
import { closeNotification, notificationStatus, showNotification } from '@exact/notifications';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'device.notifications purpose.notifications';

const sources: Sources = {
  // A post from data code, as the T3 Code clone's thread notifications are: only when granted.
  fromData: async () => {
    const { state } = await notificationStatus({});
    if (state !== 'granted') return { text: `not posted: ${state}` };
    const done = await showNotification({ title: 'From data', tag: 'data-1', silent: true, data: '/thread/9' });
    return { text: done === undefined ? 'posted' : 'answered something' };
  },
  closeData: async () => {
    await closeNotification({ tag: 'data-1' });
    return { text: 'closed' };
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
