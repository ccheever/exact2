// Adapted from T3 Code 7c3b3b3605ef (MIT); see LICENSE-T3.
// The ranking ⌘K, the sidebar's search and the composer's @ threads share: packages/shared/src/paletteSearch.ts
// (word starts, typo tolerance, use and picks), with client-runtime/src/state/threadSearch.ts (the subject a thread
// shell ranks by, this client's own opens, shown rows kept in place) and threadSearchPullRequests
// (packages/shared/src/threadPullRequests.ts). Grading messages (messageMatcher) and recording opens
// (recordThreadOpen) are the embedded server's, so they are not here.
import { arr, num, obj, str, type Obj } from '../client/domain';
import { visiblePullRequests } from '../panel/shell-pr';

const ALPHANUMERIC = /[\p{L}\p{N}]/u;
const LOWERCASE = /\p{Ll}/u;
const UPPERCASE = /\p{Lu}/u;
const DIGIT = /\p{N}/u;
const LETTER = /\p{L}/u;
const COMBINING_MARKS = /\p{M}/gu;
// Emoji and other symbols can be searched for, so they start words too.
const SYMBOL = /\p{So}/u;
const APOSTROPHE = /['’]/;
const CURLY_APOSTROPHES = /[‘’]/g;

interface SearchField {
  /** Case- and accent-folded text, one UTF-16 unit per source unit. */
  readonly text: string;
  /** 1 where a word starts in `text`. */
  readonly wordStarts: Uint8Array;
}
export interface SearchRange { readonly start: number; readonly end: number }

// Titles, branches and project names repeat across keystrokes; folding them once keeps a keystroke to plain string
// scans. Each keystroke reads every field, so the limit must hold all of them or the cache clears every time.
const fieldCache = new Map<string, SearchField>();
const FIELD_CACHE_LIMIT = 100_000;

function foldCharacter(character: string): string {
  // A typed straight apostrophe finds a curly one: iOS keyboards type "’".
  return character.normalize('NFKD').replace(COMBINING_MARKS, '').toLowerCase().replace('ς', 'σ').replace(/[‘’]/, "'");
}
/**
 * `text` with case and accents folded one UTF-16 unit at a time, so offsets and word starts still read from `text`
 * and highlights need no mapping. A letter that folds to more or less than one unit ("ß", "ﬁ") stays as it is.
 */
function foldInPlace(text: string): string {
  return text.replace(/[\u0080-￿]/g, character => {
    const folded = foldCharacter(character);
    return folded.length === 1 ? folded : character;
  }).toLowerCase();
}
// Chinese and Japanese text has no spaces, so each ideograph or kana starts a word.
function isCjk(character: string): boolean {
  const code = character.codePointAt(0)!;
  return (code >= 0x3040 && code <= 0x30ff) || (code >= 0x3400 && code <= 0x4dbf) || (code >= 0x4e00 && code <= 0x9fff) || (code >= 0xf900 && code <= 0xfaff);
}
// Two Chinese, Japanese, or Korean characters already make a common word.
function hasCjkWord(token: string): boolean {
  return Array.from(token).some(character => {
    const code = character.codePointAt(0)!;
    return isCjk(character) || (code >= 0xac00 && code <= 0xd7af);
  });
}
/**
 * Whether `character` starts a word after `previous` and `beforePrevious` ("" at the start): after a non-letter, at a
 * camelCase or digit boundary, or at any Chinese or Japanese character. The one rule for titles and messages.
 */
function isWordStart(beforePrevious: string, previous: string, character: string, next: string): boolean {
  // "Julius’s": a possessive or contraction does not start a word.
  const afterApostrophe = APOSTROPHE.test(previous) && ALPHANUMERIC.test(beforePrevious);
  return (ALPHANUMERIC.test(character) || SYMBOL.test(character)) && !afterApostrophe && (previous === '' || !ALPHANUMERIC.test(previous)
    || (LOWERCASE.test(previous) && UPPERCASE.test(character))
    // The last capital of an acronym starts the next word: "JSONSchema".
    || (UPPERCASE.test(previous) && UPPERCASE.test(character) && LOWERCASE.test(next))
    || DIGIT.test(previous) !== DIGIT.test(character) || isCjk(character));
}
function foldSearchField(source: string): SearchField {
  const wordStarts = new Uint8Array(source.length);
  const characters = Array.from(source);
  let index = 0;
  for (const [position, character] of characters.entries()) {
    if (isWordStart(characters[position - 2] ?? '', characters[position - 1] ?? '', character, characters[position + 1] ?? '')) wordStarts[index] = 1;
    index += character.length;
  }
  return { text: foldInPlace(source), wordStarts };
}
function prepareSearchField(source: string): SearchField {
  const cached = fieldCache.get(source);
  if (cached) return cached;
  // Oldest first out. A read does not re-insert: that would cost more than the fold it saves.
  if (fieldCache.size >= FIELD_CACHE_LIMIT) fieldCache.delete(fieldCache.keys().next().value!);
  const field = foldSearchField(source);
  fieldCache.set(source, field);
  return field;
}

// Quotes around a pasted query: "fix sidebar".
const WRAPPING_QUOTES = /^\s*["'`“”‘’]+|["'`“”‘’]+\s*$/g;
/** Folded, whitespace-collapsed query; empty when there is nothing to match. */
export function foldSearchQuery(query: string): string {
  return foldInPlace(query.replace(WRAPPING_QUOTES, '').normalize('NFC')).split(/\s+/).filter(token => token.length > 0).join(' ');
}

function wordEnd(field: SearchField, index: number): number {
  let end = index + 1;
  while (end < field.text.length && !field.wordStarts[end] && ALPHANUMERIC.test(field.text[end]!)) end++;
  return end;
}
// One substitution, insertion, deletion, or swap of neighbours; with `swapOnly`, only a swap.
function withinOneEdit(left: string, right: string, swapOnly: boolean): boolean {
  if (Math.abs(left.length - right.length) > (swapOnly ? 0 : 1)) return false;
  let index = 0;
  while (index < left.length && index < right.length && left[index] === right[index]) index++;
  const restLeft = left.slice(index), restRight = right.slice(index);
  const swapped = restLeft.length > 1 && restLeft[0] === restRight[1] && restLeft[1] === restRight[0] && restLeft.slice(2) === restRight.slice(2);
  if (swapOnly) return swapped;
  return swapped || restLeft === restRight || restLeft.slice(1) === restRight.slice(1) || restLeft.slice(1) === restRight || restLeft === restRight.slice(1);
}

const WHOLE_WORD = 1;
// The bonus for a field equal to the whole query lifts a whole word to this.
const WHOLE_NAME = 1.5;
const MID_WORD = 0.3;
const TYPO = 0.45;
// Whole words typed together are exact, so they rank with typos and above mid-word hits, below a word-start match.
const JOINED_WORDS = 0.6;

interface TokenMatch extends SearchRange {
  readonly score: number;
  /** Separate ranges, for words typed together. */
  readonly letters?: ReadonlyArray<SearchRange>;
}
/**
 * Mid-word hits need four or more characters and `midWord`, so "del" does not find "model". Typos keep the first
 * letter except for swaps; short tokens allow fewer edits. Numbers never match with a typo, so pull request numbers
 * stay distinct.
 */
function matchToken(field: SearchField, token: string, typos: boolean, isPrimary: boolean, midWord: boolean): TokenMatch | null {
  if (token.length === 0) return null;
  let best: TokenMatch | null = null;
  const ownBoundary = !ALPHANUMERIC.test(token[0]!) && !SYMBOL.test(token[0]!);
  for (let index = field.text.indexOf(token); index !== -1; index = field.text.indexOf(token, index + 1)) {
    // A token that opens with punctuation (".ts") brings its own boundary, as in messages.
    if (field.wordStarts[index] || ownBoundary) {
      // Whole when the match ends where a word ends: "t3c" is a prefix of "t3code", ".ts" whole in "index.ts" but a
      // prefix in "parser.tsx".
      const end = index + token.length;
      const whole = end >= field.text.length || field.wordStarts[end] === 1 || !ALPHANUMERIC.test(field.text[end]!);
      let runEnd = end;
      while (runEnd < field.text.length && ALPHANUMERIC.test(field.text[runEnd]!)) runEnd++;
      const score = whole ? WHOLE_WORD : 0.7 + (0.2 * token.length) / (runEnd - index);
      if (best === null || score > best.score) best = { score, start: index, end: index + token.length };
      if (score === WHOLE_WORD) return best;
    } else if (midWord && token.length >= 4 && best === null) {
      best = { score: MID_WORD, start: index, end: index + token.length };
    }
  }
  // Joined words are exact, so they count without the typo pass: a strong match elsewhere must not hide "VS Code" for
  // "vscode".
  if ((best === null || best.score <= MID_WORD) && isPrimary) {
    const joined = matchJoinedWords(field, token);
    if (joined) return joined;
  }
  if ((best !== null && best.score > MID_WORD) || !typos || token.length < 3) return best;
  // Numbers skip typos to keep pull requests distinct. Words with digits ("t3code") do not.
  if (LETTER.test(token)) {
    for (let index = 0; index < field.text.length; index++) {
      if (!field.wordStarts[index]) continue;
      // People rarely mistype the first letter, so a typo keeps it, unless it swaps the first two of a longer word
      // ("ing" is not "nig…").
      const first = field.text[index];
      const firstLetterKept = first === token[0] || (token.length > 3 && first === token[1] && field.text[index + 1] === token[0]);
      if (!firstLetterKept) continue;
      // The whole run, across camelCase joins: "githbu" is "GitHub".
      let runEnd = index + 1;
      while (runEnd < field.text.length && ALPHANUMERIC.test(field.text[runEnd]!)) runEnd++;
      const word = field.text.slice(index, runEnd);
      // Short tokens forgive a swapped pair or one missing letter ("srv" for "serv…"); any other edit of a three- or
      // four-letter word matches too much ("cron" would find every "con…").
      const short = token.length <= 4;
      // The same length first, so "githbu" highlights all of "GitHub".
      const lengths = short ? [token.length, token.length + 1] : [token.length, token.length - 1, token.length + 1];
      for (const length of lengths) {
        if (length > word.length) continue;
        if (withinOneEdit(token, word.slice(0, length), short && length === token.length)) return { score: TYPO, start: index, end: index + length };
      }
    }
  }
  return best;
}
/**
 * Short words followed by a short prefix are noise inside a title ("inth" in "in thread"). Allow them at its start
 * ("t3co"), before three letters ("gomod"), or a whole word ("vscode").
 */
function matchJoinedWords(field: SearchField, token: string): TokenMatch | null {
  const firstWord = field.wordStarts.indexOf(1);
  for (let start = 0; start < field.text.length; start++) {
    if (!field.wordStarts[start] || field.text[start] !== token[0]) continue;
    const runs: SearchRange[] = [];
    let at = start, rest = token;
    while (rest.length > 0 && at < field.text.length) {
      const end = wordEnd(field, at);
      if (rest.length <= end - at && field.text.startsWith(rest, at)) {
        runs.push({ start: at, end: at + rest.length });
        rest = '';
      } else if (rest.startsWith(field.text.slice(at, end))) {
        runs.push({ start: at, end });
        rest = rest.slice(end - at);
        at = end;
        while (at < field.text.length && !field.wordStarts[at]) at++;
      } else {
        break;
      }
    }
    const last = runs.at(-1);
    if (rest.length > 0 || last === undefined || runs.length < 2) continue;
    if (last.end === wordEnd(field, last.start) || last.end - last.start >= 3 || start === firstWord || runs.slice(0, -1).every(run => run.end - run.start >= 3)) {
      return { score: JOINED_WORDS, start, end: last.end, letters: runs };
    }
  }
  return null;
}

interface WeightedSearchField {
  readonly value: string;
  readonly weight: number;
  /** Only a whole-word match counts, as for a short query against keywords. */
  readonly wholeWords?: boolean;
  /** Only a match at a word start counts, without typos: loosely matched hidden keywords would list unexplained results. */
  readonly keyword?: boolean;
  /** Where the thread lives (project, environment): it helps a match, but a word start there does not make it strong. */
  readonly context?: boolean;
  /** A token of four or more letters may match inside a word here: only thread titles ("arch" in "Search" is junk). */
  readonly midWord?: boolean;
}
interface SearchFieldsMatch {
  /** Roughly 0.3 (inside a word) to 1.5 (exact title). */
  readonly quality: number;
  /** Every token matched at a word start, without a typo. */
  readonly strong: boolean;
  /** Source ranges matched in the first field, for highlighting. */
  readonly primaryRanges: ReadonlyArray<SearchRange>;
}

const FILLER_WORDS = new Set(['a', 'an', 'and', 'for', 'in', 'of', 'on', 'the', 'to', 'with']);
// Sorted, with overlapping ranges joined.
function mergeRanges(ranges: ReadonlyArray<SearchRange>): SearchRange[] {
  const merged: SearchRange[] = [];
  for (const range of [...ranges].sort((left, right) => left.start - right.start)) {
    const last = merged.at(-1);
    if (last && range.start <= last.end) merged[merged.length - 1] = { start: last.start, end: Math.max(last.end, range.end) };
    else merged.push({ start: range.start, end: range.end });
  }
  return merged;
}
/**
 * Every query token must match some field. Quality is the mean of each token's best weighted match, plus a bonus when
 * the first field holds the whole query as a phrase or equals it.
 */
function matchSearchFields(normalizedQuery: string, fields: ReadonlyArray<WeightedSearchField>, options?: { readonly typos?: boolean }): SearchFieldsMatch | null {
  if (normalizedQuery.length === 0) return null;
  const prepared = fields.filter(field => field.value.length > 0).map(field => ({ field: prepareSearchField(field.value), weight: field.weight,
    wholeWords: field.wholeWords ?? false, keyword: field.keyword ?? false, context: field.context ?? false, allowMidWord: field.midWord ?? false }));
  const primary = fields[0]?.value ? prepared[0]?.field : undefined;
  // An empty token would match everywhere, and loop forever in `matchToken`.
  const tokens = normalizedQuery.split(' ').filter(token => token.length > 0);
  if (tokens.length === 0) return null;
  // A phrase-style query ("fix the sidebar") carries filler words a name may leave out; they count where they match
  // but are not required.
  const fillerOptional = tokens.some(token => !FILLER_WORDS.has(token));
  const primaryRanges: SearchRange[] = [];
  let total = 0, counted = 0, strong = true;
  for (const token of tokens) {
    let best = 0, bestPrimary: TokenMatch | null = null, atWordStart = false;
    for (const { field, weight, wholeWords, keyword, context, allowMidWord } of prepared) {
      if (best >= weight && field !== primary && atWordStart) continue;
      const typos = (options?.typos ?? false) && !keyword;
      // Joined words only count in the name.
      const match = matchToken(field, token, typos, field === primary, allowMidWord);
      if (match === null || match.score < (wholeWords ? WHOLE_WORD : keyword ? 0.7 : 0)) continue;
      if (field === primary) bestPrimary = match;
      if (match.score >= 0.7 && !context) atWordStart = true;
      if (match.score * weight > best) best = match.score * weight;
    }
    if (best === 0) {
      if (fillerOptional && FILLER_WORDS.has(token)) continue;
      return null;
    }
    strong &&= atWordStart;
    if (bestPrimary) primaryRanges.push(...(bestPrimary.letters ?? [bestPrimary]));
    total += best;
    counted += 1;
  }
  let quality = total / counted;
  if (primary) {
    if (primary.text === normalizedQuery) quality += WHOLE_NAME - WHOLE_WORD;
    else if (tokens.length > 1 && primary.text.includes(normalizedQuery)) quality += 0.25;
  }
  return { quality, strong, primaryRanges: mergeRanges(primaryRanges) };
}

// A two-letter Latin substring of a message is noise rather than a match.
const MESSAGE_TOKEN_MIN_LENGTH = 3;
/**
 * Keep accents: the server index folds Latin accents, and stripping marks breaks Korean and Japanese. Every `long`
 * token must match; `short` tokens only narrow hits. No long tokens means no search. Two CJK characters count as long.
 */
export function messageSearchTokens(query: string): { readonly long: ReadonlyArray<string>; readonly short: ReadonlyArray<string>; readonly words: ReadonlyArray<string> } {
  // The index stores apostrophes straight, as the server writes them.
  const tokens = query.replace(WRAPPING_QUOTES, '').replace(CURLY_APOSTROPHES, "'").normalize('NFC').toLowerCase().split(/\s+/).filter(token => token.length > 0);
  const searchable = (token: string) => {
    const length = Array.from(token).length;
    return length >= MESSAGE_TOKEN_MIN_LENGTH || (length === 2 && hasCjkWord(token));
  };
  return { long: tokens.filter(searchable), short: tokens.filter(token => !searchable(token)), words: tokens };
}
/** Matches the query's words side by side as whole words, as typed, whatever whitespace is between them and ignoring case; null for one word. */
export function messagePhrase(query: string): RegExp | null {
  const { words } = messageSearchTokens(query);
  return words.length > 1 ? new RegExp(`(?<![\\p{L}\\p{N}])${words.map(word => word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('\\s+')}(?![\\p{L}\\p{N}])`, 'iu') : null;
}
// A message token shorter than this must start a word: "fts" is not in "drafts".
const MESSAGE_WORD_START_LENGTH = 4;
const MESSAGE_PREFIX = 0.8;

/**
 * Ranges of `text` that contain a long message search token, for message snippets, ignoring case and accents. Shorter
 * tokens would light up half the snippet, and a short long token only counts where it starts a word.
 */
export function findSearchTokenRanges(text: string, query: string): SearchRange[] {
  const tokens = new Map(messageSearchTokens(query).long.map(token => [foldSearchQuery(token), Array.from(token).length < MESSAGE_WORD_START_LENGTH] as const)
    // Some tokens fold to nothing ("´´´"), which would match everywhere.
    .filter(([token]) => token.length > 0));
  if (tokens.size === 0) return [];
  const field = foldSearchField(text);
  const ranges: SearchRange[] = [];
  for (const [token, needsWordStart] of tokens) {
    // A token that opens with punctuation brings its own boundary, as in scoring.
    const ownBoundary = !ALPHANUMERIC.test(token[0]!) && !SYMBOL.test(token[0]!);
    for (let index = field.text.indexOf(token); index !== -1;) {
      if (needsWordStart && !ownBoundary && !field.wordStarts[index]) { index = field.text.indexOf(token, index + 1); continue; }
      ranges.push({ start: index, end: index + token.length });
      index = field.text.indexOf(token, index + token.length);
    }
  }
  return mergeRanges(ranges);
}
export interface SearchTextPart { readonly text: string; readonly highlighted: boolean; readonly start: number }
/** `text` cut into plain and highlighted parts at sorted, non-overlapping `ranges`. */
export function splitSearchText(text: string, ranges: ReadonlyArray<SearchRange>): SearchTextPart[] {
  const parts: SearchTextPart[] = [];
  let cursor = 0;
  for (const range of ranges) {
    if (range.start > cursor) parts.push({ text: text.slice(cursor, range.start), highlighted: false, start: cursor });
    parts.push({ text: text.slice(range.start, range.end), highlighted: true, start: range.start });
    cursor = range.end;
  }
  if (cursor < text.length || parts.length === 0) parts.push({ text: text.slice(cursor), highlighted: false, start: cursor });
  return parts;
}

const DAY_MS = 24 * 60 * 60 * 1000;
/** Both activity recency and open counts halve every two weeks. */
const SEARCH_RECENCY_HALF_LIFE_MS = 14 * DAY_MS;
/** Opens closer together than this count once, so flipping between threads is not frequency. Clients skip sending them. */
export const THREAD_OPEN_DEDUPE_MS = 10 * 60 * 1000;
/** Picked queries are stored this long; clients send no more. */
const THREAD_SEARCH_PICK_MAX_LENGTH = 32;
/**
 * `query` as a pick is stored and compared: folded, then cut to the stored length without splitting an emoji in two.
 * Clients and the server both use it, so a long pick matches its query again.
 */
export function pickedQuery(query: string): string {
  const cut = foldSearchQuery(query).slice(0, THREAD_SEARCH_PICK_MAX_LENGTH);
  return (/[\ud800-\udbff]$/.test(cut) ? cut.slice(0, -1) : cut).trim();
}
function decay(ageMs: number): number { return 2 ** (-Math.max(0, ageMs) / SEARCH_RECENCY_HALF_LIFE_MS); }

/** The server's open history of a thread (OrchestrationV2ThreadOpens). */
export interface ThreadOpenStats { readonly openedAt: number; readonly score: number; readonly picks: ReadonlyArray<{ readonly query: string; readonly at: number }> }
export interface ThreadSearchSubject {
  /** Matched only by the whole query, like references. */
  readonly id?: string;
  readonly title: string;
  readonly branch?: string | null;
  readonly projectTitle?: string | null;
  /** Only set with two or more environments; it tells same-named threads apart. */
  readonly environmentLabel?: string | null;
  readonly pullRequests?: ReadonlyArray<ThreadSearchPullRequest>;
  /** Latest user activity, in epoch ms. */
  readonly activityAt: number | null;
  readonly opens?: ThreadOpenStats | null;
}

// Use reorders matches of about the same quality but never turns a mid-word hit on a busy thread into a better result
// than a whole word on a quiet one.
const USE_BOOST_LIMIT = 2.5;
/** 1 for a stale, never-opened thread, about 2 for one active today, and up to 2.5 for threads opened often. */
function threadUsageBoost(thread: ThreadSearchSubject, now: number): number {
  const recency = thread.activityAt === null ? 0 : decay(now - thread.activityAt);
  const opens = thread.opens ? thread.opens.score * decay(now - thread.opens.openedAt) : 0;
  return Math.min(USE_BOOST_LIMIT, 1 + recency + 0.5 * Math.log2(1 + opens));
}

const REFERENCE_PREFIX_MIN = 8;
// IDs and URLs have many word starts, so field matching would let short queries hit every thread. Require a long
// prefix, or any long part containing a digit. Both are ASCII; lowercasing suffices.
function matchReference(normalizedQuery: string, thread: ThreadSearchSubject): SearchFieldsMatch | null {
  if (normalizedQuery.length < REFERENCE_PREFIX_MIN) return null;
  const references = [...(thread.id ? [thread.id] : []), ...(thread.pullRequests ?? []).flatMap(pullRequest => pullRequest.references ?? [])];
  const anywhere = DIGIT.test(normalizedQuery);
  // A number is never cut short: ".../pull/2288" is not pull request 22889.
  const notCut = (text: string, end: number) => !DIGIT.test(text[end] ?? '');
  return references.some(reference => {
    const folded = reference.toLowerCase();
    const at = folded.indexOf(normalizedQuery);
    // A pasted link may go on past the reference: ".../pull/22889/files".
    return anywhere ? (at !== -1 && notCut(folded, at + normalizedQuery.length)) || (normalizedQuery.startsWith(folded) && notCut(normalizedQuery, folded.length)) : at === 0;
  }) ? { quality: 0.6, strong: true, primaryRanges: [] } : null;
}

// Picks expire from the pick time; opening a thread another way must not extend their priority. They reorder word
// matches within a group, never the groups themselves.
const PICK_WINDOW_MS = 21 * DAY_MS;
// The start of a picked query counts from three letters: "n" is too little to go on.
const PICK_PREFIX_MIN_LENGTH = 3;
/** The latest pick of the thread for this query, while it lasts. */
function activePick(thread: ThreadSearchSubject, normalizedQuery: string, now: number): ThreadOpenStats['picks'][number] | undefined {
  // Stored picks are cut to a length, so compare the query cut the same way.
  const query = pickedQuery(normalizedQuery);
  const prefix = query.length >= PICK_PREFIX_MIN_LENGTH;
  // The latest exact pick, else the latest pick the query starts.
  let exact: ThreadOpenStats['picks'][number] | undefined, latest: ThreadOpenStats['picks'][number] | undefined;
  for (const pick of thread.opens?.picks ?? []) {
    if (now - pick.at > PICK_WINDOW_MS) continue;
    if (pick.query === query && (exact === undefined || pick.at > exact.at)) exact = pick;
    const matches = pick.query === query || (prefix && pick.query.startsWith(query));
    if (matches && (latest === undefined || pick.at > latest.at)) latest = pick;
  }
  return exact ?? latest;
}

export interface SearchMatchHint { readonly label: string; readonly text: string; readonly ranges: ReadonlyArray<SearchRange> }
export interface ThreadSearchMatch {
  /** Match quality times use; what results sort by. */
  readonly score: number;
  /** Match quality alone; see `rankSearchGroups`. */
  readonly quality: number;
  /** Every token matched at a word start; see `rankSearchGroups`. */
  readonly strong: boolean;
  /** Matched ranges in the title. */
  readonly ranges: ReadonlyArray<SearchRange>;
  /** Why the thread matched when its title shows no match, built on demand so only the rows shown pay for it. */
  readonly explain?: () => SearchMatchHint | undefined;
  /** When the thread was picked for this query, while that pick lasts. */
  readonly pickedAt?: number;
  /** Picked for exactly this query, before newer picks for longer ones. */
  readonly pickedExactly?: boolean;
  /** Below the whole-name class; see `scoreThreadSearch`. */
  readonly groupQuality: number;
}

// Most threads' branches start with the app's own worktree prefix ("t3/", and "t3code/" before it), so "t3 connect"
// would match nearly every thread. Only before more text, so typing "t3/" does not empty the list for a keystroke.
const WORKTREE_BRANCH_PREFIX_PATTERN = /^(?:t3|t3code)\/(?=\S)/;
function searchableBranch(branch: string | null | undefined): string | null {
  return branch ? branch.replace(WORKTREE_BRANCH_PREFIX_PATTERN, '') : null;
}
/**
 * The field that explains a match whose title shows none: the first that holds the whole query, else, past the words
 * the project or environment holds, the first that holds the rest ("lakebed perf" in project "lakebed" with the PR
 * title "perf: …").
 */
function matchHint(normalizedQuery: string, thread: ThreadSearchSubject, typos: boolean): SearchMatchHint | undefined {
  const candidates: Array<{ readonly label: string; readonly text: string | null | undefined }> = [
    ...(thread.pullRequests ?? []).map(pullRequest => ({ label: 'PR', text: `#${pullRequest.number}${pullRequest.title ? ` ${pullRequest.title}` : ''}` })),
    { label: 'Branch', text: searchableBranch(thread.branch) }, { label: 'Project', text: thread.projectTitle }, { label: 'Environment', text: thread.environmentLabel }];
  const holds = (text: string, query: string) => matchSearchFields(query, [{ value: text, weight: 1 }], { typos })?.primaryRanges;
  const hintFor = (query: string) => {
    for (const { label, text } of candidates) {
      const ranges = text ? holds(text, query) : undefined;
      if (text && ranges) return { label, text, ranges };
    }
    return undefined;
  };
  const whole = hintFor(normalizedQuery);
  if (whole) return whole;
  const contexts = [thread.projectTitle, thread.environmentLabel].filter((text): text is string => !!text);
  const rest = normalizedQuery.split(' ').filter(token => !contexts.some(text => holds(text, token))).join(' ');
  return rest && rest !== normalizedQuery ? hintFor(rest) : undefined;
}
/**
 * Scores a thread for a normalized query, or null when it does not match. Titles and pull request numbers count
 * fully, pull request titles and branches less, project and environment names least. `picks: false` leaves palette
 * picks out where one would show up unexplained (a sidebar filter).
 */
export function scoreThreadSearch(normalizedQuery: string, thread: ThreadSearchSubject, now: number, options?: { readonly typos?: boolean; readonly picks?: boolean }): ThreadSearchMatch | null {
  const fields: WeightedSearchField[] = [{ value: thread.title, weight: 1, midWord: true }];
  for (const pullRequest of thread.pullRequests ?? []) {
    fields.push({ value: `#${pullRequest.number}`, weight: 1 });
    if (pullRequest.title) fields.push({ value: pullRequest.title, weight: 0.5 });
  }
  const branch = searchableBranch(thread.branch);
  if (branch) fields.push({ value: branch, weight: 0.6 });
  if (thread.projectTitle) fields.push({ value: thread.projectTitle, weight: 0.25, context: true });
  if (thread.environmentLabel) fields.push({ value: thread.environmentLabel, weight: 0.25, context: true });
  // Strip pull request prefixes and the worktree prefix so queries match the stored fields.
  const fieldQuery = normalizedQuery.replace(/^(?:pr|pull request|pull)\s*(?=#?\d)/, '').replace(/^#\s*/, '').replace(WORKTREE_BRANCH_PREFIX_PATTERN, '');
  const fieldMatch = matchSearchFields(fieldQuery, fields, options);
  const referenceMatch = fieldMatch ? null : matchReference(normalizedQuery, thread);
  const match = fieldMatch ?? referenceMatch;
  if (match === null) return null;
  // A row whose title shows no match must say why. With several words, one that cannot (its words spread over
  // unrelated fields) is a coincidence and is left out; a single word always sits in the field that matched it, so its
  // hint waits until the row is shown.
  const typos = options?.typos ?? false;
  const titleShowsMatch = match.primaryRanges.length > 0;
  const checkHint = !titleShowsMatch && referenceMatch === null && fieldQuery.includes(' ');
  const checkedHint = checkHint ? matchHint(fieldQuery, thread, typos) : undefined;
  if (checkHint && checkedHint === undefined) return null;
  const explain = titleShowsMatch ? undefined
    : (): SearchMatchHint | undefined => referenceMatch ? { label: 'ID or link', text: normalizedQuery, ranges: [] } : (checkedHint ?? matchHint(fieldQuery, thread, typos));
  const pick = options?.picks === false ? undefined : activePick(thread, normalizedQuery, now);
  return {
    score: match.quality * threadUsageBoost(thread, now),
    quality: match.quality,
    // A thread's whole title never lifts its group over a command or project of that name: every new thread is titled
    // "New thread" for a while.
    groupQuality: Math.min(match.quality, WHOLE_NAME - 0.01),
    strong: match.strong,
    ranges: match.primaryRanges,
    ...(explain ? { explain } : {}),
    ...(pick ? { pickedAt: pick.at } : {}),
    ...(pick?.query === pickedQuery(normalizedQuery) ? { pickedExactly: true } : {}),
  };
}
/** When the user last used a thread: their last message or their last open. */
export function threadRecentUse(thread: ThreadSearchSubject): number {
  return Math.max(thread.activityAt ?? 0, thread.opens?.openedAt ?? 0);
}
/**
 * How strongly a thread's messages match among matches of one quality: a thread that keeps using the word outweighs a
 * recent passing mention, as hits count for more than recency or use (21 hits about 5.5, one hit 2).
 */
function messageMatchWeight(hits: number): number { return 1 + Math.log2(1 + hits); }
/** Server message matches ranked; `withoutTitleMatches` reserves message rows for threads the palette has not already listed. */
export function rankMessageMatches<T extends { readonly quality?: number; readonly hits?: number; readonly messageCreatedAt?: string | null }>(
  matches: ReadonlyArray<T>, subjectOf: (match: T) => ThreadSearchSubject | undefined, normalizedQuery: string, now: number,
  options?: { readonly picks?: boolean; readonly withoutTitleMatches?: boolean }): T[] {
  return matches.flatMap(match => {
    const subject = subjectOf(match);
    if (!subject) return [];
    if (options?.withoutTitleMatches && scoreThreadSearch(normalizedQuery, subject, now, { picks: false }) !== null) return [];
    // Older servers send no quality; their matches count as words.
    const quality = match.quality ?? 1;
    return [{ match, quality, pickedAt: options?.picks === false ? 0 : (activePick(subject, normalizedQuery, now)?.at ?? 0),
      score: messageMatchWeight(match.hits ?? 1) * threadUsageBoost(subject, now) }];
  }).sort((left, right) =>
    // Word matches first; among them a pick, then the closer match.
    Number(right.quality >= MESSAGE_PREFIX) - Number(left.quality >= MESSAGE_PREFIX) || right.pickedAt - left.pickedAt || right.quality - left.quality
      || right.score - left.score || (right.match.messageCreatedAt ?? '').localeCompare(left.match.messageCreatedAt ?? '')).map(({ match }) => match);
}

/** Palette limits. */
export const PALETTE_THREAD_LIMIT = 20;
export const PALETTE_MESSAGE_LIMIT = 10;
/** Next to matching threads, other groups show only their best few rows. */
export const PALETTE_SECONDARY_LIMIT = 3;
// Whole words and prefixes share a class so "Compact" does not lift settings over "Compaction".
function matchClass(quality: number): number { return quality >= WHOLE_NAME ? 3 : quality >= 0.7 ? 2 : quality >= 0.4 ? 1 : 0; }

type RankedMatch = { readonly score: number; readonly quality: number; readonly strong: boolean; readonly pickedAt?: number; readonly pickedExactly?: boolean; readonly groupQuality?: number };
export interface RankedSearchItem<T, M> { readonly item: T; readonly match: M }
/**
 * Rank groups by match class to keep typing stable and thread use from outweighing equal commands. Try typos only
 * without word-start matches: they add noise beside real matches and cost per key.
 */
export function rankSearchGroups<G, T, M extends RankedMatch>(groups: ReadonlyArray<{ readonly group: G; readonly items: ReadonlyArray<T> }>,
  score: (item: T, options: { readonly typos: boolean }, group: G) => M | null): Array<{ readonly group: G; readonly ranked: ReadonlyArray<RankedSearchItem<T, M>> }> {
  const rank = (typos: boolean) => groups.map(({ group, items }) => ({ group, ranked: items.flatMap(item => {
    const match = score(item, { typos }, group);
    return match === null ? [] : [{ item, match }];
  }) }));
  let ranked = rank(false);
  if (!ranked.some(entry => entry.ranked.some(({ match }) => match.strong))) ranked = rank(true);
  return ranked.flatMap(({ group, ranked: items }, order) => {
    if (items.length === 0) return [];
    // Use cannot lift a project-only match above a title match or cross match classes. Picks reorder word matches,
    // keeping typos and mid-word hits below them. A longer-query pick must not pass a whole-name match for the query.
    const wordMatch = (match: M) => Math.min(matchClass(match.quality), 2);
    const exactPickAt = (match: M) => (match.pickedExactly ? (match.pickedAt ?? 0) : 0);
    const sorted = [...items].sort((left, right) => wordMatch(right.match) - wordMatch(left.match) || exactPickAt(right.match) - exactPickAt(left.match)
      || matchClass(right.match.quality) - matchClass(left.match.quality) || (right.match.pickedAt ?? 0) - (left.match.pickedAt ?? 0) || right.match.score - left.match.score);
    const best = Math.max(...items.map(({ match }) => matchClass(match.groupQuality ?? match.quality)));
    return [{ group, ranked: sorted, best, order }];
  }).sort((left, right) => right.best - left.best || left.order - right.order).map(({ group, ranked: items }) => ({ group, ranked: items }));
}
/** `rankSearchGroups` for a single list. */
export function rankSearchItems<T, M extends RankedMatch>(items: ReadonlyArray<T>, score: (item: T, options: { readonly typos: boolean }) => M | null): ReadonlyArray<RankedSearchItem<T, M>> {
  return rankSearchGroups([{ group: null, items }], score)[0]?.ranked ?? [];
}

export interface SearchTermsMatch {
  readonly score: number;
  readonly quality: number;
  /** The quality its group ranks by, when lower; see `scoreSearchTerms`. */
  readonly groupQuality?: number;
  readonly strong: boolean;
  readonly ranges: ReadonlyArray<SearchRange>;
}
const KEYWORD_QUALITY = 0.65;
// Below this, keyword prefixes match too much: "co" or "pro" would list most settings. A short query still matches a
// keyword it equals ("pr").
const KEYWORD_QUERY_MIN_LENGTH = 4;
/**
 * The first term is the name; the rest are keywords, which count less so hidden terms cannot lift a group above a
 * matching name. Short queries need whole keywords, or common prefixes would list most settings.
 */
export function scoreSearchTerms(normalizedQuery: string, terms: ReadonlyArray<string>, options?: { readonly typos?: boolean }): SearchTermsMatch | null {
  // Filler words ("the ci") do not make a short query long.
  const meaningful = normalizedQuery.split(' ').filter(token => !FILLER_WORDS.has(token)).join(' ');
  const short = (meaningful || normalizedQuery).length < KEYWORD_QUERY_MIN_LENGTH;
  const match = matchSearchFields(normalizedQuery, terms.map((value, index) => index === 0 ? { value, weight: 1 }
    // An alias typed in full ("pr") is deliberate, above a mere name prefix.
    : { value, weight: short ? 0.8 : 0.6, wholeWords: short, keyword: true }), options);
  return match && {
    score: match.quality,
    quality: match.quality,
    // Below the word-start class: a match only in hidden keywords must not put its group above a result whose name
    // matches. Inside the group an exact alias still beats a name prefix ("pr" lists "Open pull requests" first).
    ...(match.primaryRanges.length > 0 ? {} : { groupQuality: Math.min(match.quality, KEYWORD_QUALITY) }),
    strong: match.strong,
    ranges: match.primaryRanges,
  };
}

// ── Thread shells as search subjects (client-runtime state/threadSearch.ts) ─────────────────────────────────────────
// When this client last sent an open per thread, by its own clock: a server timestamp would be off by any clock skew.
// The server merges opens closer together than THREAD_OPEN_DEDUPE_MS anyway, so skipping them saves a command.
const sentThreadOpens = new Map<string, number>();
// When this client last opened each thread, sent or not, so recent threads follow quick switches the server merges.
const localThreadOpens = new Map<string, number>();
/** Records that this client opened a thread, and says whether to send the open to the server now. */
export function claimThreadOpen(environmentId: string, threadId: string, now: number): boolean {
  const key = `${environmentId}:${threadId}`;
  localThreadOpens.set(key, now);
  if (now - (sentThreadOpens.get(key) ?? Number.NEGATIVE_INFINITY) < THREAD_OPEN_DEDUPE_MS) return false;
  sentThreadOpens.set(key, now);
  return true;
}
/** A shell's `opens`, or null for a value this build cannot read: open stats only rank, so they never fail a thread. */
function threadOpens(value: unknown): ThreadOpenStats | null {
  const opens = obj(value), openedAt = opens.openedAt, score = opens.score, picks = arr(opens.picks);
  const finite = (field: unknown): field is number => typeof field === 'number' && Number.isFinite(field);
  if (!finite(openedAt) || !finite(score) || score < 0 || !Array.isArray(opens.picks) || picks.length > 3) return null;
  if (picks.some(pick => typeof pick.query !== 'string' || str(pick.query).length > THREAD_SEARCH_PICK_MAX_LENGTH || !finite(pick.at))) return null;
  return { openedAt, score, picks: picks.map(pick => ({ query: str(pick.query), at: num(pick.at) })) };
}
/** getThreadSortTimestamp(thread, "updated_at"): the latest user message, else the thread's own times. */
function latestUserMessageAt(thread: Obj): number {
  const latest = Date.parse(str(thread.latestUserMessageAt));
  if (Number.isFinite(latest)) return latest;
  const messages = arr(thread.messages).filter(message => message.role === 'user').map(message => Date.parse(str(message.createdAt))).filter(Number.isFinite);
  if (messages.length) return Math.max(...messages);
  return [thread.updatedAt, thread.createdAt].map(value => Date.parse(str(value))).find(Number.isFinite) ?? Number.NEGATIVE_INFINITY;
}
/**
 * What the palettes rank a thread shell by. Pass the environment label only when there are two or more environments;
 * `environmentId` keys this client's own opens.
 */
export function threadSearchSubject(thread: Obj, projectTitle: string | null | undefined, environmentLabel?: string | null, environmentId = ''): ThreadSearchSubject {
  // What the user wrote, not agent wakes or task notices in the user role. A thread the user never wrote in counts from
  // its creation; an older server omits the field, so the latest user-role message stands in.
  const authored = thread.latestUserAuthoredMessageAt;
  const activityAt = authored === undefined ? latestUserMessageAt(thread) : Date.parse(str(authored ?? thread.createdAt));
  const opens = threadOpens(thread.opens);
  const localOpenedAt = localThreadOpens.get(`${environmentId}:${str(thread.id)}`) ?? 0;
  return { id: str(thread.id), title: str(thread.title), branch: str(thread.branch) || null, projectTitle: projectTitle ?? null, environmentLabel: environmentLabel ?? null,
    pullRequests: threadSearchPullRequests(thread), activityAt: Number.isFinite(activityAt) ? activityAt : null,
    // This client's own latest open counts too: the server merged it, or has no record yet (an older server ignores opens).
    opens: localOpenedAt > (opens?.openedAt ?? 0) ? { score: 1, picks: [], ...opens, openedAt: localOpenedAt } : opens };
}

/** Message result keys in the order a search box last showed them for `query`. */
export interface ShownOrder {
  readonly query: string;
  readonly keys: ReadonlyArray<string>;
  /** How many of `keys` are on screen; only those hold their places. */
  readonly shown: number;
}
// Rows past the limit a slower environment's best matches may take.
const LATE_ROOM = 3;
/**
 * Keep shown rows in place so late results cannot move the pointer or keyboard target; up to LATE_ROOM extra rows
 * let late best matches show. Returns `previous` when nothing changed.
 */
export function nextShownOrder(previous: ShownOrder | null, query: string, ranked: ReadonlyArray<string>, limit: number): ShownOrder {
  if (previous?.query !== query) return { query, keys: ranked, shown: Math.min(limit, ranked.length) };
  const listed = new Set(ranked);
  const kept = previous.keys.slice(0, previous.shown).filter(key => listed.has(key));
  const held = new Set(kept), seen = new Set(previous.keys);
  const rest = ranked.filter(key => !held.has(key));
  const arrived = rest.filter(key => !seen.has(key)).length;
  const keys = [...kept, ...rest];
  const shown = Math.min(keys.length, limit + LATE_ROOM, Math.max(limit, kept.length + Math.min(arrived, LATE_ROOM)));
  const unchanged = shown === previous.shown && keys.length === previous.keys.length && keys.every((key, index) => key === previous.keys[index]);
  return unchanged ? previous : { query, keys, shown };
}

// ── Pull requests a thread is found by (packages/shared/src/threadPullRequests.ts) ───────────────────────────────────
export interface ThreadSearchPullRequest {
  readonly number: number;
  readonly title?: string | null;
  /** `owner/repo#123` and the pull request URL, matched only by a whole query. */
  readonly references?: ReadonlyArray<string>;
}
const SEARCHED_PULL_REQUEST_TITLES = 3;
const reference = (value: unknown): Obj | null => (value && typeof value === 'object' ? obj(value) : null);
/** threadSearchPullRequests: by number and link every pull request, by title only the newest few. */
export function threadSearchPullRequests(thread: Obj): ThreadSearchPullRequest[] {
  const entry = (link: Obj, title: string | null): ThreadSearchPullRequest => ({ number: num(link.number), title, references: [`${str(link.repository)}#${num(link.number)}`, str(link.url)] });
  const branchLink = reference(thread.branchPullRequest);
  const branch = branchLink ? [entry(branchLink, null)] : [];
  if (arr(thread.pullRequests).length > 0) {
    // A triage thread linked to dozens would match almost any word by title.
    const updated = (link: Obj) => { const ms = Date.parse(str(obj(link.snapshot).updatedAt ?? link.linkedAt)); return Number.isNaN(ms) ? 0 : ms; };
    const newestFirst = [...visiblePullRequests(thread.pullRequests)].sort((left, right) => updated(right) - updated(left));
    return branch.concat(newestFirst.map((link, index) => entry(link, index < SEARCHED_PULL_REQUEST_TITLES ? (str(obj(link.snapshot).title) || null) : null)));
  }
  const legacy = reference(thread.linkedPullRequest);
  return legacy ? branch.concat(entry(legacy, null)) : branch;
}
