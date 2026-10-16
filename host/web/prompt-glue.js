// A prompt's Return on the web (kernel/src/confirmation.rs; the rule is one
// on every host, docs/contract-for-agents.md): Enter in an alertdialog's text
// field. The field's own `key` handlers hear it first and may prevent it. A
// field with a `submit` handler keeps it: the runtime marks the Enter
// (`$submit` on the JS target, `exactSubmit` on the wasm page) and runs it,
// and that is all. Any other field's edit commits (its `change`, as on
// leaving it), then the first action that is not destructive is pressed;
// while that action is disabled nothing is, never the next one. Both web
// targets load this module with their input piece (input-glue.js), and the
// JS target with its commands (commands.js), so it listens once a page shows.
addEventListener("keydown", ev => {
  const field = ev.target;
  if (ev.key !== "Enter" || ev.isComposing || ev.defaultPrevented || ev.$submit || ev.exactSubmit || field?.localName !== "input") return;
  const dialog = field.closest('dialog[role="alertdialog"], [popover][role="alertdialog"]');
  if (!dialog) return;
  ev.preventDefault();
  field.blur();
  const action = [...dialog.querySelectorAll("button")].find(b => b.matches('[data-exact-on~="press"]:not([data-destructive="true"])'));
  if (action && !action.disabled) action.click();
  else field.focus();
});
