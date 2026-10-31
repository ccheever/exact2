import { expect, test } from 'bun:test';
import { contractComponent } from '../tests/support/contract-action-fixture';
import { contractFns } from '../tests/support/contract-fn-fixture';
import { mobileContentGeometry } from './content-geometry';

const owner = 'visit-1:home';
const metrics = () => ({ state: 'ready', owner, generation: 10, serial: 1, coordinateSpace: 'content-observer',
  bounds: { x: 10, y: 20, width: 466, height: 600 }, safeArea: { top: 0, right: 0, bottom: 12, left: 0 },
  horizontalSizeClass: 'compact', verticalSizeClass: 'regular', hinge: 'unknown', verticalBarEdge: 'right',
  bottomBarInset: 48, reservedRegions: [{ kind: 'occlusion', x: 450, y: 10, width: 50, height: 60 },
    { kind: 'division', x: -20, y: 200, width: 40, height: 300 },
    { kind: 'occlusion', x: 600, y: 30, width: 20, height: 20 }] });
const packet = (value: unknown, generation = 10, serial = 1) => `${generation}\n${serial}\n${JSON.stringify(value)}`;
const parse = (value: unknown) => mobileContentGeometry(owner, owner, 10, 1, packet(value));

test('content measurements retain local origins, residual insets and clipped division/occlusion rectangles', () => {
  const value = parse(metrics());
  expect(value).toEqual({ ...metrics(), reservedRegions: [
    { kind: 'occlusion', x: 450, y: 20, width: 26, height: 50 },
    { kind: 'division', x: 10, y: 200, width: 10, height: 300 }] });
  expect(value.bounds.width).toBe(466);
  expect(value.safeArea.right).toBe(0);
  expect(value.hinge).toBe('unknown');
  expect(parse({ ...metrics(), bottomBarInset: null, verticalBarEdge: 'none', hinge: 'absent' }).bottomBarInset).toBeNull();
});

test('unmeasured and detached content remains explicitly unavailable', () => {
  expect(mobileContentGeometry(owner, '', 0, 0, '').state).toBe('unavailable');
  expect(mobileContentGeometry('visit-2:thread', owner, 10, 1, packet(metrics())).state).toBe('unavailable');
  const detached = parse({ state: 'unavailable', owner, generation: 10, serial: 1,
    coordinateSpace: 'content-observer', hinge: 'present' });
  expect(detached.state).toBe('unavailable');
  expect(detached.hinge).toBe('present');
  expect(detached.generation).toBe(10);
  expect(detached.bounds).toEqual({ x: 0, y: 0, width: 0, height: 0 });
  expect(detached.reservedRegions).toEqual([]);
});

test.skipIf(!process.env.T3_NATIVE_CONTENT_GEOMETRY)('actual native lifetime packets pass the geometry boundary', () => {
  const document = JSON.parse(process.env.T3_NATIVE_CONTENT_GEOMETRY!) as { ready: string; detached: string };
  const read = (value: string) => {
    const [generation, serial] = value.split('\n');
    return mobileContentGeometry('native-bridge:home', 'native-bridge:home', Number(generation), Number(serial), value);
  };
  const ready = read(document.ready), detached = read(document.detached);
  expect(ready).toMatchObject({ state: 'ready', owner: 'native-bridge:home', hinge: 'unknown',
    bounds: { x: 10, y: 20, width: 466, height: 600 }, safeArea: { top: 0, right: 0, bottom: 12, left: 0 },
    horizontalSizeClass: 'compact', verticalSizeClass: 'regular', verticalBarEdge: 'right', bottomBarInset: 48,
    reservedRegions: [{ kind: 'occlusion', x: 450, y: 20, width: 26, height: 50 }] });
  expect(detached.state).toBe('unavailable');
  expect(detached.hinge).toBe('present');
  expect(detached.generation).toBeGreaterThan(ready.generation);
  expect(detached.bounds).toEqual({ x: 0, y: 0, width: 0, height: 0 });
});

test('geometry parser refuses malformed identity, dimensions, insets, traits and region facts', () => {
  const changed: unknown[] = [null, [], {}, { ...metrics(), owner: 'stale:home' }, { ...metrics(), generation: 11 },
    { ...metrics(), serial: 2 }, { ...metrics(), coordinateSpace: 'window' }, { ...metrics(), state: 'pending' },
    { ...metrics(), hinge: false }, { ...metrics(), verticalBarEdge: 'trailing' },
    { ...metrics(), horizontalSizeClass: '' }, { ...metrics(), verticalSizeClass: true },
    { ...metrics(), bounds: { x: 0, y: 0, width: 0, height: 600 } },
    { ...metrics(), bounds: { x: 1e308, y: 0, width: 1e308, height: 600 } },
    { ...metrics(), safeArea: { top: -1, right: 0, bottom: 0, left: 0 } },
    { ...metrics(), safeArea: { top: 0, right: '4', bottom: 0, left: 0 } },
    { ...metrics(), bottomBarInset: -1 }, { ...metrics(), bottomBarInset: undefined },
    { ...metrics(), reservedRegions: [{ kind: 'window', x: 0, y: 0, width: 10, height: 20 }] },
    { ...metrics(), reservedRegions: [{ kind: 'division', x: 0, y: 0, width: -10, height: 20 }] }];
  for (const value of changed) expect(parse(value).state).toBe('unavailable');
  for (const raw of ['', '{', `10\n1\n{`, `10\n1\n${JSON.stringify(metrics())}\nextra`,
    `1e1\n1\n${JSON.stringify(metrics())}`, `10\n01\n${JSON.stringify(metrics())}`,
    `10\n1\n${JSON.stringify(metrics()).replace('466', '1e999')}`]) {
    expect(mobileContentGeometry(owner, owner, 10, 1, raw).state).toBe('unavailable');
  }
});

