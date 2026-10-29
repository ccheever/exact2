// The JS target's open streams (LLP 1016.000): a ticket rt.js `send` starts as a stream is open from its
// start until its end. One whose resource or mutation let its ticket go (new arguments, `refresh`, a failure,
// its region ended) is closed once the commit that let it go stands, as the runner's forget path (D2).
import { inflight, say } from "./rt.js";
import { counted, longPollEnded } from "./held.js";
import { cutUpload } from "./uploads.js";

export const Open = new Set();
export function closeLetGo() {
  for (const t of Open) if (!t.held() || t.r?.gone) {
    Open.delete(t); longPollEnded(t); t.closed = true; t.ctl.abort();
    cutUpload(t); // an upload is cut off (D10)
    if (t.waiting) { t.waiting = false; counted(t, inflight); }
    say(`close stream ${t.id}: its ticket was let go`);
  }
}
