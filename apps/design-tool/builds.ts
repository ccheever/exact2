// The Builds panel's rows (LLP 1122 D4): each running build as the server lists it, and the builds gone from that
// list that may be reloading (stage 15 D1.2).

/** The build as the panel reads it: the server's record, as app.ts keeps it. */
type Listed = { id: string; host: string; page: string; status: any; pin: any; pinRevision: number; unsupported: { what: string; detail: string | null } | null };

const pinText = (pin: any) => pin ? [pin.scheme, pin.contrast && `contrast ${pin.contrast}`, ...Object.entries(pin.mode ?? {}).map(([a, v]) => `${a}=${v}`), pin.rootFontSize && `${pin.rootFontSize}px`].filter(Boolean).join(', ') : '';

/** A build as the panel lists it: its effective scheme, contrast, mode and root font size from its status (an app's
 * own setScheme beats a pin, D3), its pin, whether its status has caught up with the last pin frame it was sent (a
 * pin, or a hand-back; none before it reports), and the last command it could not apply, as the server keeps it. */
export const buildRow = (b: Listed) => {
  const st = b.status ?? {};
  const mode = Object.entries((st.mode ?? {}) as Record<string, string>).map(([a, v]) => `${a}=${v}`).join(' ');
  return {
    id: b.id, host: b.host, gallery: b.page === 'gallery', reported: b.status != null, scheme: String(st.scheme ?? ''), contrast: String(st.contrast ?? ''), mode, rootFontSize: Number(st.rootFontSize ?? 0),
    pin: pinText(b.pin), caughtUp: Number.isSafeInteger(st.revision) && st.revision >= b.pinRevision,
    behind: Number.isSafeInteger(st.revision) && st.revision < b.pinRevision,
    unsupported: b.unsupported ? `${b.unsupported.what}${b.unsupported.detail ? `: ${b.unsupported.detail}` : ''}` : '',
  };
};

/** The builds gone from the server's list, by id, each numbered by its going: a build that was connected and goes is
 * one reloading until it registers again under its id (stable across its reloads), for at most the window the root's
 * task bounds (app.contract `reloadWindow`), after which it is no build. A new run of the server forgets them. The
 * newest 16. */
export class Gone {
  private gone = new Map<string, { id: string; host: string; gen: number }>();
  newest = 0;
  /** The server's list, `ids`, after `was`: each build of `was` it no longer lists has gone, each it lists is back. */
  listed(was: Map<string, { host: string }>, ids: string[]) {
    const now = new Set(ids);
    for (const [id, b] of was) if (!now.has(id)) { this.gone.delete(id); this.gone.set(id, { id, host: b.host, gen: ++this.newest }); }
    for (const id of now) this.gone.delete(id);
    while (this.gone.size > 16) this.gone.delete(this.gone.keys().next().value!);
  }
  back(id: string) { this.gone.delete(id); }
  clear() { this.gone.clear(); }
  rows() { return [...this.gone.values()]; }
}
