// The composer's file and image previews under the keyboard: Tab follows Keyboard Navigation (LLP 1115.000 D7.2) through
// ControlGuard and the inert window behind them, a press on the backdrop keeps the focus inside so closing returns it,
// and ← and → step through the draft's images as ExpandedImageDialog's onKeyDown does. What Tab reaches is the host's
// to decide, so these read the sources and run the actions; a drive presses the keys.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { contractComponent } from './contract-action-fixture';
import { contractFns } from './contract-fn-fixture';

const source = (name: string) => readFileSync(join(import.meta.dir, name), 'utf8');
const line = (file: string, needle: string) => source(file).split('\n').find(entry => entry.includes(needle))!;

describe('Tab follows Keyboard Navigation', () => {
  test('ControlGuard is a button with no tabindex, so it is a Tab stop exactly when the dialog\'s buttons are', () => {
    const lines = source('dialog-trap.contract').split('\n');
    const view = lines.slice(lines.indexOf('component ControlGuard')).find(entry => entry.trimStart().startsWith('button id=guardId'))!;
    expect(view).toContain('aria-hidden=true focusin=enter key=hold');
    expect(view).not.toContain('tabindex');
    expect(view).toContain('pointer-events="none"');
  });

  test('both previews trap with ControlGuard, and the window behind them is inert while either is open', () => {
    for (const [file, base] of [['composer-chip-file.contract', 'chip-file-preview'], ['composer-chip-image.contract', 'chip-image-preview']] as const) {
      expect(source(file)).toContain(`ControlGuard(guardId="${base}-guard-start", targets=dialogTargets, popup="${base}")`);
      expect(source(file)).toContain(`ControlGuard(guardId="${base}-guard-end", targets=dialogTargets, popup="${base}")`);
      expect(source(file)).not.toContain('FocusGuard(');
    }
    expect(line('app-window.contract', 'row width="100%" height="100%" min-width=0 inert=('))
      .toContain('inert=(modal or shell.panel.deviceSetup or shell.composerFile.open or data.composerImagePreview.length > 0)');
  });
});

describe('a press on the backdrop keeps the focus in the dialog, so closing gives it back', () => {
  test.each([['composer-chip-file.contract', 'chip-file-preview-backdrop'], ['composer-chip-image.contract', 'chip-image-backdrop']])('%s', (file, testId) => {
    const backdrop = line(file, `testId="${testId}"`);
    // A clicked button takes no focus on the Mac, whatever its tabindex: the focus stays on the dialog's control.
    expect(backdrop).toContain('tabindex=-1 position="absolute"');
    expect(backdrop).not.toContain('retainFocus');
    // The file preview is aria-modal and its backdrop outside it, so its Escape is the dialog's Close.
    if (file === 'composer-chip-image.contract') expect(backdrop).toContain('aria-keyshortcuts="Escape"');
  });
});

describe('the image preview\'s ← and →', () => {
  const dialog = contractComponent('composer-chip-image.contract', 'ComposerImageDialog');
  const run = (many: boolean, k: string, modifiers: Record<string, boolean> = {}) => {
    const calls: string[] = [];
    const preview = { open: true, id: 'a', name: 'a.png', position: many ? '(1/2)' : '', many, width: 0, height: 0 };
    const scope = {
      preview, windowWidth: 1280, windowHeight: 840, rem: 16,
      ...dialog.derives({ zoom: 1, zoomAt: '' }, { preview, windowWidth: 1280, windowHeight: 840, rem: 16, min: Math.min, max: Math.max }),
      command: (op: string) => calls.push(op),
      dialogFallback: (popup: string, targets: string[], key: string) => calls.push(`fallback:${popup}:${targets.join(',')}:${key}`),
      preventDefault: () => calls.push('prevent'), stopPropagation: () => calls.push('stop'),
    };
    dialog.call('keys', { zoom: 1, zoomAt: '' }, scope, [k, { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...modifiers }]);
    return calls;
  };
  test('over several images they step as Previous and Next, with or without modifiers', () => {
    expect(run(true, 'ArrowLeft')).toEqual(['prevent', 'stop', 'editorlocal:chip-image-previous']);
    expect(run(true, 'ArrowRight')).toEqual(['prevent', 'stop', 'editorlocal:chip-image-next']);
    expect(run(true, 'ArrowRight', { shiftKey: true })).toEqual(['prevent', 'stop', 'editorlocal:chip-image-next']);
  });
  test('over one image they do nothing of the dialog\'s; Tab and other keys reach the trap\'s fallback', () => {
    expect(run(false, 'ArrowLeft')).toEqual(['fallback:chip-image-preview:,chip-image-close,chip-image-viewport,:ArrowLeft']);
    expect(run(true, 'Tab')).toEqual(['fallback:chip-image-preview:chip-image-previous,chip-image-close,chip-image-viewport,chip-image-next:Tab']);
  });
});

