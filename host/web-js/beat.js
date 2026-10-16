// A live clock's successor (the timers RFC, D3), as the runner's `beat_after`
// (runner/src/runner/advance.rs): the first beat of a repeat's phase
// (`beat + k·ms`) strictly after `t`, and after `beat`. The division can round
// the count either way, so it is corrected until beat k is after `t` and beat
// k − 1 is not.
export function beatAfter(beat, ms, t) {
  let k = Math.max(Math.floor((t - beat) / ms) + 1, 1);
  while (k > 1 && beat + (k - 1) * ms > t) k--;
  while (beat + k * ms <= t) k++;
  return beat + k * ms;
}
