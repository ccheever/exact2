// T3 Code 9c161462c9 (MIT, LICENSE-T3): useThreadFind.ts and shared/threadSearch.ts.
export type FindPosition = { entryId: string; occurrence: number };
export type FindMatch = FindPosition & { runId: string | null };
export type FindNavigation = { entryId: string; runId: string | null; startIndex: number; count: number };
export type FindResult = { complete?: boolean; snapshotSequence: number; totalMatches: number; activeIndex: number; match: FindMatch | null; navigation?: FindNavigation[] };
export type FindRequest = { threadId: string; query: string; skills: { name: string; displayName?: string }[]; start?: FindPosition; offset?: number };
const position = (match: FindPosition | null | undefined): FindPosition | undefined => match ? { entryId: match.entryId, occurrence: match.occurrence } : undefined;
const clampFindIndex = (index: number, count: number): number => count <= 0 || !Number.isFinite(index) || index < 0 ? 0 : Math.min(Math.trunc(index), count - 1);
export const stepFindIndex = (index: number, offset: number, count: number): number => count > 0 ? ((clampFindIndex(index, count) + offset) % count + count) % count : 0;
export const findCount = (result: FindResult | null): string => result ? result.complete === false ? '1/…' : `${result.totalMatches > 0 ? clampFindIndex(result.activeIndex, result.totalMatches) + 1 : 0}/${result.totalMatches}` : '';
/** Lowercase without changing UTF-16 offsets (including Greek final sigma). */
export function foldFindCase(text: string): string {
  const lower = text.toLowerCase().replace(/ς/g, 'σ');
  if (lower.length === text.length) return lower;
  return [...text].map(char => { const value = char.toLowerCase().replace(/ς/g, 'σ'); return value.length === char.length ? value : char; }).join('');
}
export type TextRange = { start: number; end: number };
export function literalFindRanges(text: string, query: string): TextRange[] {
  const needle = foldFindCase(query.trim()), haystack = foldFindCase(text), ranges: TextRange[] = [];
  if (!needle) return ranges;
  for (let at = haystack.indexOf(needle); at >= 0; at = haystack.indexOf(needle, at + needle.length)) ranges.push({ start: at, end: at + needle.length });
  return ranges;
}
/** Request identity belongs to one query, selected thread and connection incarnation. */
export class ThreadFind {
  open = false; query = ''; focus = 0; navigation = 0; result: FindResult | null = null; error = ''; pending = false;
  due = 0; serial = 0; accepted = 0; asked = 0; offset = 0; start: FindPosition | undefined; content = ''; refresh = false;
  constructor(readonly owner: string) {}
  show(position?: FindPosition): void {
    this.open = true; this.focus++;
    if (this.query.trim() && position) { this.start = position; this.offset = 0; this.schedule(0); }
  }
  close(): void { this.open = false; this.query = ''; this.result = null; this.pending = false; this.error = ''; this.due = 0; this.serial++; }
  edit(query: string, now: number): void {
    this.query = query.slice(0, 200); this.start = undefined; this.offset = 0; this.result = null; this.error = ''; this.serial++; this.pending = false;
    this.schedule(now + 100);
  }
  schedule(at: number): void { this.due = this.open && this.query.trim() ? Math.max(1, at) : 0; }
  changed(signature: string, now: number): void {
    if (signature === this.content) return;
    const first = !this.content; this.content = signature;
    if (first || !this.open || !this.query.trim()) return;
    this.refresh = true;
    if (this.pending) return;
    this.start = position(this.result?.match) ?? this.start; this.offset = 0; this.schedule(now + 300);
  }
  move(offset: number): void {
    if (!this.open || !this.query.trim() || this.result?.complete === false) return;
    const result = this.result;
    if (result && !this.pending && !this.refresh) {
      const index = stepFindIndex(result.activeIndex, offset, result.totalMatches);
      const entry = result.navigation?.find(entry => index >= entry.startIndex && index < entry.startIndex + entry.count);
      if (entry) { this.result = { ...result, activeIndex: index, match: { entryId: entry.entryId, runId: entry.runId, occurrence: index - entry.startIndex } }; this.navigation++; return; }
    }
    if (!this.pending && !this.due) { this.start = position(result?.match) ?? this.start; this.offset = offset; }
    else this.offset += offset;
    this.serial++; this.pending = false; this.schedule(0);
  }
  request(threadId: string, skills: FindRequest['skills'], now: number): { serial: number; payload: FindRequest } | null {
    if (!this.due || now < this.due || this.pending || !this.open || !this.query.trim()) return null;
    this.due = 0; this.pending = true; this.error = ''; this.refresh = false; this.asked = ++this.serial;
    return { serial: this.asked, payload: { threadId, query: this.query.trim(), skills, ...(this.start ? { start: this.start } : {}), ...(this.offset ? { offset: this.offset } : {}) } };
  }
  accept(serial: number, result: FindResult): boolean {
    if (!this.open || !this.pending || serial !== this.serial) return false;
    const previous = this.result?.match;
    if (this.accepted !== serial || previous?.entryId !== result.match?.entryId || previous?.occurrence !== result.match?.occurrence) this.navigation++;
    this.accepted = serial; this.result = result; this.error = '';
    if (result.complete !== false) { this.pending = false; this.offset = 0; if (this.refresh) { this.start = position(result.match) ?? this.start; this.schedule(0); } }
    return true;
  }
  fail(serial: number, error: string): void { if (this.open && serial === this.serial) { this.pending = false; this.error = error; this.due = 0; } }
}
