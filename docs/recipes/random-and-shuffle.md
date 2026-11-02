# Recipe: randomness and a shuffle

Rolls in the view: [random-rolls](random-rolls.md). The data module refuses
`Math.random()`. Deal from `exactTime()`'s `seed` (below 2^32, new each launch, `seed 2` in a
test), a stream for each thing dealt: the questions' order from the game, each question's
answers from the game and the round. One stream for both deals every question the
same shuffle, its right answer in one place all game. Keep the right answer by identity
(`correct`), never by index.

```contract
shape Choice
  id: string
  text: string
  correct: bool

shape Question
  text: string
  choices: Choice[]

shape Time
  seed: number

component Quiz
  state game = 0
  state round = 0
  resource time = exactTime() as shape Time
  resource q = deal(game, round, time.seed) as shape Question else empty()
  derive order = q.choices.map(c => c.text).join(" ")
  action next
    round = round + 1
  view
    list appearance="auto" listStyle="inset-grouped" height="100%"
      section
        header
          text q.text
        for c in q.choices key=c.id
          button press=next testId=`choice-${c.id}`
            text c.text
```

```ts
import type { Answer, Sources } from './app.contract.d.ts';

export const grants = '';
const QUESTIONS = [ // the right answer first
  ['Largest planet?', 'Jupiter', 'Mars', 'Venus', 'Mercury'],
  ['Capital of France?', 'Paris', 'Rome', 'Madrid', 'Berlin'],
];
const generator = (a: number) => () => {
  a = (a + 0x6d2b79f5) | 0;
  let t = Math.imul(a ^ (a >>> 15), 1 | a);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
// Math.imul, not *: past 2^53 a double loses the keys.
const stream = (seed: number, ...keys: number[]) =>
  generator(keys.reduce((a, k) => Math.imul(a ^ (k + 1), 0x9e3779b1) >>> 0, seed >>> 0));
const shuffled = <T,>(xs: T[], next: () => number) =>
  xs.map((x) => [next(), x] as const).sort((p, q) => p[0] - q[0]).map(([, x]) => x);
const sources: Sources = {
  deal: ([game, round, seed]) => {
    const [text, ...answers] = shuffled(QUESTIONS, stream(seed, game))[round % QUESTIONS.length];
    const choices = answers.map((a, i) => ({ id: a, text: a, correct: i === 0 }));
    return { text, choices: shuffled(choices, stream(seed, game, round)) };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
seed 2

test "each question's answers in an order of their own"
  expect state order == "Mars Jupiter Venus Mercury"
  expect state q.choices.1.correct == true
  tap "choice-Mars"
  clock data
  expect state order == "Rome Madrid Paris Berlin"
  expect state q.choices.2.correct == true
  reload
  expect state order == "Mars Jupiter Venus Mercury"
```

- A source is asked again, so `round + 1` deals again, `game + 1` a new game.
- `crypto.getRandomValues(new Uint32Array(n))` in the source: a new order each ask (assert what
  holds for any); one pick per launch: `xs.at(time.seed % xs.length)`.
