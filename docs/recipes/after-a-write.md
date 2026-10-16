# Recipe: after a write

The field clears, or the form closes, only when the reply is ok; a refusal keeps what was
typed and says why, under the field.

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
  action edit(v: string)
    draft = v
  action add
    if draft.trim() != "" and not pending(added)
      send added = addTag(draft.trim())
  action afterAdd
    if addError == ""
      draft = ""
  view
    list appearance="auto" listStyle="inset-grouped" height="100%" testId="app"
      section
        row gap=8 align-items="center"
          column flex=1 min-width=0 gap=4
            input value=draft input=edit submit=add placeholder="New tag"
              aria-label="New tag" testId="draft"
            if addError != "" and not pending(added)
              text addError font="-exact-footnote" color="-exact-system-red" testId="add-error"
          button press=add disabled=(draft.trim() == "" or pending(added)) buttonStyle="filled" testId="add"
            text "Add"
      section
        for t in tags key=t.name
          row
            text t.name
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.tags'; // exact new writes this line
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

- **Clear in `then`**, which runs as its own commit when each reply lands and reads it; clearing
  where you send loses the text on a refusal. Give the write its own mutation: a `then` runs for
  every reply of its mutation. `not pending(added)` stops a second Enter: a newer send replaces
  the reply, not the work.
- **A refusal is a reply** (`{ ok: false, error }` in words) for every failure the data module
  can name: a rule, a 4xx, a lost connection ([rest-backend](rest-backend.md)). A throw brings no
  answer: `then` doesn't run and nothing is said.
- Closing a form is the same `then`: `if saveError == "" and top(nav).name == "add"`, then
  `nav = back(nav)` ([navigation-stack](navigation-stack.md)).
