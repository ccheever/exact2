# Recipe: tell the person something failed

A failure the person must acknowledge is the platform's one-button alert: a `dialog
role="alertdialog"` with its text and one closing `button`. State holds the message; the
action that learns of the failure sets it and calls `showModal(id)`.

```contract
shape Reply
  ok: bool
  error: string

component App
  state draft = ""
  state problem = ""
  mutation saved as shape Reply then afterSave
  derive refusal = match saved { case some(r) => r.ok ? "" : r.error, case none => "" }
  action edit(v: string)
    draft = v
  action save
    send saved = saveNote(draft)
  action afterSave
    if refusal != ""
      problem = refusal
      showModal("failed")
  view
    column padding=16 gap=8
      input value=draft input=edit aria-label="Note" testId="draft"
      button press=save disabled=pending(saved) buttonStyle="filled" testId="save"
        text "Save"
      dialog id="failed" role="alertdialog" aria-label="Couldn’t Save Note"
        text problem testId="problem"
        button commandfor="failed" command="close" testId="failed-ok"
          text "OK"
```

```ts
import type { Answer, Sources } from './app.contract.d.ts';

export const grants = '';

const sources: Sources = {
  saveNote: ([text]) => text.length > 40
    ? { ok: false, error: 'A note is at most 40 characters.' }
    : { ok: true, error: '' },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "a refused save says why, and OK keeps the draft"
  type "draft" "A note far too long to be saved in one go"
  tap "save"
  clock data
  clock settle
  expect text "problem" == "A note is at most 40 characters."
  tap "failed-ok"
  clock settle
  expect text "draft" == "A note far too long to be saved in one go"
```

- The `aria-label` is the alert's title and the `text` its message. iOS shows
  `UIAlertController`, macOS `NSAlert` (Return presses OK), the web its modal `dialog`.
- Keep the title short and the message brief, as Apple's
  [HIG](https://developer.apple.com/design/human-interface-guidelines/alerts) asks: an alert
  is not for long text, which belongs in a sheet. A long one still scrolls in its own box
  where the host draws the alert (the web, Linux), its title and OK in view.
- The button has no `press`: it only closes. Give it one and the dialog is a
  confirmation, which needs its Cancel ([confirm](../contract-for-agents.md#a-confirmation)).
- `then` does not run for a request that brought no answer: show `failed(saved)` in place
  ([fetch-loading-error](fetch-loading-error.md)).
