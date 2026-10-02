import type { Answer, Sources, Result } from './app.contract.d.ts';

// RealWorld's hosted API (docs.realworld.show). The token is a store secret
// (LLP 1018): localStorage on the web, the Keychain on Apple.
export const appId = 'com.exact.realworld';
export const grants = 'net.fetch https://api.realworld.show\nsecret.keep realworld.jwt';
const API = 'https://api.realworld.show/api';
const AVATAR = '/assets/default-avatar.svg';
const PAGE = 10;

type Store = Parameters<Answer>[2];
type Json = Record<string, any>;
type User = Result<'currentUser'>;
type Feed = Result<'articles'>;
type Article = Result<'article'>;
type Profile = Result<'profile'>;
type Author = Profile;

// A mutation's answer runs its `then` (app.contract); what it changes is
// refetched by its `refreshes` declaration.

// Favorites and follows: `favs` and `follows` are the one store of what the
// reader pressed, by article and by author, and every view reads its state
// through them (the `favs` and `follows` resources). A press writes its entry
// before the request goes (the optimistic flip, read by the declared refresh
// at the send), the answer replaces it, a failure puts back what was there.
// The newest press per key wins; a change of reader forgets them all.
type Fav = { slug: string; favorited: boolean; favoritesCount: number };
type Follow = { username: string; following: boolean };
const favs = new Map<string, Fav>(), follows = new Map<string, Follow>();
const presses = new Map<string, number>();
let press = 0;
function flip<T>(store: Map<string, T>, key: string, now: T, work: () => Promise<T>, kind: string): Promise<Change> {
  const before = store.get(key), n = ++press, id = `${kind}:${key}`;
  presses.set(id, n);
  store.set(key, now);
  return change(kind, async () => {
    try {
      const after = await work();
      if (presses.get(id) === n) store.set(key, after);
    } catch (e) {
      if (presses.get(id) === n) { if (before === undefined) store.delete(key); else store.set(key, before); }
      throw e;
    }
    return key;
  });
}
const forget = () => { favs.clear(); follows.clear(); wrote(); };
const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
function date(iso: unknown): string {
  const d = new Date(String(iso));
  return Number.isNaN(d.getTime()) ? '' : `${MONTHS[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()}`;
}
const text = (v: unknown) => (typeof v === 'string' ? v : '');
const avatar = (v: unknown) => text(v) || AVATAR;

class Failure extends Error {
  constructor(readonly status: number, readonly errors: string[]) { super(errors.join('; ')); }
}
function errorList(body: Json | null, status: number): string[] {
  const errors = body?.errors;
  if (errors && typeof errors === 'object') {
    const list = Object.entries(errors).flatMap(([field, v]) =>
      (Array.isArray(v) ? v : [v]).map(m => (field === 'body' ? String(m) : `${field} ${m}`)));
    if (list.length) return list;
  }
  return [status === 401 ? 'You need to sign in first.' : `The server answered ${status || 'nothing'}.`];
}
async function api(store: Store, path: string, method = 'GET', body?: unknown): Promise<Json> {
  const headers: Record<string, string> = {};
  const token = store.get('realworld.jwt');
  if (token) headers.Authorization = `Token ${token}`;
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  // A read needs no FIFO order, so natively (the render server, Apple,
  // Linux) a page's sources fetch together, as a browser's do (LLP 1041
  // §8.4). A mutation stays on the ordered lane.
  const read = method === 'GET' ? { exactIndependentHttp: { maxResponseBytes: 4 << 20 } } : {};
  if (method !== 'GET') wrote();
  let response: Response;
  try {
    response = await fetch(API + path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body), ...read });
  } catch {
    throw new Failure(0, ['The server could not be reached.']);
  }
  if (method !== 'GET') wrote();
  const json = response.status === 204 ? {} : await response.json().catch(() => null);
  if (!response.ok) throw new Failure(response.status, errorList(json, response.status));
  return json ?? {};
}
const failed = (e: unknown) => (e instanceof Failure ? e.errors : [String(e)]);

