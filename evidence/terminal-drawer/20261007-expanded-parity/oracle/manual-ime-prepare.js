// Execute through control16324 ONLY after root grants exclusive GUI and user agrees.
await app.evaluate(({ BrowserWindow }) => {
  const window = BrowserWindow.getAllWindows()[0];
  window.show();
  window.focus();
});
if (await page.locator('.t3-ghostty-input:visible').count() === 0) {
  await page.getByRole('button', { name: 'Toggle terminal drawer', exact: true }).click();
}
const input = page.locator('.t3-ghostty-input:visible');
await input.focus();
await page.keyboard.press('Control+c');
await page.keyboard.type('python3 ' + root + '/manual-ime-pty.py');
await page.keyboard.press('Enter');
await input.evaluate((node) => {
  window.oracleManualIMEEvents = [];
  window.oracleManualIMEListener = (event) => {
    window.oracleManualIMEEvents.push({
      type: event.type, data: event.data ?? null, inputType: event.inputType ?? null,
      key: event.key ?? null, code: event.code ?? null, keyCode: event.keyCode ?? null,
      isComposing: event.isComposing ?? null, isTrusted: event.isTrusted,
      ctrl: event.ctrlKey ?? false, alt: event.altKey ?? false,
      meta: event.metaKey ?? false, shift: event.shiftKey ?? false,
      value: node.value, time: performance.now(),
    });
  };
  for (const type of ['compositionstart', 'compositionupdate', 'compositionend', 'beforeinput', 'input', 'keydown', 'keyup']) {
    node.addEventListener(type, window.oracleManualIMEListener, true);
  }
  window.oracleManualIMENode = node;
});
return { pid: app.process().pid, url: page.url(), focused: await input.evaluate(node => document.activeElement === node) };
