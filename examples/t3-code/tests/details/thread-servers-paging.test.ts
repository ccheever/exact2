// ThreadServersPanel.browser.test.tsx at T3 Code v0.1.7 (#28684), by name: a long server list pages four at a time,
// collapses back, and starts again on another thread; the rows past four leave the density's height.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { DETAILS_FRAMES, measureThreadDetailsCard } from '../../src/details/thread-details-card-layout';
import { appText } from '../support/app-files';

const section = contractComponent('shell-details.contract', 'ThreadServersSection');
const servers = (thread: string, count = 10) => ({ thread, rows: Array.from({ length: count }, (_, index) => ({ key: `${index}:${3000 + index}` })) });

describe('the Servers section', () => {
  test('pages servers four at a time, collapses back, and resets on thread switch', () => {
    let state = section.state();
    const math = { max: Math.max, min: Math.min };
    const shown = (props: object) => section.derives(state, { ...math, servers: props });
    const page = (props: object) => { section.call('page', state, { ...math, servers: props }, []); };
    const a = servers('a');
    expect(shown(a)).toMatchObject({ visible: 4, hidden: 6 });
    page(a); expect(shown(a)).toMatchObject({ visible: 8, hidden: 2 });
    page(a); expect(shown(a)).toMatchObject({ visible: 12, hidden: 0 });
    page(a); expect(shown(a)).toMatchObject({ visible: 4, hidden: 6 });
    page(a); expect(shown(a)).toMatchObject({ visible: 8 });
    expect(shown(servers('b'))).toMatchObject({ visible: 4 });
    page(servers('b')); expect(shown(a)).toMatchObject({ visible: 4 });
  });
  test('reads "Show N more" while rows are hidden, then "Show less", only past four servers', () => {
    const card = appText('src/details/shell-details.contract').split('\n');
    const more = card.findIndex(line => line.includes('testId="details-servers-more"'));
    expect(card[more - 1]!.trim()).toBe('if servers.rows.length > 4');
    expect(card[more]).toContain('press=page');
    expect(card[more + 2]).toContain('(hidden > 0 ? `Show ${min(hidden, 4)} more` : "Show less")');
  });
  test('the rows past four leave the height the density fits', () => {
    const frame = (height: number) => [0, 0, 300, height];
    const base = { [`${DETAILS_FRAMES.content}`]: frame(400), [DETAILS_FRAMES.fullTop]: frame(100), [DETAILS_FRAMES.fullTail]: frame(200) };
    const paged = measureThreadDetailsCard({ ...base, [DETAILS_FRAMES.serverOverflow]: frame(160) });
    expect(paged.full).toBe(measureThreadDetailsCard(base).full! - 160);
  });
});
