# Recipe: fetch with loading, error and retry

```contract
shape Feed
  owner: string
  posts: string[]

component App
  state account = "default"
  resource feed = loadFeed(account) as shape Feed else empty()
  action reload
    refresh feed
  view
    column
      button press=reload testId="refresh"
        text "Refresh"
      scroll refresh=reload refreshing=(pending(feed) and feed.owner == account) flex=1 min-height=0
        if pending(feed)
          progress aria-label="Loading" testId="loading"
        if failed(feed)
          text "Couldn't load the feed." testId="error"
          button press=reload testId="retry"
            text "Try Again"
        if feed.owner == account
          if feed.posts.length == 0 and not failed(feed)
            text "No posts yet."
          for p, i in feed.posts key=p
            text p testId=`post-${i}`
```

`empty()` sets `owner` to `""`; account IDs must be nonempty. The owner check
hides A's retained rows after switching to B, even if B fails. A failed refresh
of A keeps its rows beside Retry; a valid empty answer replaces them.
Use the same check for a server or workspace: its answer names that owner.

```ts
import type { Answer, Result } from './app.contract.d.ts';

const API = 'https://jsonplaceholder.typicode.com';
export const grants = `net.fetch ${API}`;

function isPost(p: unknown): p is { title: string } {
  return p !== null && typeof p === 'object' && 'title' in p && typeof p.title === 'string';
}

export const answer: Answer = async (_source, [owner]): Promise<Result<'loadFeed'>> => {
  const response = await fetch(`${API}/posts?_limit=10`, { exactTimeout: 10000 });
  if (!response.ok) {
    throw new Error('The feed request failed.');
  }
  const body: unknown = await response.json();
  if (!Array.isArray(body) || !body.every(isPost)) {
    throw new Error('The feed response is invalid.');
  }
  return { owner, posts: body.map((p): string => p.title) };
};
```

```contract-test
test "a failed refresh retains rows and retries"
  clock data
  fail fetch "https://jsonplaceholder.typicode.com"
  tap "refresh"
  expect tree has "loading"
  expect tree has "post-0"
  clock data
  expect tree has "post-0"
  expect tree has "error"
  pass fetch "https://jsonplaceholder.typicode.com"
  tap "retry"
  clock data
  expect tree missing "error"
```

`fail fetch` rejects; it cannot delay or supply an HTTP status/body. Use a
[test server](rest-backend.md) for those.

For private data, also gate by the authorized principal and auth generation:
A → B → A must reject the first A session's answers. Check it before writing a cache
after an await. Await session prerequisites in the source; dependencies do not
sequence requests. See the
[data guide](../contract-for-agents.md#data-requests-and-side-effects).

Development `state.resourceStatus` reports retained answers and argument equality; `failed` and `failures` give the error and count. It changes neither
retention nor authorization. `with` only admits persisted answers at boot.
