// The window's side of the transport, as app.contract and app.ts declare it: a slot's resources for each
// session and update slot the transport keeps, the tick publishing the table as each answer that may wait on a
// reply starts or ends and as the delivering answer reports a newer table, and the grants a server named at run
// time needs.
import { describe, test, expect } from 'bun:test';
import { appText } from '../support/app-files';

import { SESSION_SLOTS, Transport } from '../../src/transport/transport';
import { UPDATE_SLOTS } from '../../src/transport/transport-outdated';
import { answer, grants } from '../../app';

const contract = await appText('app.contract');
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

  test('every answer that may wait on a server reply has the tick publish the table as it starts and ends; nothing polls', () => {
    // The platform's facts, the Rust Markdown renderer, the highlight pump and the transport's own sources ask no server.
    const exempt = new Set(['promptLinkPage', 'promptLinksReady', 'wallTime', 'viewport', 'page', 'delivery', 'notificationPermission', 'markdown', 'questionMarkdown', 'traceMarkdown', 'prMarkdown', 'filesMarkdown', 'highlightAsked', 'highlightCoded', 'highlightStored', 'transportInbox', 'transportPumped', 'serverBrowserLive', 'serverBrowserMiniLive', 'serverBrowserChanged', 'serverBrowserMaintained', 'serverBrowserMiniMaintained']);
    // Shared field delivery can wait on an app edit, never a server reply;
    // source projection, intent and accepted-input ACKs are local too.
    // A pull request surface's remembered tab (prTabSaved) is the panel model's, as local as these.
    // The window's last interaction is noted in the module (app.ts), as local as these.
    // The model picker's shortcut announcements (pickerAnnounced) are a `cclocal:` op the module answers itself.
    for (const name of ['composerFieldNext', 'richComposer', 'richPromptPreview', 'composerEdited', 'composerFieldChanged', 'composerFieldAck', 'chipObserved', 'chipPressed', 'prTabSaved', 'interactionNoted', 'interactionResumed', 'pickerAnnounced']) exempt.add(name);
    // A Browser frame's package calls, which the root names but data code makes (app.contract frameCommand), ask no server.
    for (const name of ['siteCleared', 'promptAnswered']) exempt.add(name);
    // A draft image's upload is a stream over HTTP that no socket reply ends (composer-uploads.ts).
    for (const name of ['composerUpload0', 'composerUpload1', 'composerUpload2']) exempt.add(name);
    expect(contract).toMatch(/task transportAnswered key=transportAnswering\n    after\(1, transportPump\)/);
    expect(contract).toMatch(/task transportRepublish when transportInbox\.table > transportTable\.rev key=transportInbox\.table\n    after\(1, transportPump\)/);
    expect(contract).not.toMatch(/every\(16, transportPump\)/);
    // An area may tell the tick about its own answers in a task of its own, keyed on their pending
    // (composer-controls-members.contract composerUploadsAnswered).
    const own = [...contract.matchAll(/task [A-Za-z0-9]+ key=`((?:\$\{pending\([A-Za-z0-9]+\)\})+)`\n    after\(1, transportPump\)/g)].map(match => match[1]!);
    expect(own.length).toBeGreaterThan(0);
    const waiting = new Set([/derive transportAnswering = (.*)/.exec(contract)?.[1] ?? '', ...own].flatMap(key => [...key.matchAll(/pending\(([A-Za-z0-9]+)\)/g)].map(match => match[1]!)));
    const answers = [...declared('resource'), ...declared('mutation')].filter(name => !exempt.has(name) && !/^transport(Connect|Socket|UpdateStep|UpdateSocket)\d$/.test(name));
    expect(answers.filter(name => !waiting.has(name))).toEqual([]);
    expect([...waiting].filter(name => !answers.includes(name))).toEqual([]);
  });

  test('the snapshot and Settings › Connections are asked again for news only once their read in flight lands', () => {
    expect(contract).toMatch(/task transportNews when transportInbox\.news != transportSeen and not pending\(data\)/);
    expect(contract).toMatch(/resource data = snapshot\([^\n]*transportSeen, composerTyping, layoutFrames, wallTime\.formatLocale\)/);
    expect(contract).toMatch(/resource connectionsPage = connectionsPage\([^\n]*transportFleetSeen\)/);
  });
});

describe('app.ts', () => {
  test('grants whole schemes, for servers named at run time, and the catalog secret', () => {
    expect(grants.split('\n')).toEqual(expect.arrayContaining(['net.fetch http:', 'net.fetch https:', 'net.websocket ws:', 'net.websocket wss:', 'secret.keep t3-catalog']));
  });

  test('claims the camera and microphone for the commands terminals and agents run (build-desktop-artifact.ts)', async () => {
    expect(grants.split('\n')).toEqual(expect.arrayContaining(['device.camera purpose.camera', 'device.microphone purpose.microphone']));
    const strings = await Bun.file(new URL('../../strings/en.json', import.meta.url)).json() as Record<string, string>;
    expect(strings['purpose.camera']).toBe('T3 Code lets commands run by terminals and coding agents use the camera.');
    expect(strings['purpose.microphone']).toBe('T3 Code lets commands run by terminals and coding agents use the microphone.');
  });

  test('the tick answers now, as a mutation that sends must', () => {
    const table = answer('transportPump', [1_000_000, ''], null, { fs: {} } as never, null);
    expect(typeof (table as { then?: unknown }).then).toBe('undefined');
    expect(table).toMatchObject({ s0: { key: 'focused' }, live: false });
    expect(answer('transportDeliver', [0], null, { fs: {} } as never, null)).toMatchObject({ news: expect.any(Number), fleet: expect.any(Number), table: (table as { rev: number }).rev });
  });

  test("a topic watched after the answer replied is taken up by that source's next answer", () => {
    // exact2 refuses a watch once the answer has replied, as code resumed by a delivered reply runs then.
    let replied = true;
    const watched: string[] = [];
    const module = { available: true, watch(topic: string) { if (replied) throw new Error('native.watch outside an answer'); watched.push(topic); }, later: async () => ({}) };
    const transport = new Transport();
    transport.wrap(module, 'snapshot')!.watch('t3.local');
    expect(watched).toEqual([]);
    replied = false;
    transport.wrap(module, 'snapshot');
    transport.wrap(module, 'editor');
    expect(watched).toEqual(['t3.local']);
  });

  test("a module call of a let-go answer rejects as exact2 rejected it, so the client reads it as superseded", async () => {
    const letGo = Object.assign(new Error('the answer was let go before this reply; the request may already have been sent'), { name: 'FetchError', kind: 'Aborted' });
    const module = { available: true, watch() {}, later: async () => { throw letGo; } };
    const bridge = new Transport().wrap(module, 'snapshot')!;
    expect(await bridge.later({ op: 'status' }).catch(error => error)).toBe(letGo);
  });
});
