import {test,expect} from 'bun:test';
import {open} from '../../../scripts/agent.mjs';

test('Caltrain web frame-only stack remains a plain column',async()=>{
  const s=await open({host:'web'});
  try {
    await s.clock('settle');await s.tap('deck-toggle');await s.clock('settle');
    const cards=(await s.layout()).nodes.filter(n=>n.testId?.startsWith('card-'));
    expect(cards.length).toBeGreaterThan(1);
    for(let i=1;i<cards.length;i++) {
      expect(Math.abs(cards[i].x-cards[0].x)).toBeLessThan(.1);
      expect(Math.abs(cards[i].w-cards[0].w)).toBeLessThan(.1);
      expect(cards[i].y).toBeGreaterThanOrEqual(cards[i-1].y+cards[i-1].h-.1);
    }
  } finally {await s.close();}
},60000);
