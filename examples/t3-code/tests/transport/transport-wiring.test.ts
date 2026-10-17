// The window's side of the transport, as app.contract and app.ts declare it: a slot's resources for each
// session and update slot the transport keeps, the tick running fast while any answer that may wait on a
// reply is in flight, and the grants a server named at run time needs.
import { describe, test, expect } from 'bun:test';
import { appPath } from '../support/app-files';
import { SESSION_SLOTS } from '../../src/transport/transport';
import { UPDATE_SLOTS } from '../../src/transport/transport-outdated';
import { answer, grants } from '../../app';

const contract = await Bun.file(appPath('app.contract')).text();
const declared = (kind: string) => [...contract.matchAll(new RegExp(`^  ${kind} ([A-Za-z0-9]+) `, 'gm'))].map(match => match[1]!);

describe('app.contract', () => {
  test('each session slot has its attempt answer and its socket at the URL the tick publishes, each update slot its step and bare socket', () => {
    const resources = new Set(declared('resource'));
    for (let slot = 0; slot < SESSION_SLOTS; slot++) {
      expect(resources.has(`transportConnect${slot}`) && resources.has(`transportSocket${slot}`)).toBe(true);
      expect(contract).toContain(`transportSocket(${slot}, transportTable.s${slot}.url)`);
    }
    for (let slot = 0; slot < UPDATE_SLOTS; slot++) expect(resources.has(`transportUpdateStep${slot}`) && resources.has(`transportUpdateSocket${slot}`)).toBe(true);
    expect(resources.has(`transportSocket${SESSION_SLOTS}`)).toBe(false);
  });

  test('every answer that may wait on a server reply keeps the tick fast while it is in flight', () => {
    // The platform's facts, the Rust Markdown renderer, the highlight pump and the transport's own sources ask no server.
    const exempt = new Set(['wallTime', 'viewport', 'page', 'delivery', 'markdown', 'prMarkdown', 'filesMarkdown', 'highlightSliced', 'transportInbox', 'transportPumped']);
    const waiting = new Set([...(/derive transportWaiting = (.*)/.exec(contract)?.[1] ?? '').matchAll(/pending\(([A-Za-z0-9]+)\)/g)].map(match => match[1]!));
    const answers = [...declared('resource'), ...declared('mutation')].filter(name => !exempt.has(name) && !/^transport(Connect|Socket|UpdateStep|UpdateSocket)\d$/.test(name));
    expect(answers.filter(name => !waiting.has(name))).toEqual([]);
    expect([...waiting].filter(name => !answers.includes(name))).toEqual([]);
  });

  test('the snapshot and Settings › Connections are asked again for news only once their read in flight lands', () => {
    expect(contract).toMatch(/task transportNews when transportInbox\.news != transportSeen and not pending\(data\)/);
    expect(contract).toMatch(/resource data = snapshot\([^\n]*transportSeen\)/);
    expect(contract).toMatch(/resource connectionsPage = connectionsPage\([^\n]*transportFleetSeen\)/);
  });
});

describe('app.ts', () => {
  test('grants whole schemes, for servers named at run time, and the catalog secret', () => {
    expect(grants.split('\n').sort()).toEqual(['net.fetch http:', 'net.fetch https:', 'net.websocket ws:', 'net.websocket wss:', 'secret.keep t3-catalog']);
  });

  test('the tick answers now, as a mutation that sends must', () => {
    const table = answer('transportPump', [1_000_000, ''], null, { fs: {} } as never, null);
    expect(typeof (table as { then?: unknown }).then).toBe('undefined');
    expect(table).toMatchObject({ s0: { key: 'focused' }, live: false });
    expect(answer('transportDeliver', [0], null, { fs: {} } as never, null)).toMatchObject({ news: expect.any(Number), fleet: expect.any(Number) });
  });
});
