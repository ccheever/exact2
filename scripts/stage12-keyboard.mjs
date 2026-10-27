// stage12-gate.mjs's iOS keyboard rule (theme2's s15b ruling: the composer's keyboard state deterministic on both
// sides), apart from the gate script for its line cap.

/** The screens whose shot needs iOS's software keyboard up. It comes up on the platform's clock and its service drops
 * under load, so a drive with it down (or failed) is driven again, twice at most, and a kept drive with it down fails.
 * A fixture refusal is never driven again. */
export const KEYBOARD_UP = ['composer'];

/** The software keyboard's height in a drive's output, from the reply naming testId `target`; null with no reply. */
export function keyboardInset(target, out) {
  const line = out.split('\n').findLast(l => l.startsWith('{') && l.includes(`"testId":"${target}"`));
  return line ? (JSON.parse(line).env?.['keyboard-inset-height'] ?? 0) : null;
}

/** Drive `r` (`{ ok, out }`), driven again by `again` while the keyboard is down or the drive failed (and refused
 * nothing: `refused` is the fixture's log word): the kept drive, whether its keyboard is down, and what was redone. */
export function keyboardRedrive(target, refused, r, again) {
  const down = x => x.ok && !x.out.includes(refused) && keyboardInset(target, x.out) === 0;
  const redone = [];
  for (let k = 0; k < 2 && (down(r) || (!r.ok && !r.out.includes(refused))); k++) {
    redone.push(r.ok ? 'keyboard down' : 'drive failed');
    r = again();
  }
  const inset = r.ok ? keyboardInset(target, r.out) : null;
  const kept = !r.ok ? 'drive failed' : r.out.includes(refused) ? 'fixture refused' : inset === null ? 'no field reply' : inset > 0 ? 'keyboard up' : 'keyboard down';
  return { r, down: down(r), keyboard: redone.length ? `driven ${redone.length + 1} times (${redone.join(', ')} before), kept: ${kept}` : '' };
}
