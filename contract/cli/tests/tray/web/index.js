// @fixture/tray on the web: a module outside exact2's tree. A put keeps
// what each item carried, as the seam handed it over (a string, bytes as
// base64, a file the host read under the source's grant), and reports it as
// `$state`, which the host keeps for `state.extensions`. It is its own
// substitute, so under the agent it runs too, saying so. A put whose
// sequence number is older than one already taken is superseded.

export function implementation({ agent }) {
  let newest = 0;
  const describe = async (v) => {
    if (typeof v === 'string') return `text ${v}`;
    if (v?.$bytes !== undefined) return `bytes ${atob(v.$bytes).length}`;
    if (v?.$file) return `file ${v.$file.size}`;
    return `other ${JSON.stringify(v)}`;
  };
  const take = async (seq, items) => {
    if (seq === 1) newest = 0;
    if (seq <= newest) return { error: 'AbortError', message: `put ${seq}: a later put superseded it` };
    newest = seq;
    const last = await Promise.all(items.map(async item => Object.fromEntries(await Promise.all(Object.entries(item).map(async ([k, v]) => [k, await describe(v)])))));
    return { $state: { last, substituting: agent } };
  };
  return {
    put: ({ items, $sequence }) => take($sequence?.seq ?? 0, items),
    putText: ({ text, $sequence }) => take($sequence?.seq ?? 0, [{ 'text/plain': text }]),
  };
}
