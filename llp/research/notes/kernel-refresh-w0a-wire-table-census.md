RFC 0491 W0-A draft artifact — hand-verified inventory/draft, provenance commands inline; generated authorities under tests/protocol/exwf/ own their boundaries and this pack never restates them. Registered in exact-verify.json as `kernel-refresh-w0a-pack` (RFC 0491 W0-B).

# Wire-table census

This is the WS-B hand census of hand-ordered style vocabularies and the seven currently live extension/sidecar opcodes. `wire-table-census.json` is the machine-readable form.

## Style mask

`StyleMask` fills all **64** bits of its low `u64`; `StyleMask128` adds **24** populated bits in a second `u64`. The current style vocabulary therefore has **88 populated bits** in a **128-bit storage mask**. Provenance:

```sh
sed -n '1091,1154p' kernel/src/style.rs | rg 'pub const [A-Z0-9_]+: u64 = 1 <<' | wc -l
sed -n '1179,1204p' kernel/src/style.rs | rg 'pub const [A-Z0-9_]+: u64 = 1 <<' | wc -l
sed -n '1089,1204p' kernel/src/style.rs | rg 'pub const [A-Z0-9_]+: u64 = 1 <<' | wc -l
sed -n '1165,1173p' kernel/src/style.rs
```

## TypeScript ordered vocabularies

There are **15** literal `*_VALUES` tables in `packages/exact-core/src/protocol/buffer-writer.ts`; their entry counts are:

| Table | Entries |
| --- | ---: |
| `FLEX_DIRECTION_VALUES` | 4 |
| `FLEX_WRAP_VALUES` | 3 |
| `JUSTIFY_CONTENT_VALUES` | 6 |
| `ALIGN_ITEMS_VALUES` | 5 |
| `ALIGN_SELF_VALUES` | 6 |
| `POSITION_TYPE_VALUES` | 2 |
| `OVERFLOW_VALUES` | 3 |
| `TEXT_ALIGN_VALUES` | 4 |
| `ELLIPSIZE_MODE_VALUES` | 4 |
| `DISPLAY_VALUES` | 3 |
| `GRID_AUTO_FLOW_VALUES` | 4 |
| `DIRECTION_VALUES` | 2 |
| `WRITING_MODE_VALUES` | 3 |
| `FONT_STYLE_VALUES` | 2 |
| `TEXT_DECORATION_LINE_VALUES` | 4 |

Provenance:

```sh
rg '^const [A-Z_]+_VALUES = \[' packages/exact-core/src/protocol/buffer-writer.ts | wc -l
awk '/const [A-Z_]+_VALUES = \[/ { name=$2; count=0; active=1 } active { line=$0; while (match(line, /\047[^\047]*\047/)) { count++; line=substr(line,RSTART+RLENGTH) } if ($0 ~ /as const;/) { print name,count; active=0 } }' packages/exact-core/src/protocol/buffer-writer.ts
```

Backdrop material is the additional authority: `packages/exact-core/src/style/backdrop-material.ts:11-18` has **6 authoring values**, while the inventory wire enum includes `None` and therefore has **7 wire entries**. Reproduce with:

```sh
sed -n '11,18p' packages/exact-core/src/style/backdrop-material.ts
jq '.styleEnums.BackdropMaterial | length' tests/protocol/protocol-inventory.json
```

## SetStyle implementations

- Rust decoder: `kernel/src/protocol/style_decoder.rs:22-475`.
- TypeScript encoder: `packages/exact-core/src/protocol/buffer-writer.ts:834-1176`.
- Web host decoder: `packages/exact-native-web/src/protocol.ts:159-334`, dispatched at line 418. `protocol-dom-host.ts` applies the decoded result rather than decoding the style payload itself.
- Kotlin/Android decoder: `android/ExactApp/app/src/main/java/com/exact/androidhost/surface/ExactAndroidRenderer.kt:2580-2754`, dispatched at line 539.

Locate them with:

```sh
rg -n 'decode_style_payload_into|setStyle\(|decodeStylePayload|OP_SET_STYLE|SetStyle' kernel/src/protocol/style_decoder.rs packages/exact-core/src/protocol/buffer-writer.ts packages/exact-native-web/src android/ExactApp/app/src/main/java/com/exact/androidhost/surface/ExactAndroidRenderer.kt
```

At census time the Rust decoder explicitly discarded its final cursor with `let _ = offset;` at `kernel/src/protocol/style_decoder.rs:472`, so a payload could contain unread trailing bytes after the selected fields. **Resolved by RFC 0491 W0-B:** trailing style bytes now fail loudly with a `ProtocolError`. Provenance (post-W0-B; the historical discard is visible in the file's git history):

```sh
rg -n 'style payload trailing bytes' kernel/src/protocol/style_decoder.rs
```

## Live sidecar opcodes

| ID | Family | Opcode inventory | Recovery authority |
| ---: | --- | --- | --- |
| 35 | pager | `tests/protocol/protocol-inventory.json:181` | `tests/protocol/exwf/recovery-classes.json#/rows/0` |
| 36 | motion-snapshot | `tests/protocol/protocol-inventory.json:186` | `tests/protocol/exwf/recovery-classes.json#/rows/1` |
| 37 | motion-command | `tests/protocol/protocol-inventory.json:191` | `tests/protocol/exwf/recovery-classes.json#/rows/2` |
| 38 | list-model | `tests/protocol/protocol-inventory.json:196` | `tests/protocol/exwf/recovery-classes.json#/rows/3` |
| 39 | menu | `tests/protocol/protocol-inventory.json:201` | `tests/protocol/exwf/recovery-classes.json#/rows/4` |
| 40 | graphics-publication | `tests/protocol/protocol-inventory.json:206` | `tests/protocol/exwf/recovery-classes.json#/rows/5` |
| 41 | graphics-teardown | `tests/protocol/protocol-inventory.json:211` | `tests/protocol/exwf/recovery-classes.json#/rows/6` |

```sh
jq -r '.opcodes[] | select(.value >= 35 and .value <= 41) | "\(.value)\t\(.name)"' tests/protocol/protocol-inventory.json
jq -n --slurpfile inventory tests/protocol/protocol-inventory.json --slurpfile recovery tests/protocol/exwf/recovery-classes.json '$recovery[0].rows | map(. as $row | ($inventory[0].opcodes[] | select(.value == $row.familyDiscriminant)) as $op | {family:$row.family, opId:$row.familyDiscriminant, inventoryName:$op.name, recoveryName:$row.inventoryOpcodeName, agrees:($op.name == $row.inventoryOpcodeName)})'
```

Both authorities agree on all seven IDs and opcode names. The table above joins to the generated recovery rows without copying their class assignments or dispositions.
