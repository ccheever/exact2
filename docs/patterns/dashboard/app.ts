import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.starter';
export const grants = ''; // a server's data: `net.fetch <origin>` here, and fetch in the source

type Category = Result<'loadSummary'>['categories'][number];
const month: Category[] = [
  { id: 'groceries', name: 'Groceries', spent: 412.5, budget: 450 },
  { id: 'dining', name: 'Eating Out', spent: 286, budget: 200 },
  { id: 'transport', name: 'Transport', spent: 96.4, budget: 150 },
];
const recent = [
  { id: '1', title: 'Corner Market', detail: 'Groceries · Today', amount: 23.4 },
  { id: '2', title: 'Noodle Bar', detail: 'Eating Out · Yesterday', amount: 31 },
  { id: '3', title: 'Bus Pass', detail: 'Transport · Monday', amount: 45 },
];

const sources: Sources = {
  // The view has no reduce or sort: the data module adds and orders; the view shows.
  loadSummary: ([period]) => period === 'week' ? { spent: 0, budget: 200, categories: [], recent: [] }
    : { spent: month.reduce((sum, c) => sum + c.spent, 0), budget: 800, categories: month, recent },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
