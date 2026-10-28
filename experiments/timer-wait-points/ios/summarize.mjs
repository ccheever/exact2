// Prints each scenario's per-phase counts across repetitions from results/*.json: `bun summarize.mjs [scenario]`
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import process from 'node:process';

const directory = join(import.meta.dirname, 'results');
const only = process.argv[2];

const reports = readdirSync(directory)
  .filter((name) => name.endsWith('.json') && !name.endsWith('.events.json'))
  .map((name) => JSON.parse(readFileSync(join(directory, name), 'utf8')))
  .filter((report) => !only || report.scenario === only)
  .sort((a, b) => a.scenario.localeCompare(b.scenario) || a.repetition - b.repetition);

// Keep the control, which runs with no UI test attached, apart from the test's runs
const byScenario = Map.groupBy(reports, (report) => `${report.scenario} (${report.environment.driver ?? 'xctest'})`);
for (const [scenario, runs] of byScenario) {
  console.log(`\n## ${scenario}, ${runs.length} runs; ${JSON.stringify(runs[0].environment)}`);
  const phaseNames = [...new Set(runs.flatMap((run) => run.summary.phases.map((phase) => phase.name)))];
  for (const name of phaseNames) {
    const phases = runs.map((run) => run.summary.phases.find((phase) => phase.name === name));
    const durations = phases.map((phase) => (phase ? `${Math.round(phase.duration * 1000)}` : '-'));
    console.log(`${name} (ms: ${durations.join(', ')})`);
    const kinds = Object.keys(phases.find(Boolean).counts);
    for (const kind of kinds) {
      const counts = phases.map((phase) => phase?.counts[kind] ?? '-');
      const intervals = phases.map((phase) => phase?.medianIntervalMs[kind] ?? '-');
      console.log(`  ${kind.padEnd(28)} counts ${counts.join(', ').padEnd(16)} median interval ms ${intervals.join(', ')}`);
    }
  }
  for (const run of runs) {
    for (const line of backgroundTimeline(run)) {
      console.log(line);
    }
    if (run.summary.ordering) {
      console.log(`ordering, run ${run.repetition}: ${JSON.stringify(run.summary.ordering, null, 1)}`);
    }
  }
}

// For each gesture during which the app went to the background, when that happened relative to the deceleration:
// times in ms from the gesture's release (`didEndDragging`), and the scroll state at `didEnterBackground`
function backgroundTimeline(run) {
  const file = join(directory, `${run.environment.driver === 'control' ? 'control-' : ''}${run.scenario}-${run.repetition}.events.json`);
  const events = JSON.parse(readFileSync(file, 'utf8'));
  const marks = events.filter((event) => event.kind === 'gesture');
  const lines = [];
  for (const [index, mark] of marks.entries()) {
    const end = marks[index + 1]?.t ?? Infinity;
    const inside = events.filter((event) => event.t >= mark.t && event.t < end);
    const first = (kind) => inside.find((event) => event.kind === kind);
    const background = first('didEnterBackground');
    const release = first('didEndDragging');
    if (!background || !release) {
      continue;
    }
    const at = (event) => (event ? `${Math.round((event.t - release.t) * 1000)}` : 'none');
    const foreground = first('willEnterForeground');
    const scrollsInBackground = inside.filter((event) => event.kind === 'scroll' && event.t > background.t && (!foreground || event.t < foreground.t)).length;
    const state = background.scroll;
    lines.push(
      `run ${run.repetition} ${mark.note}: willDeactivate ${at(first('willDeactivate'))}, didEnterBackground ${at(background)} ` +
        `(decelerating ${state.decelerating}, dragging ${state.dragging}), didEndDecelerating ${at(first('didEndDecelerating'))}, ` +
        `willEnterForeground ${at(foreground)}, scroll callbacks in the background ${scrollsInBackground}`,
    );
  }
  return lines;
}
