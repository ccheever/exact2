# Recipe: randomness and a shuffle

The data module refuses `Math.random()`. Deal from `exactTime()`'s `seed`: a whole number
below 2^32, new at each launch, fixed under the driver (`seed 2` in a test), with the round
mixed in. A quiz shuffles its answers too and keeps the right one by identity (`correct`,
set before the shuffle), never by index.

```contract
shape Choice
  id: string
  text: string
  correct: bool

shape Question
  text: string
  choices: list<Choice>

shape Time
  seed: number

component Quiz
  state round = 0
  state picked = ""
  resource time = exactTime() as shape Time
  resource q = deal(round, time.seed) as shape Question else empty()
  derive order = join(map(q.choices, c => c.text), " ")
  action pick(id: string)
    picked = id
  action next
    round = round + 1
    picked = ""
  view
    list appearance="auto" listStyle="inset-grouped" height="100%"
      section
        header
          text q.text
        each c in q.choices key=c.id
          button press=pick(c.id) testId=`choice-${c.id}`
            text c.text flex=1
            when picked != "" and c.correct
              image "symbol:checkmark"
      section
        button press=next testId="next"
          text "Next"
```

```ts
import type { Answer, Sources } from './app.contract.d.ts';

export const appId = 'com.example.quiz'; // exact new writes this line
export const grants = '';
const QUESTIONS = [ // the right answer first; `correct` keeps it
  ['Largest planet?', 'Jupiter', 'Mars', 'Venus', 'Mercury'],
  ['Capital of France?', 'Paris', 'Rome', 'Madrid', 'Berlin'],
];
const generator = (a: number) => () => {
  a = (a + 0x6d2b79f5) | 0;
  let t = Math.imul(a ^ (a >>> 15), 1 | a);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const shuffled = <T,>(xs: T[], next: () => number) =>
  xs.map((x) => [next(), x] as const).sort((p, q) => p[0] - q[0]).map(([, x]) => x);
const sources: Sources = {
  deal: ([round, seed]) => {
    const next = generator((seed + Math.imul(round, 0x9e3779b1)) >>> 0); // Math.imul, not *: past 2^53 a double loses the round
    const [text, ...answers] = shuffled(QUESTIONS, next)[0];
    const choices = answers.map((a, i) => ({ id: a, text: a, correct: i === 0 }));
    return { text, choices: shuffled(choices, next) };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
seed 2

test "each round deals its order; the right answer moves with its text"
  expect state order == "Paris Rome Berlin Madrid"
  expect state q.choices.0.correct == true
  tap "next"
  clock data
  expect state order == "Madrid Berlin Rome Paris"
  expect state q.choices.3.correct == true
  reload
  expect state order == "Paris Rome Berlin Madrid"
```

- A source is asked again, so `round + 1` deals again.
- `crypto.getRandomValues(new Uint32Array(n))` in the source: a new order each ask (assert what
  holds for any); one pick per launch: `at(xs, time.seed % length(xs))`.
