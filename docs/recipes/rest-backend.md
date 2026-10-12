# Recipe: your own REST server

A list shared through a Bun server (`pattern rest` writes `server.ts` and this):
`fetch` with a bearer token, polled. The server listens at app.ts's
`API` and answers CORS, the `OPTIONS` preflight too (the page is another origin).

```contract
routes nav
  home "/"

shape Items
  ok: bool
  error: string
  titles: list<string>

shape Reply
  ok: bool
  error: string

component Shared
  state persona = "Ada" persist
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
    if trim(draft) != "" and not pending(added)
      send added = addItem(persona, trim(draft))
  action afterAdd
    if addError == ""
      draft = ""
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="list" position="absolute" inset=0
          header display="flex" align-items="center" padding="8px 16px"
            text "Shared List" role="heading" aria-level=1
          column padding="0 16px 8px"
            row role="radiogroup" aria-label="Signed in as"
              each p in ["Ada", "Grace"] key=p
                button role="radio" aria-checked=(persona == p) press=signIn(p) testId=`persona-${p}` flex=1
                  text p
          list id="list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              row
                column flex=1 min-width=0 gap=4
                  input value=draft input=edit submit=add placeholder="Add an item" testId="draft"
                  when listError != ""
                    text listError font="-exact-footnote" color="-exact-system-red" testId="error"
              each t in items.titles key=t
                row
                  text t testId=`item-${t}`
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.shared'; // exact new writes this line
export const API = 'http://127.0.0.1:8787'; // the server's one address
export const grants = `net.fetch ${API}`;

async function call(persona: string, path: string, init: RequestInit = {}): Promise<{ error: string; body: unknown }> {
  try {
    const res = await fetch(`${API}${path}`, { ...init, exactTimeout: 10000,
      headers: { authorization: `Bearer ${persona.toLowerCase()}`, 'content-type': 'application/json' } });
    const body = (await res.json().catch(() => ({}))) as { error?: unknown };
    return { error: res.ok ? '' : typeof body.error === 'string' ? body.error : 'The server had a problem. Try again.', body };
  } catch (e) {
    if ((e as { code?: string }).code === 'bake') throw e; // no server at build
    return { error: 'Can’t reach the server. Check that it’s running, then try again.', body: [] };
  }
}

let last: Result<'loadItems'>['titles'] = []; // a failed poll keeps the list on screen
const sources: Sources = {
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
test "another person's item arrives on the poll; the persona is kept"
  type "draft" "Milk"
  type "draft" key "Enter"
  clock data
  expect tree has "item-Milk"
  request POST "/items" {"title": "Bread"} as grace
  clock +4000
  clock data
  expect tree has "item-Bread"
  tap "persona-Grace"
  reload
  expect state persona == "Grace"
```

- **`test` and `verify` run the server themselves** on an empty database: it starts server.ts with `DB`
  naming a scratch file (`new Database(process.env.DB ?? 'data.db')`) and stops it after; a
  server already answering is refused: stop yours.
- **A second person is a test line**: `request POST "/items" {...} as grace` sends `Authorization:
  Bearer grace` (a persona's token is its name); then `clock +4000` (the poll), `clock data`.
- **A persona switcher is a `radiogroup`** kept with `persist`, never a disabled button;
  `loadItems(persona)` reads again when it changes.
- Every failure is words (the server's `error`, or a sentence for a lost connection), never a
  status code. The field clears in `then`, on an ok reply ([after-a-write](after-a-write.md)); a
  failed poll keeps the last list.
