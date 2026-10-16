# Recipe: fetch with loading, error and retry

A screen that loads over the network: a spinner first, the error as words with Try Again,
then pull to refresh.

```contract
routes nav
  home "/"

shape Feed
  ok: bool
  error: string
  posts: string[]

component App
  resource feed = loadFeed() as shape Feed else empty(ok=true)
  action reload
    refresh feed
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="feed-list" position="absolute" inset=0
          header padding="8px 16px"
            text "Feed" role="heading" aria-level=1
          scroll id="feed-list" refresh=reload refreshing=(pending(feed) and feed.posts.length > 0) flex=1 min-height=0
            if pending(feed) and feed.posts.length == 0
              row justify-content="center" padding=40
                progress aria-label="Loading"
            else if not feed.ok
              column align-items="center" gap=8 padding=32 testId="feed-error"
                text feed.error color="-exact-secondary-label" text-align="center"
                button press=reload testId="retry"
                  text "Try Again"
            else
              for p in feed.posts key=p
                text p padding="10px 16px"
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.feed';
const API = 'https://jsonplaceholder.typicode.com';
export const grants = `net.fetch ${API}`;

const failed = (error: string): Result<'loadFeed'> => ({ ok: false, error, posts: [] });

const sources: Sources = {
  loadFeed: async () => {
    try {
      const res = await fetch(`${API}/posts?_limit=10`, { exactTimeout: 10000 });
      if (!res.ok) return failed('The server had a problem. Try again.');
      const posts = (await res.json()) as Array<{ title: string }>;
      return { ok: true, error: '', posts: posts.map((p) => p.title) };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e; // no network at build
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

- The error is data the source answers (`ok: false` and a sentence a person can act on); the
  view branches on it, never on a status code. `else empty(ok=true)` hides it on the first frame.
- `progress` without a `value` is the activity indicator; `refresh=` is pull to refresh and
  `refreshing` holds its spinner. Map rows field by field: an extra field fails the resource.
- Grant each origin; `exactTimeout` bounds a request. Your own server: [rest-backend](rest-backend.md).
