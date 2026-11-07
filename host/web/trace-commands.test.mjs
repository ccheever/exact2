// The trace commands on the wasm web host: glue.js refuses both in the journal (the Apple hosts record traces), run
// as glue.js runs them, with the host doubles glue-fixture.mjs gives and those its batch needs after its ops.
import { test, expect } from 'bun:test';
import vm from 'node:vm';
import { declaration, fixture } from './glue-fixture.mjs';

test('the wasm host refuses startTrace_EXPERIMENTAL and stopTrace_EXPERIMENTAL in the journal', () => {
  const f = fixture(false), said = [];
  Object.assign(f, { log: line => said.push(line), prepareContexts() {}, listSelection: null, pendingStore: { flush() {} }, inputHandlers: null,
    focusAutofocus() {}, positionContexts() {}, markScrollDocument() {}, refreshSymbols() {}, syncLists() {} });
  f.navigation.project = () => {};
  f.collections = { ...f.collections, restyled() {} };
  vm.runInContext(declaration('apply'), f);
  f.apply({ ops: [{ op: 'command', name: 'startTrace_EXPERIMENTAL', args: [] }, { op: 'command', name: 'stopTrace_EXPERIMENTAL', args: [7500] }] });
  expect(said).toEqual([
    "startTrace_EXPERIMENTAL refused: the web host does not record traces; use the browser's performance panel",
    "stopTrace_EXPERIMENTAL refused: the web host does not record traces; use the browser's performance panel",
  ]);
});
