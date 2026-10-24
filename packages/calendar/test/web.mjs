// LLP 1133.018 §11 drive 7 and stage 1: the Calendar example on the web in
// production, in each engine, with the web implementations bound and no
// agent. The browser has no calendar store, so there is no "Show my
// calendars"; a desktop browser has no contact picker, so there is no
// Invite…; Add to Calendar… downloads an `.ics` file, which the suite reads
// against RFC 5545 (§3.1 lines and folding, §3.4 the object, §3.6.1 the
// VEVENT's required properties, §3.3.5 and §3.3.4 its start).
//
// It also records whether the module call reaches the page inside the
// press's transient activation (HTML's `navigator.userActivation`), which
// the Contact Picker needs: read at the download's click, and at the
// picker's `select`, which a stand-in `navigator.contacts` answers in each
// engine, since only Chrome on Android has the picker.
//
// usage: bun packages/calendar/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { mkdirSync, readFileSync } from 'node:fs';
import { extname, join, resolve } from 'node:path';
import { checker, root, run } from '../../contacts/test/fixture.mjs';

const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'firefox', 'webkit'];
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { check, failed } = checker();

const dist = resolve(root, 'target/calendar-web-dist'), downloads = resolve(root, 'target/calendar-web-downloads');
mkdirSync(downloads, { recursive: true });
run('bun', [resolve(root, 'host/web/build.mjs'), 'calendar-web'], root, { ...process.env, EXACT_APP_DIR: resolve(root, 'examples/calendar'), EXACT_WEB_DIST: dist });
const types = { '.js': 'text/javascript', '.mjs': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css', '.png': 'image/png', '.webp': 'image/webp', '.svg': 'image/svg+xml' };
const server = Bun.serve({ port: 0, async fetch(req) {
  let path = decodeURIComponent(new URL(req.url).pathname);
  if (path.endsWith('/')) path += 'index.html';
  const file = Bun.file(join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
const origin = `http://127.0.0.1:${server.port}`;

/** An iCalendar file's content lines, unfolded (RFC 5545 §3.1), and whether every physical line ended CRLF and fit 75 octets. */
function lines(text) {
  const physical = text.split('\r\n');
  const crlf = physical.at(-1) === '' && !physical.slice(0, -1).some(l => l.includes('\n'));
  const short = physical.every(l => new TextEncoder().encode(l).length <= 75);
  const out = [];
  for (const l of physical.slice(0, -1)) {
    if (l.startsWith(' ') || l.startsWith('\t')) out[out.length - 1] += l.slice(1);
    else out.push(l);
  }
  return { out, crlf, short };
}
/** A property's value by name (parameters dropped), and its parameters. */
const property = (all, name) => {
  const l = all.find(x => x.split(/[;:]/, 1)[0] === name);
  if (!l) return null;
  const colon = l.indexOf(':');
  return { params: l.slice(name.length, colon), value: l.slice(colon + 1) };
};
const unescape = v => v.replace(/\\n/gi, '\n').replace(/\\([;,\\])/g, '$1');

for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch();
  try {
    const context = await browser.newContext({ acceptDownloads: true, viewport: { width: 1180, height: 800 } });
    // The browser's own word on activation, at the download's click.
    await context.addInitScript(() => {
      const click = HTMLAnchorElement.prototype.click;
      globalThis.__activation = [];
      HTMLAnchorElement.prototype.click = function () {
        if (this.download) globalThis.__activation.push(navigator.userActivation ? String(navigator.userActivation.isActive) : 'unknown');
        return click.call(this);
      };
    });
    const page = await context.newPage();
    page.on('pageerror', e => console.log(`${engine} page error: ${e.message}`));
    page.on('console', m => { if (m.type() === 'error') console.log(`${engine} console: ${m.text()}`); });
    await page.goto(origin);
    await page.getByTestId('new-item').click({ timeout: 30000 });
    const title = `Drive seven; ${engine}, with a long title that folds past seventy-five octets: café`;
    await page.getByTestId('schedule-title').fill(title);
    await page.getByTestId('schedule-notes').fill('First line\nSecond, with a comma; and a semicolon');
    await page.getByTestId('save-schedule').click();
    await page.locator('[data-testid^="agenda-item-"][aria-label^="Drive seven"]').first().click({ timeout: 10000 });
    const has = async id => (await page.getByTestId(id).count()) > 0;
    check(engine, '7: no calendar store, so no "Show my calendars"', !(await has('show-my-calendars')), 'the switch is shown');
    await page.getByTestId('add-to-calendar').waitFor({ timeout: 10000 }).catch(() => {});
    check(engine, '7: no contact picker, so no Invite…', !(await has('invite')), 'Invite… is shown');
    const [download] = await Promise.all([page.waitForEvent('download', { timeout: 10000 }), page.getByTestId('add-to-calendar').click({ timeout: 10000 })]);
    const file = join(downloads, `${engine}.ics`);
    await download.saveAs(file);
    const text = readFileSync(file, 'utf8'), { out, crlf, short } = lines(text);
    check(engine, '7: the file is named for the item, .ics', download.suggestedFilename().endsWith('.ics') && download.suggestedFilename().startsWith('Drive seven'), download.suggestedFilename());
    check(engine, '7: CRLF lines, folded at 75 octets', crlf && short, JSON.stringify(text.slice(0, 200)));
    const begin = out.indexOf('BEGIN:VEVENT'), end = out.indexOf('END:VEVENT');
    check(engine, '7: one VEVENT inside a VCALENDAR 2.0', out[0] === 'BEGIN:VCALENDAR' && out.at(-1) === 'END:VCALENDAR' && out.includes('VERSION:2.0') && !!property(out, 'PRODID') && begin > 0 && end > begin, out.join(' | '));
    const event = out.slice(begin + 1, end);
    const start = property(event, 'DTSTART'), duration = property(event, 'DURATION');
    check(engine, '7: UID, DTSTAMP in UTC, DTSTART, DURATION', !!property(event, 'UID')?.value && /^\d{8}T\d{6}Z$/.test(property(event, 'DTSTAMP')?.value ?? '') && !!start && /^P/.test(duration?.value ?? ''), event.join(' | '));
    check(engine, '7: a timed item starts in UTC (form 2) or at a date', /^\d{8}T\d{6}Z$/.test(start?.value ?? '') || (start?.params === ';VALUE=DATE' && /^\d{8}$/.test(start?.value ?? '')), JSON.stringify(start));
    check(engine, '7: SUMMARY is the title, escaped', unescape(property(event, 'SUMMARY')?.value ?? '') === title, property(event, 'SUMMARY')?.value);
    check(engine, '7: DESCRIPTION is the notes, escaped', unescape(property(event, 'DESCRIPTION')?.value ?? '') === 'First line\nSecond, with a comma; and a semicolon', property(event, 'DESCRIPTION')?.value);
    const note = await page.getByTestId('add-to-calendar-result').innerText({ timeout: 5000 }).catch(() => '');
    check(engine, '7: the editor answers unknown: "Opened in your calendar app"', note === 'Opened in your calendar app', note);
    const activation = await page.evaluate(() => globalThis.__activation);
    console.log(`NOTE ${engine}: the module call reached the page with transient activation ${JSON.stringify(activation)}`);
    await context.close();

    // The Contact Picker, which only Chrome on Android has, stood in for by
    // `navigator.contacts` answering two contacts: Invite… shows, a press
    // reaches `select` inside its activation, and both become guests.
    const picking = await browser.newContext({ viewport: { width: 1180, height: 800 } });
    await picking.addInitScript(() => {
      globalThis.__selects = [];
      Object.defineProperty(Navigator.prototype, 'contacts', { configurable: true, get: () => ({
        getProperties: async () => ['email', 'name', 'tel'],
        select: async (properties, options) => {
          globalThis.__selects.push({ properties, multiple: !!options?.multiple, activation: String(navigator.userActivation?.isActive) });
          return [{ name: ['Ana Lima'], email: ['ana@example.com'] }, { name: [], email: ['dana@example.com'] }];
        },
      }) });
    });
    const invited = await picking.newPage();
    await invited.goto(origin);
    await invited.getByTestId('new-item').click({ timeout: 30000 });
    await invited.getByTestId('schedule-title').fill(`Invite ${engine}`);
    await invited.getByTestId('save-schedule').click();
    await invited.locator('[data-testid^="agenda-item-"][aria-label^="Invite"]').first().click({ timeout: 10000 });
    await invited.getByTestId('invite').click({ timeout: 10000 });
    const guests = await invited.getByTestId('guest-names').innerText({ timeout: 10000 }).catch(() => '');
    const selects = await invited.evaluate(() => globalThis.__selects);
    check(engine, 'picker: a picker shows Invite…, whose press asks for names and emails, several', selects.length === 1 && selects[0].multiple && selects[0].properties.join() === 'name,email', JSON.stringify(selects));
    check(engine, 'picker: the press reaches select inside its transient activation', selects[0]?.activation === 'true', JSON.stringify(selects));
    check(engine, 'picker: both picked contacts are guests, the nameless one by email', guests === 'Guests: Ana Lima, dana@example.com', guests);
    await picking.close();
  } finally {
    await browser.close();
  }
}
server.stop(true);
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
