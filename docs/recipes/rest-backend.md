# Recipe: your own REST server

A list shared through a Bun server (`pattern rest` writes `server.ts` and this), with a
bearer token, polled. The server listens at app.ts's `API` and answers CORS and its preflight.

```contract
routes nav
  home "/"

shape Items
  ok: bool
  error: string
  titles: string[]

shape Reply
  ok: bool
  error: string

shape Person
  token: string
  name: string

component Shared
  state persona = "ada" persist
  resource people = loadPeople() as shape Person[] else empty()
  state draft = ""
  resource items = loadItems(persona) as shape Items else empty(ok=true)
  mutation added as shape Reply refreshes items then afterAdd
  derive addError = match added { case some(r) => r.error, case none => "" }
  derive listError = pending(added) ? "" : addError != "" ? addError : items.error
  task poll mount
    every(4000, pull)
  action pull
    refresh items
  action signIn(p: string)
    persona = p
  action edit(v: string)
    draft = v
  action add
    if draft.trim() != "" and not pending(added)
      send added = addItem(persona, draft.trim())
  action afterAdd
    if addError == ""
      draft = ""
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="list" position="absolute" inset=0
          header display="flex" align-items="center" padding="8px 16px"
            text "Shared List" role="heading" aria-level=1 flex=1
            button popovertarget="who" aria-label="Signed In" testId="persona"
              image "symbol:person"
          list id="list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              row
                column flex=1 min-width=0 gap=4
                  input value=draft input=edit submit=add placeholder="Add an item" testId="draft"
                  if listError != ""
                    text listError font="-exact-footnote" color="-exact-system-red" testId="error"
              for t in items.titles key=t
                row
                  text t testId=`item-${t}`
      column id="who" popover="auto" role="menu"
        for p in people key=p.token
          button role="menuitemradio" aria-checked=(persona == p.token) press=signIn(p.token)
            popovertarget="who" popovertargetaction="hide" testId=`persona-${p.token}`
            text p.name
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.shared';
export const API = 'http://127.0.0.1:8787';
export const grants = `net.fetch ${API}`;
const PEOPLE = [{ token: 'ada', name: 'Ada Lovelace' }, { token: 'grace', name: 'Grace Hopper' }];

async function call(persona: string, path: string, init: RequestInit = {}): Promise<{ error: string; body: unknown }> {
  try {
    const res = await fetch(`${API}${path}`, { ...init, exactTimeout: 10000,
      headers: { authorization: `Bearer ${persona}`, 'content-type': 'application/json' } });
    const body = (await res.json().catch(() => ({}))) as { error?: unknown };
    return { error: res.ok ? '' : typeof body.error === 'string' ? body.error : 'The server had a problem. Try again.', body };
  } catch (e) {
    if ((e as { code?: string }).code === 'bake') throw e;
    return { error: 'Can’t reach the server. Check that it’s running, then try again.', body: [] };
  }
}

let last: Result<'loadItems'>['titles'] = [];
const sources: Sources = {
  loadPeople: async () => PEOPLE,
  loadItems: async ([persona]) => {
    const { error, body } = await call(persona, '/items');
    if (!error) last = (body as Array<{ title: string }>).map((i) => i.title);
    return { ok: !error, error, titles: last };
  },
  addItem: async ([persona, title]) => {
    const { error } = await call(persona, '/items', { method: 'POST', body: JSON.stringify({ title }) });
    return { ok: !error, error };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "a second person's item arrives; the persona is kept"
  type "draft" "Milk"
  type "draft" key "Enter"
  clock data
  expect tree has "item-Milk"
  request POST "/items" {"title": "Bread"} as grace
  clock +4000
  clock data
  expect tree has "item-Bread"
  tap "persona"
  tap "persona-grace"
  reload
  expect state persona == "grace"
```

- **`test`, `verify` and `agent` run the server** on the database `DB` names (empty for a
  test, one per `--storage`), which server.ts must read (`process.env.DB ?? 'data.db'`); stop yours.
- **A second person is a test line**: `request POST "/items" {...} as grace` sends `Authorization:
  Bearer grace`; `expect 409` after it tests a refusal; then `clock +4000` (the poll), `clock data`.
- **Who is signed in is the bar's menu**, kept with `persist`; it shows names, not tokens.
- Every failure is words (the server's `error` or a sentence), never a status code. The field
  clears in `then` on an ok reply ([after-a-write](after-a-write.md)); a failed poll keeps the list.
