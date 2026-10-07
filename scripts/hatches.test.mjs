// Access hatches in the build scripts (LLP 1075.003.000.001 §4.3, §5): which
// words a platform's module handles, and the delivery check over them.
// `bun test ./scripts/hatches.test.mjs`.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { classifyArtifacts, hatchWords } from './app.mjs';

test('a platform handles every word of a list, and of an object the words that name it', () => {
  assert.deepEqual(hatchWords({ hatches: ['dot', 'avatar'] }, 'linux'), ['avatar', 'dot']);
  const manifest = { hatches: { dot: ['web', 'ios'], avatar: ['ios', 'macos'], nowhere: [] } };
  assert.deepEqual(hatchWords(manifest, 'ios'), ['avatar', 'dot']);
  assert.deepEqual(hatchWords(manifest, 'web'), ['dot']);
  assert.deepEqual(hatchWords(manifest, 'linux'), []);
  assert.deepEqual(hatchWords({}, 'ios'), []);
});

// LLP 1075.003.000.001 §4.3: a plan's hatch words are a requirement per
// platform, against what the installed cohort's module was built to handle.
test('a bundle whose plan needs a hatch word the installed module lacks is not published to that stream', () => {
  const plan = (hatches) => ({ name: 'app.plan', requires: hatches ? { hatches } : {} });
  const candidate = (hatches) => ({ binary: { sha256: 'same' }, graph: { artifacts: [plan(hatches)], sources: {} }, compat: { inputs: {} } });
  const cohort = (hatches) => ({ binary: 'same', sources: {}, compat: { id: 'c', inputs: { store: { L: 'A' }, ...(hatches !== undefined ? { hatches } : {}) } } });
  // An Apple cohort built without `avatar`: no bundle, and the word is named.
  const refused = classifyArtifacts(candidate(['avatar', 'dot']), cohort(['dot']));
  assert.equal(refused.bundle, false);
  assert.deepEqual(refused.missing, ["app.plan: hatches.avatar (handled by this platform's module, not by the installed one)"]);
  // A cohort from before the capability existed handles nothing.
  assert.equal(classifyArtifacts(candidate(['dot']), cohort(undefined)).bundle, false);
  assert.equal(classifyArtifacts(candidate(['dot']), cohort(null)).bundle, false);
  // The same plan for a platform that does not handle the word asks nothing of it: its Linux cohort takes the bundle.
  assert.equal(classifyArtifacts(candidate(null), cohort(null)).bundle, true);
  // A cohort that handles more than the plan marks is fine.
  assert.equal(classifyArtifacts(candidate(['dot']), cohort(['avatar', 'dot'])).bundle, true);
});
