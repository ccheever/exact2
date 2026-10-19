// The origins the tool frames (LLP 1122 D3): a dev loop's page, by hello's
// `page`.

// A dev loop's page the canvas may frame: a loopback origin, no path
// (hello's `page`), never the tool's own port (`own`: a frame of its own
// origin could lift its sandbox); anything else is none. Its URL as the dev
// loop prints it (127.0.0.1, an address its gate admits), for "Open in a
// tab" and --via, and the canvas's: localhost on that port (the dev loop
// answers on both loopback addresses), another site than the tool's, so a
// browser with site isolation runs the page in its own process and a busy
// one cannot stall the tool.
export const pageOf = (page: unknown, own: number) => {
  const m = typeof page === 'string' ? /^http:\/\/(127\.0\.0\.1|localhost|\[::1\]):(\d{1,5})$/.exec(page) : null;
  const port = m ? Number(m[2]) : 0;
  if (!m || port < 1 || port > 65535 || port === own) return { page: '', frame: '' };
  return { page: `http://127.0.0.1:${port}`, frame: `http://localhost:${port}` };
};
