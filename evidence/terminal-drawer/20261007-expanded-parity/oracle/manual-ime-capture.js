// Read-only capture; does not type, switch input source, or change focus.
const events = await page.evaluate(() => window.oracleManualIMEEvents ?? []);
const ptyText = fs.existsSync(root + '/manual-ime-pty.ndjson') ? fs.readFileSync(root + '/manual-ime-pty.ndjson', 'utf8') : '';
fs.writeFileSync(root + '/manual-ime-events.json', JSON.stringify(events, null, 2));
await page.screenshot({ path: root + '/manual-ime-original.png', timeout: 2000 });
return { events, pty: ptyText.split('\n').filter(Boolean).map(line => JSON.parse(line)) };
