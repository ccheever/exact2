// `user_settings_proto` as Discord sends it (`discord/settings.ts` reads it): PreloadedUserSettings
// with its guild folders, after an appearance field the reader skips. Each folder is a list of guild
// ids; a named one carries a StringValue name as a real folder does.

const varint = (n: bigint): number[] => {
  const out: number[] = [];
  do {
    let byte = Number(n & 0x7fn);
    n >>= 7n;
    if (n) byte |= 0x80;
    out.push(byte);
  } while (n);
  return out;
};
const fixed64 = (id: string): number[] => {
  let v = BigInt(id);
  return Array.from({ length: 8 }, () => { const byte = Number(v & 0xffn); v >>= 8n; return byte; });
};
const bytes = (field: number, value: number[]): number[] => [...varint(BigInt(field << 3 | 2)), ...varint(BigInt(value.length)), ...value];

export function settingsProto(folders: { ids: string[]; name?: string }[]): string {
  const folder = (f: { ids: string[]; name?: string }) => bytes(1, [...bytes(1, f.ids.flatMap(fixed64)),
    ...(f.name ? bytes(3, bytes(1, [...new TextEncoder().encode(f.name)])) : [])]);
  const appearance = bytes(13, [...varint(1n << 3n), ...varint(2n)]);
  return btoa(String.fromCharCode(...appearance, ...bytes(14, folders.flatMap(folder))));
}
