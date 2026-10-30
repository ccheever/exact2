// @exact/in-app-browser on the web (LLP 1133.024 D11, D12): the web has no
// in-app browser, so the page opens in a new tab or window as the browser
// chooses, which is what a web app's outside link does. With `noopener`,
// `window.open` returns null whether or not it opened, so it answers
// `opened`; the browser's popup policy is the only check.
//
// Under the agent nothing opens: the `browser` ticket's tap is the answer.

/** An absolute http or https URL, or null (D11). */
export function webURL(text) {
  try {
    const url = new URL(String(text ?? ''));
    return url.protocol === 'http:' || url.protocol === 'https:' ? url : null;
  } catch { return null; }
}

export function implementation({ agent, log }) {
  const say = line => log?.(`in-app-browser: ${line}`);
  return {
    async inAppBrowserStatus() { return { available: true, overApp: false }; },
    async openBrowser({ url } = {}, call) {
      const page = webURL(url);
      if (!page) return { error: 'TypeError', message: 'openBrowser takes an absolute http or https URL; open any other with openURL' };
      if (agent) {
        const action = call?.preferred?.tap ?? 'opened';
        say(`${action} ${page.origin}`);
        return { action };
      }
      window.open(page.href, '_blank', 'noopener,noreferrer');
      say(`opened ${page.origin}`);
      return { action: 'opened' };
    },
  };
}
