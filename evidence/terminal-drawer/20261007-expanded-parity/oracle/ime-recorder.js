// Run inside the existing isolated Electron renderer through its driver.
// Records only the temporary control or the terminal input, never unrelated fields.
window.oracleIMEEvents = [];
window.oracleIMEListener = (event) => {
  const target = event.target;
  if (!(target instanceof HTMLElement) || (target.id !== 'oracle-ime-control' && !target.classList.contains('t3-ghostty-input'))) return;
  window.oracleIMEEvents.push({
    type: event.type, data: event.data ?? null, inputType: event.inputType ?? null,
    key: event.key ?? null, code: event.code ?? null, keyCode: event.keyCode ?? null,
    isComposing: event.isComposing ?? null, isTrusted: event.isTrusted,
    ctrl: event.ctrlKey ?? false, alt: event.altKey ?? false,
    meta: event.metaKey ?? false, shift: event.shiftKey ?? false,
    target: target.id || target.className, value: target.value ?? null,
    time: performance.now(),
  });
};
for (const type of ['compositionstart', 'compositionupdate', 'compositionend', 'beforeinput', 'input', 'keydown', 'keyup']) document.addEventListener(type, window.oracleIMEListener, true);
