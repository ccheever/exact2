# Recipe: random rolls in an expression

Contract has no ambient randomness (LLP 1027.000 D1): a draw comes from a seed passed to
a pure function. A deal or a shuffle is the data module's
([random-and-shuffle](random-and-shuffle.md)); dozens of rolls a second, dice under a held
finger, need no source. `roll(seed, k, n)` is a whole number from 0 to `n - 1`, the `k`th
draw of the stream `seed` names. Copy its three `fn`s. They use only integer `*`, `%` and
`floor`, every product below 2^47, then one division, so the runner, the web and the
semantics roll the same. A `sin` hash would not: the web's `Math.sin` need not match the
runner's bits. The state is a counter, so the same seed and count roll the same again.

```contract
shape Time
  seed: number

fn turn(x: number): number = (x % 65536) * 32768 + floor(x / 65536)
fn stir(x: number): number = turn((x * (16807 + floor(x / 65536))) % 2147483647)
fn roll(seed: number, k: number, n: number): number = floor(stir(stir(stir(1 + (stir(stir(1 + seed % 2147483646)) + k) % 2147483646))) * n / 2147483647)

component Dice
  state rolls = 0
  state shaking = false
  resource time = exactTime() as shape Time
  derive die = 1 + roll(time.seed, rolls, 6)
  task shake when shaking
    every(50, next)
  action next
    rolls = rolls + 1
  action hold(on: bool)
    shaking = on
  view
    column align-items="center" gap=16 padding=16
      text `${die}` font-size="4rem" font-variant-numeric="tabular-nums" testId="die"
      button pointerdown=hold(true) pointerup=hold(false) testId="roll"
        text "Hold to roll"
```

```contract-test
seed 2

test "a hold rolls; the same seed rolls the same"
  expect text "die" == "2"
  tap "roll" drag 0 0 hold 500
  expect state rolls == 10
  expect text "die" == "6"
  clock +500
  expect state rolls == 10
  reload
  expect text "die" == "2"
```

- `stir` is a Lehmer step (modulus 2^31 − 1) whose multiplier depends on the state, then a
  rotation of its 31 bits; both keep 1 … 2^31 − 2 in range. Two stirs scatter the seed and
  three the count, so consecutive counts give unrelated draws.
- A second stream takes a second seed: `roll(time.seed + 1, k, n)`.
- A game's dice, not a secret: that is `crypto.getRandomValues` in a source.
