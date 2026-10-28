// @exact/calls on the web (LLP 1133.033 D19). The call is the module's own state: every name is the declared keyed
// list's answer (`declared`). What the web adds is the browser's in-call controls: while a call is `connected`, Media
// Session's `hangup` and `togglemicrophone` actions (https://w3c.github.io/mediasession/#enumdef-mediasessionaction)
// take the call's `hangup` and `mute`/`unmute` choices, which raise `callhangup` and `callmute` as the person's, and
// `setMicrophoneActive` follows `muted`. Where the browser shows those controls is its own; the module opens no window.
// Chromium has both actions, WebKit `togglemicrophone` only, Firefox neither; a browser refuses `setActionHandler` for an
// action it lacks, and the call works without that control.
// The page runtime (host/web-js/modules.js) loads this with the page, under the agent too.

const NAMES = ['startCall', 'reportIncomingCall', 'answerCall', 'reportCallState', 'setCallHeld', 'setCallMuted', 'endCall'];

export function implementation({ declared, listed, mediaAction, log }) {
  let call = null; // the connected call the controls act on: the last listed
  let controls = false;
  const pick = choice => () => call && [call.ref, typeof choice === 'function' ? choice(call.fields) : choice];
  const follow = entries => {
    call = entries.filter(e => e.fields.state === 'connected').at(-1) ?? null;
    if (!!call !== controls) {
      controls = !!call;
      const hangup = mediaAction('hangup', controls ? pick('hangup') : null);
      const mic = mediaAction('togglemicrophone', controls ? pick(f => (f.muted ? 'unmute' : 'mute')) : null);
      if (controls && !hangup && !mic) log?.('calls: this browser has no in-call controls');
    }
    const session = typeof navigator === 'object' ? navigator.mediaSession : undefined;
    if (call && session?.setMicrophoneActive) {
      try { session.setMicrophoneActive(!call.fields.muted); } catch { /* not while no capture is live */ }
    }
  };
  listed('calls', follow);
  const out = {
    async calls() { return declared('calls', {}); },
    async callsStatus() { return declared('callsStatus', {}); },
  };
  for (const name of NAMES) out[name] = async args => declared(name, args);
  return out;
}
