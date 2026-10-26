// `user_settings_proto` as Discord sends it (`discord/settings.ts` reads it): PreloadedUserSettings
// with its guild folders, after an appearance field the reader skips. Each folder is a list of guild
// ids; a real folder also carries its id (Int64Value), name (StringValue) and colour (UInt64Value).

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
// A wrapper message's one value, a varint in its field 1, which proto3 leaves out when it is zero.
const wrapped = (field: number, n: number): number[] => bytes(field, n ? [...varint(1n << 3n), ...varint(BigInt(n))] : []);

export interface FolderSpec { ids: string[]; id?: number; name?: string; color?: number }
export function settingsProto(folders: FolderSpec[]): string {
  const folder = (f: FolderSpec) => bytes(1, [...bytes(1, f.ids.flatMap(fixed64)),
    ...(f.id !== undefined ? wrapped(2, f.id) : []),
    ...(f.name ? bytes(3, bytes(1, [...new TextEncoder().encode(f.name)])) : []),
    ...(f.color !== undefined ? wrapped(4, f.color) : [])]);
  const appearance = bytes(13, [...varint(1n << 3n), ...varint(2n)]);
  return btoa(String.fromCharCode(...appearance, ...bytes(14, folders.flatMap(folder))));
}
