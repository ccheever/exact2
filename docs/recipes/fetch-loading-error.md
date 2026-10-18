# Recipe: fetch with loading, error and retry

```contract
routes nav
  home "/"
shape Feed
  owner: string
  posts: string[]

component App
  state account = "default"
  resource feed = loadFeed(account) as shape Feed else empty()
  action reload
    refresh feed
  view
    main navigationKey=`${top(nav).id}` height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="posts" position="absolute" inset=0
          header
            h1 "Feed"
            button press=reload testId="refresh"
              text "Refresh"
          scroll id="posts" refresh=reload refreshing=(pending(feed) and feed.owner == account) flex=1 min-height=0
            if pending(feed)
              progress aria-label="Loading" testId="loading"
            if failed(feed)
              text "Couldn't load the feed. Try again." testId="error"
              button press=reload testId="retry"
                text "Try Again"
            if feed.owner == account
              if feed.posts.length == 0 and not failed(feed)
                text "No posts yet."
              for p, i in feed.posts key=p
                text p testId=`post-${i}`
```

The build-time placeholder cannot read `account`; `empty()` gives `owner` the
value `""`. Account IDs here must be nonempty. The view shows rows only when
`feed.owner == account`, so they remain visible during a refresh or failure. A
validated empty `posts` array replaces the previous rows.

```ts
import type { Answer, Result } from './app.contract.d.ts';

export const appId = 'com.example.feed';
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

For private data, gate answers by the app's authorized principal and auth
generation. Await session prerequisites in the source; resource dependencies do
not sequence requests. See the [data guide](../contract-for-agents.md#data-requests-and-side-effects).
