import type { Answer, Sources } from './app.contract.d.ts';
import { ndefRecords, sendCommand, write } from '@exact/nfc';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'device.nfc purpose.nfc';

const failed = (e: unknown) => `error ${(e as { code?: string }).code}`;
const hex = (b: Uint8Array) => [...b].map((x) => x.toString(16).padStart(2, '0')).join('');

const sources: Sources = {
  // A reading's records with their bytes, as `type=hex` each.
  recordsOf: async ([reading]) => {
    try {
      const records = await ndefRecords({ reading });
      return { text: records.map((r) => `${r.recordType}=${hex(r.data)}`).join(' ') };
    } catch (e) {
      return { text: failed(e) };
    }
  },
  // A write of an app's own record, whose data is bytes.
  writeBytes: async () => {
    try {
      const done = await write({ message: { records: [{ recordType: 'example.com:fixture', mediaType: null, id: null, encoding: null, lang: null, data: new Uint8Array([1, 2, 3]) }] } });
      return { text: `written ${done.serialNumber}` };
    } catch (e) {
      return { text: failed(e) };
    }
  },
  // A tier 2 command to the last tag read: an ISO 7816 SELECT of no application.
  command: async ([tag]) => {
    try {
      const r = await sendCommand({ tag, apdu: new Uint8Array([0x00, 0xa4, 0x04, 0x00, 0x00]) });
      return { text: `${hex(r.data)} ${r.sw1} ${r.sw2}` };
    } catch (e) {
      return { text: failed(e) };
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