const fns = contractFns('mobile/content-geometry.contract', [], { floor: Math.floor,
  parseNumber: (value: string) => value.trim() && Number.isFinite(Number(value)) ? Number(value) : null });
const root = contractComponent('mobile/content-geometry-root.contract', 'App');

test('root geometry action rejects stale owners and counter envelopes before replacing current geometry', () => {
  const state = root.state();
  const scope = { ...fns, nav: [{ id: 'visit-1', name: 'home' }], top: (nav: unknown[]) => nav.at(-1) };
  const deliver = (value: string, from = owner) => root.call('contentGeometryEvent', state, scope, [from, value]);
  deliver(packet(metrics()));
  expect(state).toEqual({ layoutJSON: '', contentGeometryOwner: owner, contentGeometryGeneration: 10,
    contentGeometrySerial: 1, contentGeometryPacket: packet(metrics()) });
  const before = { ...state };
  for (const [generation, serial] of [['9', '99'], ['10', '1'], ['', '2'], ['Infinity', '2'],
    ['NaN', '2'], ['10.5', '2'], ['010', '2'], ['1e1', '2'], ['9007199254740992', '2'], ['11', '-1'], ['11', '2.5']]) {
    deliver(`${generation}\n${serial}\n{}`);
    expect(state).toEqual(before);
  }
  deliver(packet(metrics(), 12, 1), 'stale:home');
  expect(state).toEqual(before);
  const detached = { state: 'unavailable', owner, generation: 11, serial: 1,
    coordinateSpace: 'content-observer', hinge: 'present' };
  deliver(packet(detached, 11, 1));
  expect(mobileContentGeometry(owner, String(state.contentGeometryOwner), Number(state.contentGeometryGeneration),
    Number(state.contentGeometrySerial), String(state.contentGeometryPacket)).state).toBe('unavailable');
  deliver(packet({ ...metrics(), generation: 12, serial: 1 }, 12, 1));
  expect(state.contentGeometryGeneration).toBe(12);
  deliver(packet(detached, 11, 2));
  expect(state.contentGeometryGeneration).toBe(12);
  deliver(packet({ ...metrics(), generation: 12, serial: 2 }, 12, 2));
  expect(state.contentGeometrySerial).toBe(2);
  scope.nav = [{ id: 'visit-2', name: 'thread' }];
  deliver(packet(metrics(), 99, 1));
  expect(state.contentGeometryGeneration).toBe(12);
  expect(mobileContentGeometry('visit-2:thread', String(state.contentGeometryOwner), 12, 1,
    String(state.contentGeometryPacket)).state).toBe('unavailable');
  deliver(packet({ ...metrics(), owner: 'visit-2:thread', generation: 13 }, 13, 1), 'visit-2:thread');
  expect(state.contentGeometryOwner).toBe('visit-2:thread');
  expect(state.contentGeometryGeneration).toBe(13);
  scope.nav = [{ id: 'visit-1', name: 'home' }];
  deliver(packet({ ...metrics(), generation: 14 }, 14, 1));
  expect(state.contentGeometryOwner).toBe(owner);
  expect(state.contentGeometryGeneration).toBe(14);
});

test('route geometry observer mounts only for an active route and cannot capture input', () => {
  const view = contractComponent('mobile/content-geometry.contract', 'MobileContentGeometry');
  expect(view.drawn('t3-content-geometry', {}, { owner: '' })).toBe(false);
  expect(view.drawn('t3-content-geometry', {}, { owner })).toBe(true);
  expect(view.attribute('t3-content-geometry', 'pointer-events', {}, { owner })).toBe('none');
  const route = contractComponent('mobile/route-content.contract', 'MobileRouteContent');
  const scope = { focused: false, routeId: 'visit-1', routeName: 'home', composerExpanded: false, composerFocused: false,
    threadData: { environmentId: '', threadId: '', composer: { contentOwner: '' }, pendingInput: { active: false } },
    layout: { safeBottom: 0 }, workingControl: { visible: false }, workspace: { fileInspectorSupported: false }, max: Math.max };
  expect(route.drawn('MobileContentGeometry(', route.state(), scope)).toBe(false);
  expect(route.drawn('MobileContentGeometry(', route.state(), { ...scope, focused: true })).toBe(true);
});

test('Home uses a ready route-local height and safe top for its transparent empty frame', () => {
  const home = contractComponent('mobile/home.contract', 'Home');
  const scope = { ...contractFns('mobile/home.contract', [], { first: (values: unknown[]) => values[0] }),
    fullPageEmpty: true, liquidGlass: true,
    windowHeight: 900, safeTop: 40, routeId: 'visit-1', listId: 'home-list', contentGeometry: metrics() };
  expect(home.attribute('height=(fullPageFrame ?', 'height', home.state(), scope)).toBe(600);
  expect(home.attribute('padding-top=(fullPageFrame ?', 'padding-top', home.state(), scope)).toBe(72);
  expect(home.attribute('height=(fullPageFrame ?', 'height', home.state(), { ...scope, contentGeometry: { ...metrics(), state: 'unavailable' } })).toBe(900);
  expect(home.attribute('padding-right=(listId == "sidebar-list"', 'padding-right', home.state(), scope)).toBe(26);
});
