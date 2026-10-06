// Local notifications on the JS target (rules/DEFERRED.md, 2026-10-04),
// bundled only for a plan that runs `showNotification` or
// `closeNotification` (linked by use, the standing rule in
// queue/standing-rule-a-new-optional-web-capability-links-by-use.md): the
// runner's rule (runner/src/notify.rs) — the data and `device.notifications`
// checked, listed under the agent (a tag replacing its older one; agent.js
// reads `exact.notices` for `state.notifications`), else posted by the web
// host's notify-glue.js, fetched on first use. The outcome is a journal line.
import { journal, clock, Hosts, data } from './rt.js';

const say = line => journal.push(`t=${clock.now} ${line}`);
const notices = (globalThis.exact ??= {}).notices = [];
const untag = tag => { for (let i = notices.length; i--;) if (notices[i].tag === tag) notices.splice(i, 1); };
const glue = () => import('./notify-glue.js').then(() => globalThis.exact.notifications);
Hosts.showNotification = (title, body, tag, showTrigger) => {
  const refuse = why => say(`showNotification: refused: ${why}`);
  if (typeof title !== 'string' || !title) return refuse('needs a title=');
  if (showTrigger != null && !(Number.isFinite(showTrigger) && showTrigger >= 0)) return refuse('showTrigger= is a time in epoch milliseconds');
  if (!/^\s*device\.notifications\s/m.test(data.grants ?? '')) return refuse('the grants name no device.notifications');
  const notice = { title, body: body ?? null, tag: tag ?? null, showTrigger: showTrigger ?? null };
  if (!clock.agent) return glue().then(n => n.show(notice, say));
  say(`showNotification: listed${tag == null ? '' : ` (${tag})`}`);
  if (tag != null) untag(tag);
  notices.push(notice);
};
Hosts.closeNotification = tag => clock.agent ? untag(tag) : glue().then(n => n.close(tag));