// Reads answered from what this page already has, by source, arguments and
// reader, until anything writes (as the other RealWorlds' query caches keep
// a route's answers, a served page's included): coming back to a page shows
// it at once, not after a round trip. A write or a change of reader forgets
// them all.
const CACHED = new Set(['popularTags', 'articles', 'article', 'comments', 'profile']);
const answers = new Map<string, unknown>();
let writes = 0, asking = ''; // a read that a write began or ended during is not kept
const wrote = () => { writes++; answers.clear(); };
const keep = (key: string, v: unknown) => {
  if (answers.size >= 64) answers.delete(answers.keys().next().value!);
  answers.set(key, v);
};
const answerKey = (token: string, source: string, args: unknown[]) => `${token} ${source} ${JSON.stringify(args)}`;
function got<T>(store: Store, path: string, build: (json: Json) => T, fail: (e: unknown) => T): Promise<T> {
  const key = asking, at = writes;
  return api(store, path).then(json => {
    const v = build(json);
    if (key && at === writes) keep(key, v);
    return v;
  }).catch(fail);
}

function author(a: Json | undefined): Author {
  return { found: true, username: text(a?.username), bio: text(a?.bio), image: avatar(a?.image), following: !!a?.following };
}
function preview(a: Json) {
  return {
    slug: text(a.slug), title: text(a.title), description: text(a.description),
    tags: Array.isArray(a.tagList) ? a.tagList.map(String) : [], date: date(a.createdAt),
    favorited: !!a.favorited, favoritesCount: Number(a.favoritesCount) || 0, author: author(a.author),
  };
}
const anonymous: User = { signedIn: false, username: '', email: '', bio: '', image: '', avatar: AVATAR };
function user(u: Json | undefined): User {
  return { signedIn: true, username: text(u?.username), email: text(u?.email), bio: text(u?.bio), image: text(u?.image), avatar: avatar(u?.image) };
}
const emptyFeed: Feed = { ready: false, message: '', articles: [], pages: [] };
const emptyArticle: Article = { found: false, slug: '', title: '', description: '', body: '', tags: [], date: '', favorited: false, favoritesCount: 0, author: author(undefined) };
const emptyProfile: Profile = { found: false, username: '', bio: '', image: AVATAR, following: false };

async function currentUser(store: Store): Promise<User> {
  if (!store.get('realworld.jwt')) return anonymous;
  try { return user((await api(store, '/user')).user); } catch (e) {
    if (e instanceof Failure && e.status === 401) store.forget('realworld.jwt');
    return anonymous;
  }
}
function articles(store: Store, kind: string, tag: string, name: string, page: number): Feed | Promise<Feed> {
  if (!kind) return emptyFeed;
  const q = `limit=${PAGE}&offset=${(page - 1) * PAGE}`;
  const path = kind === 'feed' ? `/articles/feed?${q}`
    : kind === 'tag' ? `/articles?tag=${encodeURIComponent(tag)}&${q}`
    : kind === 'author' ? `/articles?author=${encodeURIComponent(name)}&${q}`
    : kind === 'favorited' ? `/articles?favorited=${encodeURIComponent(name)}&${q}`
    : `/articles?${q}`;
  return got(store, path, data => {
    const list = Array.isArray(data.articles) ? data.articles.map(preview) : [];
    const count = Math.ceil((Number(data.articlesCount) || 0) / PAGE);
    return {
      ready: true, message: list.length ? '' : 'No articles are here... yet.', articles: list,
      pages: count > 1 ? Array.from({ length: count }, (_, i) => ({ n: i + 1 })) : [],
    };
  }, e => ({ ...emptyFeed, ready: true, message: failed(e).join(' ') }));
}
function article(store: Store, slug: string): Article | Promise<Article> {
  if (!slug) return emptyArticle;
  return got(store, `/articles/${encodeURIComponent(slug)}`, json => {
    const a = json.article;
    return { ...preview(a), found: true, body: text(a.body) };
  }, () => emptyArticle);
}
function comments(store: Store, slug: string, viewer: string) {
  if (!slug) return [];
  return got(store, `/articles/${encodeURIComponent(slug)}/comments`, json => {
    const list = json.comments;
    return (Array.isArray(list) ? list : []).map((c: Json) => ({
      id: String(c.id), body: text(c.body), date: date(c.createdAt), author: author(c.author),
      mine: viewer !== '' && c.author?.username === viewer,
    }));
  }, () => []);
}
function profile(store: Store, name: string): Profile | Promise<Profile> {
  if (!name) return emptyProfile;
  return got(store, `/profiles/${encodeURIComponent(name)}`, json => author(json.profile), () => emptyProfile);
}

