# Recipe: randomness and a shuffle

The data module refuses `Math.random()`. Draw randomness with `crypto.getRandomValues`
in a source, and ask again for a new draw by changing the source's argument. A
shuffle seeded with a constant deals the same game on every launch.

```contract
routes nav
  home "/"

shape Card
  id: string
  word: string

shape Deck
  cards: list<Card>

component Flashcards
  state round = 0
  resource deck = deal(round) as shape Deck else empty()
  derive dealt = length(deck.cards)
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

/** Fisher–Yates over secure random numbers: a new order on every call. */
function shuffled<T>(items: T[]): T[] {
  const out = [...items], r = crypto.getRandomValues(new Uint32Array(out.length));
  for (let i = out.length - 1; i > 0; i--) {
    const j = r[i] % (i + 1);
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}

const sources: Sources = {
  deal: () => ({ cards: shuffled(WORDS.map((word, i) => ({ id: String(i), word }))) }),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "a shuffle deals the whole deck again"
  expect state dealt == 5
  tap "shuffle"
  clock data
  expect state dealt == 5
  expect tree has "card-0"
```

- A source is asked again when its arguments change, so `deal(round)` with `round =
  round + 1` is a new shuffle; `refresh deck` is another. The argument itself is unused.
- `crypto.randomUUID()` makes an id, and `crypto.getRandomValues` any other random
  value (a password's characters, a dice roll), each inside a source.
- For one pick per launch in Contract, `exactTime()`'s `seed` is a whole number drawn
  from secure entropy at each launch: `at(xs, time.seed % length(xs))`. The driver's
  seed is 1 (`seed 7` in a test), so that pick is repeatable in tests.
- Real randomness isn't repeatable, so test what holds for every order: the count,
  that each card is there, that answering scores.
