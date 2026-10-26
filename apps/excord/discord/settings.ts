// The account's settings arrive as protobuf, base64 (`user_settings_proto` in READY,
// `USER_SETTINGS_PROTO_UPDATE` after it), when IDENTIFY asks for USER_SETTINGS_PROTO. Only the
// server list is read: PreloadedUserSettings field 14, GuildFolders { repeated GuildFolder
// folders = 1; repeated fixed64 guild_positions = 2 }, GuildFolder { repeated fixed64 guild_ids = 1;
// Int64Value id = 2; StringValue name = 3; UInt64Value color = 4 }
// (discord-userdoccers/discord-protos, discord_users/v1/PreloadedUserSettings.proto). `folders`
// lists every guild in a folder, a guild in none as a folder of one; `guild_positions` is its
// deprecated predecessor, read only when there are no folders.

type Field = { field: number; wire: number; value: bigint | Uint8Array };

function base64(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

// https://protobuf.dev/programming-guides/encoding/: wire type 0 varint, 1 fixed64, 2 length-delimited, 5 fixed32.
function fields(bytes: Uint8Array): Field[] {
  const out: Field[] = [];
  let at = 0;
  const varint = (): bigint => {
    let value = 0n, shift = 0n;
    for (;;) {
      if (at >= bytes.length) throw new Error('A varint runs past the end');
      const byte = bytes[at++];
      value |= BigInt(byte & 0x7f) << shift;
      if (!(byte & 0x80)) return value;
      shift += 7n;
    }
  };
  const take = (n: number): Uint8Array => {
    if (at + n > bytes.length) throw new Error('A field runs past the end');
    return bytes.subarray(at, at += n);
  };
  while (at < bytes.length) {
    const key = Number(varint());
    const field = key >>> 3, wire = key & 7;
    if (wire === 0) out.push({ field, wire, value: varint() });
    else if (wire === 1) out.push({ field, wire, value: take(8) });
    else if (wire === 2) out.push({ field, wire, value: take(Number(varint())) });
    else if (wire === 5) out.push({ field, wire, value: take(4) });
    else throw new Error(`Wire type ${wire} is not read`);
  }
  return out;
}

const fixed64 = (b: Uint8Array, at = 0): string => {
  let value = 0n;
  for (let i = 7; i >= 0; i--) value = (value << 8n) | BigInt(b[at + i]);
  return value.toString();
};

// A repeated fixed64 is packed (one length-delimited run) or one field per value.
function ids(all: Field[], field: number): string[] {
  const out: string[] = [];
  for (const f of all) {
    if (f.field !== field || !(f.value instanceof Uint8Array)) continue;
    if (f.wire === 1) out.push(fixed64(f.value));
    else if (f.wire === 2) for (let i = 0; i + 8 <= f.value.length; i += 8) out.push(fixed64(f.value, i));
  }
  return out;
}

export interface Folder { id: string; name: string; color: number | null; guildIds: string[] }

// A wrapper message (Int64Value, StringValue, UInt64Value) holds its value in field 1; a wrapper that is
// there with no field 1 holds the default, zero or "" (proto3).
const wrapped = (all: Field[], field: number, empty: bigint | Uint8Array): Field['value'] | undefined => {
  const w = all.filter(f => f.field === field && f.wire === 2).at(-1);
  return w ? fields(w.value as Uint8Array).find(f => f.field === 1)?.value ?? empty : undefined;
};

/** The account's server list as folders, in its order: a folder with no id holds one guild that sits
 * in no folder. Null when these settings do not carry the list (a partial update about something else). */
export function guildFolders(proto: string): Folder[] | null {
  const folders = fields(base64(proto)).filter(f => f.field === 14 && f.wire === 2).at(-1);
  if (!folders) return null;
  const inner = fields(folders.value as Uint8Array);
  const out = inner.filter(f => f.field === 1 && f.wire === 2).map(f => {
    const all = fields(f.value as Uint8Array);
    const id = wrapped(all, 2, 0n), name = wrapped(all, 3, new Uint8Array()), color = wrapped(all, 4, 0n);
    return { id: typeof id === 'bigint' ? id.toString() : '', name: name instanceof Uint8Array ? new TextDecoder().decode(name) : '',
      color: typeof color === 'bigint' ? Number(color) : null, guildIds: ids(all, 1) };
  });
  return out.length ? out : ids(inner, 2).map(id => ({ id: '', name: '', color: null, guildIds: [id] }));
}

/** The guild ids in the order the account's server list shows them, or null when these settings
 * do not carry the list. */
export function guildOrder(proto: string): string[] | null {
  return guildFolders(proto)?.flatMap(f => f.guildIds) ?? null;
}
