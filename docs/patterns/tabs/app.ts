import type { Answer, Sources } from './app.contract.d.ts';

export const appId = 'com.example.starter';
export const grants = '';

const sources: Sources = {
  loadPlaces: () => [
    { id: '1', name: 'Dolores Park', area: 'Mission', about: 'A sunny hillside with a view of downtown.' },
    { id: '2', name: 'Lands End', area: 'Outer Richmond', about: 'Cliffside trails above the Golden Gate.' },
    { id: '3', name: 'Ferry Building', area: 'Embarcadero', about: 'A market hall on the bay, busiest on Saturdays.' },
  ],
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
