/** Observe fixture readiness from before navigation, including failures that prevent its script from finishing.
 * A caller's own deadline must exceed `timeoutMs` by browser setup and CDP cleanup, each CDP call taking
 * up to 15 s: the tests here allow 60 s against the default 30 s. */
export async function readyPage(cdp, sessionId, url, { timeoutMs = 30000 } = {}) {
  const call = (method, params) => cdp.send(method, params, sessionId);
  const binding = '__exactTestReady', requests = new Map();
  let script, timer, listener, closed, failed = false, detached = false;
  try {
    await call('Page.enable');
    await call('Runtime.enable');
    await call('Network.enable');
    await call('Runtime.addBinding', { name: binding });
    ({ identifier: script } = await call('Page.addScriptToEvaluateOnNewDocument', { source: `
      if (window === window.top) {
        let ready;
        Object.defineProperty(window, 'ready', {
          configurable: true,
          get: () => ready,
          set(value) { ready = value; if (value === true) window.${binding}('ready'); }
        });
      }
    ` }));
    const outcome = new Promise((resolve, reject) => {
      const fail = reason => reject(new Error(`${url}: ${reason}`));
      timer = setTimeout(() => fail(`window.ready was not set within ${timeoutMs} ms`), timeoutMs);
      closed = fail;
      cdp.closeListeners.add(closed);
      if (cdp.closed) fail(cdp.closed);
      listener = ({ sessionId: session, method, params }) => {
        if (method === 'Target.detachedFromTarget' && params.sessionId === sessionId) {
          detached = true;
          fail(`page target detached (session ${sessionId})`);
          return;
        }
        if (session !== sessionId) return;
        if (method === 'Runtime.bindingCalled' && params.name === binding) resolve();
        else if (method === 'Runtime.exceptionThrown') {
          const e = params.exceptionDetails;
          fail(e.exception?.description ?? `${e.text} at ${e.url}:${e.lineNumber + 1}`);
        } else if (method === 'Network.requestWillBeSent' && ['Document', 'Script'].includes(params.type)) {
          requests.set(params.requestId, params.request.url);
        } else if (method === 'Network.responseReceived' && ['Document', 'Script'].includes(params.type) && params.response.status >= 400) {
          fail(`HTTP ${params.response.status} ${params.response.url}`);
        } else if (method === 'Network.loadingFailed' && requests.has(params.requestId)) {
          fail(`${params.errorText || 'request failed'}${params.blockedReason ? ` (blocked: ${params.blockedReason})` : ''} ${requests.get(params.requestId)}`);
        }
      };
      cdp.listeners.push(listener);
    });
    await Promise.all([outcome, call('Page.navigate', { url }).then(result => {
      if (result.errorText) throw new Error(`${url}: ${result.errorText}`);
    })]);
  } catch (error) {
    failed = true;
    throw error;
  } finally {
    clearTimeout(timer);
    if (listener) cdp.listeners.splice(cdp.listeners.indexOf(listener), 1);
    if (closed) cdp.closeListeners.delete(closed);
    if (!cdp.closed && !detached) {
      try {
        await Promise.all([
          ...(script ? [call('Page.removeScriptToEvaluateOnNewDocument', { identifier: script })] : []),
          call('Runtime.removeBinding', { name: binding }),
        ]);
      } catch (error) {
        if (!failed) throw error;
      }
    }
  }
}
