# Recipe: randomness and a shuffle

The data module refuses `Math.random()`. Deal from the launch seed, `exactTime()`'s
`seed`: a whole number below 2^32, drawn from secure entropy at each launch and fixed
under the driver (1, or `seed 7` in a test), so each launch deals a new game and a test
can name the exact order. Pass the round with it, and mix the two in 32-bit integer
steps in the source. A shuffle seeded with a constant deals the same game on every
launch.

```contract
routes nav
  home "/"

shape Card
  id: string
  word: string

shape Deck
  cards: list<Card>

shape Time
  seed: number

component Flashcards
  state round = 0
  resource time = exactTime() as shape Time
  resource deck = deal(round, time.seed) as shape Deck else empty()
  derive order = join(map(deck.cards, c => c.word), " ")
  action shuffle
    round = round + 1
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="cards" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" align-items="center" justify-content="space-between" padding="8px 16px"
            text "Cards" role="heading" aria-level=1
            button press=shuffle testId="shuffle"
              text "Shuffle"
          list id="cards" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              each c in deck.cards key=c.id
                row
                  text c.word testId=`card-${c.id}`
```

```ts
import type { Answer, Sources } from './app.contract.d.ts';

export const appId = 'com.example.flashcards'; // exact new writes this line
export const grants = '';

const WORDS = ['apple', 'river', 'cloud', 'stone', 'ember'];

/** The round mixed into the seed in 32-bit steps, each exact: `Math.imul` multiplies
 *  modulo 2^32, `>>> 0` keeps the result unsigned. */
const mix = (seed: number, round: number) =>
  Math.imul(seed ^ Math.imul(round, 0x9e3779b1), 0x85ebca6b) >>> 0;

/** mulberry32: a small 32-bit generator, a number in [0, 1) per call. */
function generator(a: number): () => number {
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Fisher–Yates: one order per seed and round. */
function shuffled<T>(items: T[], next: () => number): T[] {
  const out = [...items];
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(next() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}

const sources: Sources = {
  deal: ([round, seed]) => ({
    cards: shuffled(WORDS.map((word, i) => ({ id: String(i), word })), generator(mix(seed, round))),
  }),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
seed 7

test "each round deals its own order, and a round deals the same order again"
  expect state order == "stone river cloud ember apple"
  tap "shuffle"
  clock data
  expect state order == "ember stone river cloud apple"
  reload
  expect state order == "stone river cloud ember apple"
```

- A source is asked again when its arguments change, so `round = round + 1` is a new
  deal, and the same round and seed (a `reload` under the driver) the same deal. A
  real launch draws a new seed, so a new game.
- The seed is a whole number below 2^32, so `%` and `>>> 0` take all of it. Mix the
  round in only with `mix`'s 32-bit steps (`Math.imul`, `^`, `>>> 0`): a multiply such
  as `seed * 2654435761` passes 2^53, where a double drops the round.
- For a new order on every ask, even of the same round, draw in the source with
  `crypto.getRandomValues(new Uint32Array(n))`; `crypto.randomUUID()` makes an id.
  Neither is repeatable, so test what holds for every order (the count, that each
  card is there).
- For one pick per launch in Contract: `at(xs, time.seed % length(xs))`.
