# Recipe: fetch with loading, error and retry

A screen that loads over the network: a spinner the first time, the error as words
with Try Again, pull to refresh after that.

```contract
routes nav
  home "/"

shape Post
  id: string
  title: string

shape Feed
  ok: bool
  error: string
  posts: list<Post>

component App
  resource feed = loadFeed() as shape Feed else empty(ok=true)
  action reload
    refresh feed
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="feed-list" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" align-items="center" padding="8px 16px"
            text "Feed" role="heading" aria-level=1
          scroll id="feed-list" refresh=reload refreshing=(pending(feed) and length(feed.posts) > 0) flex=1 min-height=0
            when pending(feed) and length(feed.posts) == 0
              row justify-content="center" padding=40
                progress aria-label="Loading" testId="feed-loading"
            else when not feed.ok
              column align-items="center" gap=8 padding=32 testId="feed-error"
                text "Couldn’t Load Posts" role="heading" aria-level=3
                text feed.error color="-exact-secondary-label" text-align="center"
                button press=reload testId="retry"
                  text "Try Again"
            else
              each p in feed.posts key=p.id
                text p.title padding="10px 16px" testId=`post-${p.id}`
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.feed'; // exact new writes this line
export const grants = 'net.fetch https://jsonplaceholder.typicode.com';

const failed = (error: string): Result<'loadFeed'> => ({ ok: false, error, posts: [] });

const sources: Sources = {
  loadFeed: async () => {
    try {
      const res = await fetch('https://jsonplaceholder.typicode.com/posts?_limit=10', { exactTimeout: 10000 });
      if (!res.ok) return failed('The server had a problem. Try again in a moment.');
      const posts = (await res.json()) as Array<{ id: number; title: string }>;
      return { ok: true, error: '', posts: posts.map((p) => ({ id: String(p.id), title: p.title })) };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e; // no network at build: stay unbaked
      return failed('Check your connection and try again.');
    }
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "the feed shows an error, then retries"
  fail fetch "https://jsonplaceholder.typicode.com"
  expect tree has "feed-error"
  pass fetch "https://jsonplaceholder.typicode.com"
  tap "retry"
  clock data
  expect tree missing "feed-error"
```

- The error is data the source answers (`ok: false` and a sentence a person can act
  on), so the view branches on it. Never show a status code, an exception's text or a
  "Loading…" line once the data is in.
- `else empty(ok=true)` keeps the first frame from showing the error before the
  answer. `progress` without a `value` is the system's activity indicator.
- `refresh=` on the scroller is pull to refresh; `refreshing` holds its spinner until
  the reload lands. Map each row field by field: an extra field fails the resource.
- Grant each origin (`net.fetch https://…`). `exactTimeout` bounds a request. Test the
  error path with `fail fetch "<prefix>"` and `pass fetch`. Your own server, polling
  and tokens: [rest-backend](rest-backend.md).
