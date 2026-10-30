// @exact/compose on the web (LLP 1133.024 D15): a `mailto:` URL composed from
// the fields, percent-encoded as RFC 6068 §2 says, which opens the person's mail
// app and answers `opened`. The web carries no attachment, and offers no
// message: a desktop browser has no `sms:` handler and fails silently, and the
// page cannot tell a phone's browser from it.
//
// Under the agent nothing opens: the `compose` ticket's tap is the answer. The
// journal names the composer and counts, never an address, the subject or the body.

const STATUS = { available: true, email: true, emailAttachments: false, emailCopiesWithAttachments: false, message: false, messageSubject: false, messageAttachments: false };

const list = (v) => (Array.isArray(v) ? v : []).map(String).filter(Boolean);
/** RFC 6068 §2: each part percent-encoded as UTF-8, a line break as `%0D%0A`. */
const enc = (s) => encodeURIComponent(s).replace(/%0A/g, '%0D%0A').replace(/%0D%0D%0A/g, '%0D%0A');

/** The `mailto:` URL for a mail's fields: the addresses in the path, `cc`, `bcc`, `subject` and `body` as headers. */
export function mailto({ to, cc, bcc, subject, body } = {}) {
  const headers = [['cc', list(cc).join(',')], ['bcc', list(bcc).join(',')], ['subject', subject ?? ''], ['body', body ?? '']]
    .filter(([, v]) => v).map(([k, v]) => `${k}=${enc(v)}`);
  return `mailto:${list(to).map((a) => enc(a).replace(/%40/g, '@')).join(',')}${headers.length ? `?${headers.join('&')}` : ''}`;
}

export function implementation({ agent, log }) {
  const say = (line) => log?.(`compose: ${line}`);
  const counted = (o) => `${list(o.to).length + list(o.cc).length + list(o.bcc).length} recipients, ${list(o.attachments).length} attachments`;
  return {
    async composeStatus() { return STATUS; },
    async composeEmail(o = {}, call) {
      if (list(o.attachments).length) return { error: 'NotSupportedError', message: 'the web carries no attachment in a mail; check composeStatus().emailAttachments' };
      if (agent) {
        const action = call?.preferred?.tap ?? 'opened';
        say(`email ${action}, ${counted(o)}`);
        return { action };
      }
      location.href = mailto(o);
      say(`email opened, ${counted(o)}`);
      return { action: 'opened' };
    },
    async composeMessage() {
      return { error: 'NotSupportedError', message: 'the web offers no message composer; check composeStatus().message' };
    },
  };
}
