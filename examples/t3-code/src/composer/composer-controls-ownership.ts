// Where the permissions chip shows, whether the mode block overflows, and which control answers composer.mode.
// Adapted from T3 Code 2359643cff (MIT; see LICENSE-T3): chat/composerControlsOwnership.ts, ported as is. The model
// picker's footer always holds Permissions, so the chip only shows while the mode differs from the default. The rule:
// composer.mode has one target.
export function resolveComposerControlsOwnership(input: {
  /** Permissions hide while the mode is the default, or while the new-thread strip names it. */
  readonly matchesDefault: boolean;
  readonly planModeUiEnabled: boolean;
  readonly collapsed: boolean;
  /** How many trailing blocks the width measurement moved into the overflow menu. */
  readonly measuredHiddenBlockCount: number;
}) {
  const accessDisplay: 'label' | 'icon' | 'hidden' = input.matchesDefault ? 'hidden' : input.collapsed ? 'icon' : 'label';
  const showModeBlock = accessDisplay !== 'hidden' || input.planModeUiEnabled;
  const modeHidden = showModeBlock && input.measuredHiddenBlockCount > 0;
  const hiddenBlockIds: Array<'mode'> = modeHidden ? ['mode'] : [];
  // An overflowed chip needs a home for Permissions only when the Plan toggle is not there to fill the overflow menu;
  // otherwise Permissions stay in the model picker.
  const overflowShowsPermissions = modeHidden && accessDisplay !== 'hidden' && !input.planModeUiEnabled;
  return {
    accessDisplay, showModeBlock, hiddenBlockCount: hiddenBlockIds.length, hiddenBlockIds, modeHidden, overflowShowsPermissions,
    /** Without a visible chip or an overflow menu holding it, the model picker answers composer.mode. */
    pickerOwnsModeShortcut: !overflowShowsPermissions && (accessDisplay === 'hidden' || modeHidden),
  };
}