type Auth = Result<'login'>;
async function signIn(store: Store, path: string, body: Json, method = 'POST'): Promise<Auth> {
  try {
    const u = (await api(store, path, method, { user: body })).user;
    if (u?.token) store.set('realworld.jwt', String(u.token));
    forget();
    return { ok: true, errors: [] };
  } catch (e) { return { ok: false, errors: failed(e) }; }
}
type Change = Result<'favorite'>;
async function change(kind: string, work: () => Promise<string>): Promise<Change> {
  try { return { kind, ok: true, slug: await work(), errors: [] }; } catch (e) {
    return { kind, ok: false, slug: '', errors: failed(e) };
  }
}
const slugPath = (slug: string) => `/articles/${encodeURIComponent(slug)}`;

const sources: Sources = {
  currentUser: (_, store) => currentUser(store),
  popularTags: ([home], store) => {
    if (!home) return [];
    return got(store, '/tags', json => (Array.isArray(json.tags) ? json.tags.map(String) : []), () => []);
  },
  articles: ([kind, tag, name, page], store) => articles(store, kind, tag, name, page),
  article: ([slug], store) => article(store, slug),
  comments: ([slug, viewer], store) => comments(store, slug, viewer),
  profile: ([name], store) => profile(store, name),
  login: ([email, password], store) => signIn(store, '/users/login', { email, password }),
  register: ([username, email, password], store) => signIn(store, '/users', { username, email, password }),
  saveSettings: ([image, username, bio, email, password], store) =>
    signIn(store, '/user', { image, username, bio, email, ...(password ? { password } : {}) }, 'PUT'),
  logout: (_, store) => { store.forget('realworld.jwt'); forget(); return { ok: true, errors: [] }; },
  favorites: () => [...favs.values()],
  followings: () => [...follows.values()],
  favorite: ([slug, on, count], store) => flip(favs, slug, { slug, favorited: on, favoritesCount: Math.max(0, count + (on ? 1 : -1)) }, async () => {
    const a = (await api(store, `${slugPath(slug)}/favorite`, on ? 'POST' : 'DELETE')).article;
    return { slug, favorited: !!a?.favorited, favoritesCount: Number(a?.favoritesCount) || 0 };
  }, 'favorite'),
  follow: ([name, on], store) => flip(follows, name, { username: name, following: on }, async () => {
    const p = (await api(store, `/profiles/${encodeURIComponent(name)}/follow`, on ? 'POST' : 'DELETE')).profile;
    return { username: name, following: !!p?.following };
  }, 'follow'),
  publish: ([slug, title, description, body, tagList], store) => change('publish', async () => {
    const a = { title, description, body, tagList };
    const saved = await api(store, slug ? slugPath(slug) : '/articles', slug ? 'PUT' : 'POST', { article: a });
    return text(saved.article?.slug);
  }),
  deleteArticle: ([slug], store) => change('delete', async () => { await api(store, slugPath(slug), 'DELETE'); return slug; }),
  addComment: ([slug, body], store) => change('comment', async () => {
    await api(store, `${slugPath(slug)}/comments`, 'POST', { comment: { body } }); return slug;
  }),
  deleteComment: ([slug, id], store) => change('uncomment', async () => {
    await api(store, `${slugPath(slug)}/comments/${encodeURIComponent(id)}`, 'DELETE'); return slug;
  }),
  addTag: ([session, tags, tag]) => { const t = tag.trim(); return { session, items: t && !tags.includes(t) ? [...tags, t] : tags }; },
  removeTag: ([session, tags, tag]) => ({ session, items: tags.filter(t => t !== tag) }),
};
export const answer: Answer = (source, args, store, storage) => {
  if (!CACHED.has(source)) return sources[source](args, store, storage);
  const key = answerKey(store.get('realworld.jwt') ?? '', source, args);
  if (answers.has(key)) return answers.get(key) as ReturnType<Answer>;
  asking = key;
  try { return sources[source](args, store, storage); } finally { asking = ''; }
};
// The answers this page was rendered with (by the render host, signed out),
// given once at boot on the web (host/web-js/ts-data.js): this module's own from then on.
export const kept = (source: string, args: unknown[], value: unknown) => {
  if (CACHED.has(source)) keep(answerKey('', source, args), value);
};
