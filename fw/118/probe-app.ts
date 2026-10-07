import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.probe';
export const grants = '';

const tags = ['en-US', 'de-DE', 'ar-EG', 'ar-SA', 'ja-JP', 'ko-KR', 'en-GB', 'fa-IR'];
const sources: Sources = {
  probe: (): Result<'probe'> => [
    ...tags.map(tag => {
      try { return { k: tag, v: JSON.stringify(new (Intl as any).Locale(tag).getWeekInfo()) }; }
      catch (e) { return { k: tag, v: `THROWS ${(e as Error).name}: ${(e as Error).message}` }; }
    }),
    (() => {
      try { const l = new (Intl as any).Locale('EN-latn-us-u-ca-gregory-hc-h12'); return { k: 'parse', v: `${l} ${l.language}/${l.script}/${l.region} ${l.hourCycle}` }; }
      catch (e) { return { k: 'parse', v: `THROWS ${(e as Error).name}` }; }
    })(),
  ],
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
