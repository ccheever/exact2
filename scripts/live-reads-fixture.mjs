// The Bluesky clone's three live reads, on a server of this machine (theme2's 23:41Z approval): the clone's
// tests of them asked Bluesky itself, so main's runs of them failed whenever the network did. Each read here is
// what the clone's is, a plain fetch from a TypeScript source, and each answer is DELAYED, so a `clock data`
// that stopped waiting for a fetch in flight fails (the class 566d1aa573 touched). host/web-js/live-reads.test.mjs
// runs the tests below on the JS target, then FAILING sends every read a failure, which each test must fall back
// from, not hang on; host/web/live-reads.test.mjs runs both on the wasm target, against a dist built as `native`
// below says.
//   1. Discover's poll: a timer's re-ask lands within `clock data`, and the scroll-dependent button follows it.
//   2. Explore's tabs: a tap-started fetch is waited on; Top's 403 is the source's to shape (LLP 1016 D4).
//   3. Content & Media: a config read gates the switches; a device-kept setting survives a reload.
import { mkdtempSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

export const CONTRACT = `shape Post
  id: string
  text: string

shape Count
  count: number

shape Actor
  handle: string

shape FeedGen
  name: string

shape Top
  unavailable: bool

shape Config
  topicsEnabled: bool
  answered: bool

shape Trending
  videosOff: bool

component LiveReads
  state reread = 0
  state down = false
  state fresh = false
  state searchTab = ""
  state kept = 0
  resource feed = discover(reread) as shape Post[] else empty()
  mutation latest as shape Count then gotLatest
  resource top = searchPosts(searchTab == "top" ? "bluesky" : "") as shape Top else noTop()
  resource people = searchActors(searchTab == "people" ? "bluesky" : "") as shape Actor[] else empty()
  resource feeds = popularFeeds(searchTab == "feeds") as shape FeedGen[] else empty()
  resource config = getConfig() as shape Config else offConfig()
  resource trending = trendingKept(kept) as shape Trending else offTrending()
  mutation keep as shape Trending then wasKept
  derive posts = feed.length
  derive videosOff = trending.videosOff
  derive pollFailed = failed(latest)
  derive configAnswered = config.answered
  task poll when true key=reread
    every(60000, checkLatest)
  action scrolled(x: number, y: number)
    let d = y > 200
    if d != down
      down = d
  action checkLatest
    if not pending(latest)
      send latest = pollLatest(reread)
  action gotLatest
    match latest
      case some(r)
        if r.count > 0
          fresh = true
      case none
        fresh = fresh
  action loadLatest
    fresh = false
    down = false
    reread = reread + 1
    scrollIntoView("post-0", block="start")
  action search(tab: string)
    searchTab = tab
  action toggleVideos
    send keep = keepVideos(not trending.videosOff)
  action wasKept
    kept = kept + 1
  view
    column height="100%" min-height=0
      row
        button press=search("top") testId="search-tab-top"
          text "Top"
        button press=search("people") testId="search-tab-people"
          text "People"
        button press=search("feeds") testId="search-tab-feeds"
          text "Feeds"
      if top.unavailable
        text "Search is unavailable" testId="search-unavailable"
      if people.length > 0
        column testId="people-results"
          for a in people key=a.handle
            text a.handle
      if failed(people)
        text "People failed" testId="people-failed"
      if feeds.length > 0
        column testId="feed-results"
          for f in feeds key=f.name
            text f.name
      if failed(feeds)
        text "Feeds failed" testId="feeds-failed"
      if config.topicsEnabled
        text "Trending topics" testId="show_trending_topics"
        button press=toggleVideos testId="show_trending_videos"
          text (trending.videosOff ? "Trending videos: off" : "Trending videos: on")
      if down or fresh
        button press=loadLatest testId="loadLatestBtn"
          text "Load new posts"
      list testId="feed" scroll=scrolled flex=1 min-height=0
        for p in feed key=p.id
          text p.text id=p.id testId=p.id height=60
`;

/** The tests with every read answered. */
export const TESTS = `test "Discover's poll: a timer's re-ask lands within clock data, and the load-new-posts button follows the scroll"
  clock data
  expect state posts == 30
  expect tree missing "loadLatestBtn"
  tap "feed" drag 0 -400
  clock settle
  expect state down == true
  expect tree has "loadLatestBtn"
  tap "loadLatestBtn"
  clock settle
  expect state fresh == false
  expect state down == false
  expect tree missing "loadLatestBtn"
  clock data
  expect state posts == 31
  clock + 61000
  clock data
  expect state fresh == true
  expect tree has "loadLatestBtn"
  tap "loadLatestBtn"
  clock data
  expect state posts == 32
  expect state fresh == false
  expect tree missing "loadLatestBtn"

test "Explore's tabs: each tap's fetch is waited on, and Top's 403 is the source's to shape"
  clock data
  tap "search-tab-top"
  clock data
  expect tree has "search-unavailable"
  expect tree missing "people-results"
  tap "search-tab-people"
  clock data
  expect tree has "people-results"
  tap "search-tab-feeds"
  clock data
  expect tree has "feed-results"

test "Content & Media: a config read gates the switches, and the setting kept on the device survives a reload"
  clock data
  expect tree has "show_trending_topics"
  expect tree has "show_trending_videos"
  expect state videosOff == false
  tap "show_trending_videos"
  clock data
  expect state videosOff == true
  reload
  clock data
  expect tree has "show_trending_videos"
  expect state videosOff == true
`;

/** The same reads failing: each falls back, and every `clock data` still ends. */
export const FAILING_TESTS = `test "a poll that fails leaves no new posts"
  clock data
  expect state posts == 0
  clock + 61000
  clock data
  expect state pollFailed == true
  expect state fresh == false
  expect tree missing "loadLatestBtn"

test "a search that fails says so"
  clock data
  tap "search-tab-people"
  clock data
  expect tree has "people-failed"
  expect tree missing "people-results"
  tap "search-tab-feeds"
  clock data
  expect tree has "feeds-failed"

test "a config read that fails shows no switches"
  clock data
  expect state configAnswered == true
  expect tree missing "show_trending_topics"
  expect tree missing "show_trending_videos"
`;

/** Bluesky's three reads, each answered after `delay` ms: Discover's feed (30 posts, one more for each load of new posts)
 * and its poll (`count` new), searchPosts (403, as Bluesky answers a signed-out search), searchActors,
 * getPopularFeedGenerators and unspecced.getConfig, typed as Bluesky types them. `failing` answers every one
 * 500. `seen` lists the paths asked, in order. */
export function blueskyServer({ delay = 300, port = 0 } = {}) {
  const seen = [];
  const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*', 'content-type': 'application/json' };
  const state = { failing: false };
  const json = (body, status = 200) => new Promise(ok => setTimeout(() => ok(new Response(JSON.stringify(body), { status, headers: cors })), delay));
  const server = Bun.serve({ hostname: '127.0.0.1', port, idleTimeout: 0, fetch(req) {
    if (req.method === 'OPTIONS') return new Response(null, { headers: cors });
    const url = new URL(req.url), path = url.pathname;
    seen.push(path);
    if (state.failing) return json({ error: 'InternalServerError', message: 'down' }, 500);
    switch (path) {
      case '/xrpc/app.bsky.feed.getFeed': {
        // Read again after `n` loads of new posts: one more each time, however often a host asks.
        const n = Number(url.searchParams.get('n') ?? 0);
        const feed = Array.from({ length: 30 + n }, (_, i) => ({ post: { uri: `at://post/${i}`, record: { text: `post ${i}` } } }));
        return json({ feed });
      }
      case '/xrpc/app.bsky.feed.getFeed.poll': return json({ feed: [{ post: { uri: 'at://post/new', record: { text: 'new' } } }] });
      case '/xrpc/app.bsky.feed.searchPosts': return json({ error: 'Forbidden', message: 'Search is unavailable' }, 403);
      case '/xrpc/app.bsky.actor.searchActors': return json({ actors: [{ did: 'did:plc:1', handle: 'bsky.app' }] });
      case '/xrpc/app.bsky.unspecced.getPopularFeedGenerators': return json({ feeds: [{ uri: 'at://feed/1', displayName: 'Discover' }] });
      case '/xrpc/app.bsky.unspecced.getConfig': return json({ checkEmailConfirmed: true, topicsEnabled: true });
      default: return json({ error: 'NotFound' }, 404);
    }
  } });
  return {
    origin: `http://127.0.0.1:${server.port}`, seen, state,
    stop: () => server.stop(true),
  };
}

/** The app in a fresh directory under `root` (target/), reading `origin`. */
export function liveReadsApp(root, origin) {
  const dir = mkdtempSync(resolve(root, 'target/live-reads-test-'));
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Live Reads', app: { id: 'test.livereads', name: 'Live Reads' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), CONTRACT);
  writeFileSync(resolve(dir, 'app.ts'), `export const appId = 'test.livereads';
export const grants = ${JSON.stringify(`net.fetch ${origin}\nsecret.keep trending`)};
const origin = ${JSON.stringify(origin)};
const get = (path: string) => fetch(origin + '/xrpc/' + path).then(r => r.json().then((body: any) => ({ ok: r.ok, status: r.status, body })));
const must = (r: { ok: boolean; status: number; body: any }) => { if (!r.ok) throw new Error('HTTP ' + r.status); return r.body; };
export function answer(name: string, args: any[], store: any): any {
  switch (name) {
    case 'discover': return get('app.bsky.feed.getFeed?n=' + args[0]).then(must).then(b => b.feed.map((f: any, i: number) => ({ id: 'post-' + i, text: f.post.record.text })));
    case 'pollLatest': return get('app.bsky.feed.getFeed.poll').then(must).then(b => ({ count: b.feed.length }));
    case 'noTop': return { unavailable: false };
    case 'searchPosts': return args[0] === '' ? { unavailable: false } : get('app.bsky.feed.searchPosts').then(r => {
      if (r.status === 403) return { unavailable: true };
      must(r);
      return { unavailable: false };
    });
    case 'searchActors': return args[0] === '' ? [] : get('app.bsky.actor.searchActors?q=bluesky').then(must).then(b => b.actors.map((a: any) => ({ handle: a.handle })));
    case 'popularFeeds': return !args[0] ? [] : get('app.bsky.unspecced.getPopularFeedGenerators').then(must).then(b => b.feeds.map((f: any) => ({ name: f.displayName })));
    case 'offConfig': return { topicsEnabled: false, answered: false };
    case 'getConfig': return get('app.bsky.unspecced.getConfig').then(r => ({ topicsEnabled: r.ok && r.body.topicsEnabled === true, answered: true }));
    case 'offTrending': return { videosOff: false };
    case 'trendingKept': return { videosOff: store.get('trending') === 'off' };
    case 'keepVideos': { store.set('trending', args[0] ? 'off' : 'on'); return { videosOff: args[0] === true }; }
    default: throw new Error('no source ' + name);
  }
}
`);
  return dir;
}

// `bun scripts/live-reads-fixture.mjs native <dir> [port]`: the app as an `exact new` app at `dir` (outside the
// repo), reading 127.0.0.1:<port> (47712), for the wasm build host/web/live-reads.test.mjs runs against.
if (import.meta.main) {
  const [verb, dir, port = '47712'] = process.argv.slice(2);
  if (verb !== 'native' || !dir) { console.error('usage: bun scripts/live-reads-fixture.mjs native <dir> [port]'); process.exit(2); }
  const { spawnSync } = await import('node:child_process');
  const { cpSync, rmSync } = await import('node:fs');
  const root = resolve(import.meta.dir, '..');
  const made = spawnSync(process.execPath, ['scripts/exact.mjs', 'new', resolve(dir), '--name', 'Live Reads', '--bundle-id', 'test.livereads'], { cwd: root, stdio: 'inherit' });
  if (made.status) process.exit(made.status);
  const app = liveReadsApp(root, `http://127.0.0.1:${port}`);
  for (const f of ['app.contract', 'app.ts']) cpSync(resolve(app, f), resolve(dir, f));
  rmSync(app, { recursive: true, force: true });
  rmSync(resolve(dir, 'app.test.contract'), { force: true });
  console.log(`${dir}: build its wasm dist with (cd ${dir} && EXACT_APP_DIR=${resolve(dir)} EXACT_WEB_DIST=<dist> bun ${root}/host/web/build.mjs <its name>-web --wasm)`);
}
