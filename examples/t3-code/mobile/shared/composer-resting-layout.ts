// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/composer-resting-layout.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// How the composer's footer controls fit their host: labels drop first, then
// trailing blocks move into the "More composer controls" menu, then the model
// picker shrinks, and below its minimum the cluster hides (task
// composer-fidelity, G11). Adapted from T3 Code 1e2ecbd975 (MIT); see
// LICENSE-T3. Source: apps/web/src/components/composerFooterLayout.ts
// (RestingComposerControlsMeasurement, resolveRestingComposerControlsNaturalWidth,
// resolveRestingComposerControlsLayout, RESTING_CONTROLS_SLACK_PX).
// Changes: none in the logic; the widths come from the clone's toolbar geometry
// and measured labels (composer-controls-view.ts footerLayout,
// r5-composer-measure.ts) instead of the DOM (restingComposerControlsMeasurement.ts).

export interface RestingComposerControlsMeasurement {
  gap: number;
  naturalFixedWidth: number;
  minimumFixedWidth: number;
  blockWidths: readonly number[];
  overflowWidth: number;
  iconOnlyBlockWidths?: readonly number[];
}
export type RestingComposerControlsLayout = { hiddenCount: number; iconOnlyCount?: number; visible: boolean };

function controlsWidth(input: RestingComposerControlsMeasurement, hiddenCount: number, fixedWidth = input.naturalFixedWidth, iconOnlyCount = 0): number {
  const { blockWidths, gap } = input;
  const visibleCount = blockWidths.length - hiddenCount;
  return fixedWidth
    + blockWidths.slice(0, visibleCount).reduce((sum, width, index) => sum + (index >= blockWidths.length - iconOnlyCount ? (input.iconOnlyBlockWidths?.[index] ?? width) : width), 0)
    + (hiddenCount > 0 ? input.overflowWidth : 0)
    + gap * (visibleCount + (hiddenCount > 0 ? 1 : 0));
}

/** The width the controls take with nothing moved into overflow (what the context strip reserves). */
export function resolveRestingComposerControlsNaturalWidth(input: RestingComposerControlsMeasurement): number {
  return controlsWidth(input, 0);
}

/** Promotions need a point of slack; demotions are immediate, so a threshold cannot flip between layouts. */
const RESTING_CONTROLS_SLACK_PX = 1;

export function resolveRestingComposerControlsLayout(input: RestingComposerControlsMeasurement & {
  hostWidth: number; previous?: RestingComposerControlsLayout }): RestingComposerControlsLayout {
  const { blockWidths, hostWidth, previous } = input;
  const iconSteps = input.iconOnlyBlockWidths ? blockWidths.length : 0;
  const previousStep = previous
    ? previous.hiddenCount > 0 ? iconSteps + Math.min(previous.hiddenCount, blockWidths.length) : Math.min(previous.iconOnlyCount ?? 0, iconSteps)
    : 0;
  const widthAtStep = (candidate: number, fixedWidth = input.naturalFixedWidth) =>
    controlsWidth(input, Math.max(0, candidate - iconSteps), fixedWidth, Math.min(candidate, iconSteps));
  let step = 0;
  while (step < iconSteps + blockWidths.length && widthAtStep(step) > hostWidth - (step < previousStep ? RESTING_CONTROLS_SLACK_PX : 0)) step += 1;
  const hiddenCount = Math.max(0, step - iconSteps);
  const iconOnlyCount = Math.min(step, iconSteps);
  const minimumWidth = widthAtStep(step, input.minimumFixedWidth);
  const visible = previous && !previous.visible ? minimumWidth <= hostWidth - RESTING_CONTROLS_SLACK_PX : minimumWidth <= hostWidth;
  return { hiddenCount, ...(input.iconOnlyBlockWidths ? { iconOnlyCount } : {}), visible };
}
