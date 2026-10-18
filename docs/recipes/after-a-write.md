# Recipe: after a write

Clear fields and close forms only on `ok: true`. On refusal, keep the draft and
show its reason.

```contract
shape Tag
  name: string

shape Reply
  ok: bool
  error: string

component TagList
  state draft = ""
  resource tags = loadTags() as shape Tag[] else empty()
  mutation added as shape Reply refreshes tags then afterAdd
  derive addError = match added { case some(r) => r.error, case none => "" }
  derive shownError = pending(added) ? "" : failed(added) ? "Couldn't save the tag. Try again." : addError
  action edit(v: string)
    draft = v
  action add
    if draft.trim() != "" and not pending(added)
      send added = addTag(draft.trim())
  action afterAdd
    if match added { case some(r) => r.ok, case none => false }
      draft = ""
  view
    list appearance="auto" listStyle="inset-grouped" height="100%" testId="app"
      section
        row gap=8 align-items="center"
          column flex=1 min-width=0 gap=4
            input value=draft input=edit submit=add placeholder="New tag"
              aria-label="New tag" testId="draft"
            if shownError != ""
              text shownError font="-exact-footnote" color="-exact-system-red" testId="add-error"
          button press=add disabled=(draft.trim() == "" or pending(added)) buttonStyle="filled" testId="add"
            text "Add"
      section
        for t in tags key=t.name
          row
            text t.name
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.tags';
export const grants = '';

const tags: Result<'loadTags'> = [];
const sources: Sources = {
  loadTags: () => tags,
  addTag: ([name]) => {
    const same = tags.find((t) => t.name.toLowerCase() === name.toLowerCase());
    if (same) return { ok: false, error: `${same.name} is taken. Try another name.` };
    tags.push({ name });
    return { ok: true, error: '' };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "a refused add keeps its text and says why; the next one clears it"
  type "draft" "Work"
  tap "add"
  clock data
  expect text "draft" == ""
  type "draft" "work"
  tap "add"
  clock data
  expect text "add-error" == "Work is taken. Try another name."
  expect text "draft" == "work"
  type "draft" "Home"
  tap "add"
  clock data
  expect tree missing "add-error"
```

Do not also set an unqueued mutation's slot to `none` when sending it: assignments
commit after sends and may drop the reply. Hide old errors with `pending(added)`.
`none` means no reply; `failed(added)` marks request failure. A request failure
runs no `then`; an `{ ok: false, error }` reply does.

`afterAdd` clears the draft only when its reply has `ok: true`. The pending guard
prevents a second Enter from replacing the first reply. A continuation that closes
a form should also check the current route before setting `nav = back(nav)`
([navigation-stack](navigation-stack.md)).