// ExpandedImageDialog's ZoomableImage (chat/ZoomableImage.tsx): the image's fitted size, zoom around a point, and pan.
describe('the image preview\'s zoomable image', () => {
  const dialog = contractComponent('composer-chip-image.contract', 'ComposerImageDialog');
  const { ciExp, ciSpan } = contractFns('composer-chip-image.contract');
  const base = { windowWidth: 1280, windowHeight: 840, rem: 16, min: Math.min, max: Math.max, round: Math.round };
  const rig = (width: number, height: number) => {
    const preview = { open: true, id: 'a', name: 'a.png', position: '', many: false, width, height };
    const state = dialog.state();
    const calls: string[] = [];
    const scope = { ...base, preview, ciExp, ciSpan, frame: () => ({ x: 100, y: 50, width: 1000, height: 600 }), command: (op: string) => calls.push(op),
      dialogFallback: () => calls.push('fallback'), preventDefault: () => calls.push('prevent'), stopPropagation: () => calls.push('stop') } as Record<string, unknown>;
    const derived = () => dialog.derives(state, scope);
    const act = (name: string, ...args: unknown[]): unknown => dialog.call(name, state, { ...scope, ...derived() }, args);
    scope.place = (x: number, y: number, at: number) => act('place', x, y, at);
    scope.change = (next: number, x: number, y: number) => act('change', next, x, y);
    return { state, scope, derived, act, calls };
  };
  const keys = (extra: Record<string, unknown> = {}) => ({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, repeat: false, ...extra });

  test('the region takes the image\'s natural size, fitted into the media box; an unknown size keeps the whole box', () => {
    expect(rig(320, 200).derived()).toMatchObject({ viewWidth: 320, viewHeight: 200 });
    // 92% of 1280 less 6rem wide, min(86vh, 100vh - 160) high: a tall image is fitted by its height.
    expect(rig(1000, 2000).derived()).toMatchObject({ maxHeight: 680, viewWidth: 340, viewHeight: 680 });
    expect(rig(1000, 2000).derived().maxWidth).toBeCloseTo(1081.6);
    expect(rig(0, 0).derived().viewWidth).toBeCloseTo(1081.6);
    expect(rig(0, 0).derived().viewHeight).toBe(680);
  });

  test('a click (a press and its release over the image) zooms 2× around its point and back; a drag pans and ends no click', () => {
    // 1000×600 fits the 1081.6×680 box as it is; at 2× it scrolls 918.4 across and 520 down.
    const { state, act } = rig(1000, 600);
    act('down', { clientX: 150, clientY: 80, buttons: 1, pointerType: 'mouse' });
    act('up', { clientX: 150, clientY: 80, buttons: 0 });
    expect(state).toMatchObject({ zoom: 2, seenLeft: 50, seenTop: 30 }); // the point 50,30 stays under the pointer
    act('down', { clientX: 200, clientY: 100, buttons: 1, pointerType: 'mouse' });
    act('move', { clientX: 180, clientY: 90, buttons: 1 });
    expect(state).toMatchObject({ seenLeft: 70, seenTop: 40, dragged: true });
    act('up', { clientX: 180, clientY: 90, buttons: 0 });
    expect(state).toMatchObject({ zoom: 2, dragged: false, downX: -1 });
    // A release outside the viewport is no click; a secondary button's press none either.
    act('down', { clientX: 150, clientY: 80, buttons: 1, pointerType: 'mouse' });
    act('up', { clientX: 1150, clientY: 80, buttons: 0 });
    expect(state.zoom).toBe(2);
    act('down', { clientX: 150, clientY: 80, buttons: 2, pointerType: 'mouse' });
    act('up', { clientX: 150, clientY: 80, buttons: 0 });
    expect(state.zoom).toBe(2);
    act('down', { clientX: 150, clientY: 80, buttons: 1, pointerType: 'mouse' });
    act('up', { clientX: 150, clientY: 80, buttons: 0 });
    expect(state.zoom).toBe(1);
    expect(line('composer-chip-image.contract', 'testId="chip-image-picture"')).toContain('dblclick=doubled pointerdown=down pointermove=move pointerup=up');
    expect(line('composer-chip-image.contract', 'testId="chip-image-picture"')).not.toContain('press=');
  });

  test('an image that fits its box zoomed has nothing to scroll: the offsets stay 0', () => {
    const { state, act } = rig(400, 300);
    act('down', { clientX: 150, clientY: 80, buttons: 1, pointerType: 'mouse' });
    act('up', { clientX: 150, clientY: 80, buttons: 0 });
    expect(state).toMatchObject({ zoom: 2, seenLeft: 0, seenTop: 0 });
    act('down', { clientX: 200, clientY: 100, buttons: 1, pointerType: 'mouse' });
    act('move', { clientX: 150, clientY: 60, buttons: 1 });
    expect(state).toMatchObject({ seenLeft: 0, seenTop: 0 });
  });

  test('panning stops at the edge, so the way back moves at once (the reference sets the live scrollLeft)', () => {
    const { state, act } = rig(1000, 600);
    for (let i = 0; i < 6; i++) act('zoomKey', '=', keys()); // 1.5^6 > 8: held at 8×
    expect(state.zoom).toBe(8);
    const right = 8000 - 1280 * 0.92 + 96, bottom = 4800 - 680;
    for (let i = 0; i < 300; i++) act('keys', 'ArrowRight', keys());
    for (let i = 0; i < 200; i++) act('keys', 'ArrowDown', keys());
    expect(state.seenLeft).toBeCloseTo(right);
    expect(state.seenTop).toBeCloseTo(bottom);
    act('keys', 'ArrowLeft', keys());
    expect(state.seenLeft).toBeCloseTo(right - 40);
    // A drag past the edge stops there too, and the next zoom anchors on where the view really is.
    act('down', { clientX: 500, clientY: 300, buttons: 1, pointerType: 'mouse' });
    act('move', { clientX: 200, clientY: 300, buttons: 1 });
    expect(state.seenLeft).toBeCloseTo(right);
    act('up', { clientX: 200, clientY: 300, buttons: 0 });
    act('zoomKey', '-', keys());
    const viewWidth = 1280 * 0.92 - 96;
    expect(state.seenLeft).toBeCloseTo(Math.min(((right + viewWidth / 2) / 8) * (8 / 1.5) - viewWidth / 2, ciSpan(1000, 8 / 1.5, viewWidth)));
  });

  // The reference's drag sets scrollLeft to the press's offset less the pointer's travel (onPointerMove), which the
  // browser clamps; its origin stays the press. Dragged past an edge and back, the image moves only once the pointer is
  // back where the edge was hit (Chrome for Testing 154 over ZoomableImage.tsx: 0 until then, then 50 and 150).
  test('a drag past an edge keeps its press as origin: the image moves back once the pointer passes where the edge was hit', () => {
    const { state, act } = rig(1000, 600);
    act('zoomKey', 'Enter', keys());
    expect(state).toMatchObject({ zoom: 2, seenLeft: 500, seenTop: 300 });
    act('down', { clientX: 600, clientY: 300, buttons: 1, pointerType: 'mouse' });
    act('move', { clientX: 1100, clientY: 300, buttons: 1 });
    expect(state.seenLeft).toBe(0);
    act('move', { clientX: 1300, clientY: 300, buttons: 1 });
    act('move', { clientX: 1200, clientY: 300, buttons: 1 });
    expect(state.seenLeft).toBe(0);
    act('move', { clientX: 1050, clientY: 300, buttons: 1 });
    expect(state.seenLeft).toBe(50);
    act('up', { clientX: 1050, clientY: 300, buttons: 0 });
  });

  test('a resize fits the image again, and a zoom after it starts from the fitted view', () => {
    const { state, scope, derived, act } = rig(1000, 600);
    act('zoomKey', 'Enter', keys());
    expect(derived()).toMatchObject({ z: 2, viewHeight: 680 });
    scope.windowWidth = 1200;
    expect(derived()).toMatchObject({ z: 1, viewWidth: 1000, viewHeight: 600 });
    act('zoomKey', '+', keys());
    expect(state.zoom).toBe(1.5);
    expect(derived().z).toBe(1.5);
  });

  test('a double click zooms once: its second click\'s toggle is undone by the dblclick after it', () => {
    const { state, act } = rig(1000, 600);
    act('down', { clientX: 150, clientY: 80, buttons: 1, pointerType: 'mouse' });
    act('up', { clientX: 150, clientY: 80, buttons: 0 });
    const first = { zoom: state.zoom, seenLeft: state.seenLeft, seenTop: state.seenTop };
    act('down', { clientX: 150, clientY: 80, buttons: 1, pointerType: 'mouse' });
    act('up', { clientX: 150, clientY: 80, buttons: 0 });
    expect(state.zoom).toBe(1);
    act('doubled');
    expect(state).toMatchObject(first);
    act('doubled'); // nothing left to undo
    expect(state).toMatchObject(first);
  });

  test('the viewport is the reference\'s: the page background, a ring, a polite zoom level; its picture has its own tile key', () => {
    const viewport = line('composer-chip-image.contract', 'id="chip-image-viewport"');
    expect(viewport).toContain('background-color="light-dark(#ffffff, #0a0a0a)"');
    expect(viewport).toContain('box-shadow="0 0 0 1px light-dark(#e4e4e7b3, #ffffff0d), 0 25px 50px -12px #00000040"');
    expect(line('composer-chip-image.contract', 'testId="chip-image-zoom"')).toContain('text `${round(z * 100)}% zoom` aria-live="polite"');
    const picture = line('composer-chip-image.contract', 'testId="chip-image-picture"');
    expect(picture).toContain('data-snapshot-id=`${fit > 0 ? "preview" : "fit"}:${preview.id}`');
    expect(picture).toContain('dblclick=doubled');
    expect(source('modules/apple/T3SnapShot.swift')).toContain('contain = fit || id.hasPrefix("contain:") || id.hasPrefix("preview:")');
  });

  test('Enter toggles, + and - step by 1.5 within 1–8, 0 fits; zoomed, the arrows pan before they step images', () => {
    const { state, act, calls } = rig(400, 300);
    act('zoomKey', '+', keys());
    expect(state.zoom).toBe(1.5);
    act('zoomKey', '0', keys());
    expect(state.zoom).toBe(1);
    act('zoomKey', '-', keys());
    expect(state.zoom).toBe(1);
    act('zoomKey', 'Enter', keys());
    expect(state.zoom).toBe(2);
    act('zoomKey', 'Enter', keys({ repeat: true }));
    expect(state.zoom).toBe(2);
    calls.length = 0;
    act('keys', 'ArrowDown', keys());
    expect(calls).toEqual(['prevent', 'stop']);
    // 800 wide fits in the 1081.6 box: ← and → do not pan, they reach the dialog (one image: the fallback).
    act('keys', 'ArrowRight', keys());
    expect(calls.at(-1)).toBe('fallback');
    for (let i = 0; i < 9; i++) act('zoomKey', '=', keys());
    expect(state.zoom).toBe(8);
  });

  test('the wheel zooms by e^(-delta × 0.002), a pinch by 0.01', () => {
    expect(ciExp(0)).toBe(1);
    expect(Math.abs(ciExp(-0.2) - Math.exp(-0.2))).toBeLessThan(0.001);
    const { state, act } = rig(400, 300);
    act('wheeled', { deltaY: -100, deltaMode: 0, ctrlKey: false, offsetX: 0, offsetY: 0 });
    expect(Math.abs(state.zoom - Math.exp(0.2))).toBeLessThan(0.001);
    act('wheeled', { deltaY: -50, deltaMode: 0, ctrlKey: true, offsetX: 0, offsetY: 0 });
    expect(Math.abs(state.zoom - Math.exp(0.2) * Math.exp(0.5))).toBeLessThan(0.001);
  });

  test('presses beside the image reach the backdrop; the image and its name keep theirs', () => {
    const view = source('composer-chip-image.contract');
    expect(view).toContain('row width=(windowWidth * 0.92) align-items="center" justify-content="space-between" pointer-events="none"');
    expect(view).toContain('column min-width=0 align-items="center" pointer-events="auto"');
    expect(line('composer-chip-image.contract', 'id="chip-image-viewport"')).toContain('box id="chip-image-viewport" tabindex=0 role="region" aria-label=`${preview.name}, zoomable image`');
    // The picture takes the clicks but no Tab stop: the viewport is the stop.
    expect(line('composer-chip-image.contract', 'testId="chip-image-picture"')).toContain('tabindex=-1 retainFocus=true');
  });
});
