# Any-type file input and image pixel readback/re-encode in data modules

**Status:** Open
**Systems:** Contract, pickers, grants
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/116

## Current scope

Propose only the any-type input exception, requiring a DEFERRED trade/waiver before code. showOpenFilePicker already reads arbitrary files. Image codecs/averaging/re-encode and Canvas readback remain deferred.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

## Summary
A data module can already get the bytes of a file of any type: `showOpenFilePicker` returns a `doc:/` handle, and `storage.fs.readFile` reads it, on the web and on macOS. That part is not filed. Still refused, by `rules/DEFERRED.md`: a file `input` with no `accept` (or any type outside images, video and the app's `file_handlers`), and reading pixels back (`getImageData`, `toDataURL`, `toBlob`, `OffscreenCanvas`). A data module also cannot decode an image and re-encode it at a given quality or size. This is a policy gap: a decision is needed before implementation. The ask: decide whether to admit an any-type `input type="file"` and a script-side image decode/read/encode path (not window pixels), then add them, macOS first.

## Why it matters
A coding-agent desktop app lets the person attach any file to a message: a hidden `<input type="file" multiple>` with no `accept`, plus drop and paste. Images are decoded and re-encoded before upload. HEIC becomes JPEG at quality 0.92. An image over the upload cap goes down a quality ladder (0.92 → 0.68), then is scaled to a 2048 px long edge. The app also tints an image chip with the alpha-weighted average colour of a 16×16 draw of the thumbnail. On exact2 the any-type picker works only through `showOpenFilePicker`, a different control from the web code being ported. The image steps need native code: a second download of the image to average its pixels, and an AppKit pipeline for re-encoding. The ported TypeScript then has no tests that reach that logic, and its behavior can drift from the web build.

## Current behavior (exact2 4c893fef6)
- Works: `showOpenFilePicker("opened")`, answered with a 3000-byte `archive.xyz`, gives `Text [handle] "handle=doc:/1/archive.xyz"`. The data module's `readFile` then answers `Text [bytes] "read archive.xyz: 3000 bytes"`. This was seen on the web and on macOS (agent drives; the agent answers the picker). Source: the macOS panel filters only by the manifest's `file_handlers` (`DocumentPickers.swift:122-124,197-198`), and the web passes only `{ multiple }`.
- Refused: `input type="file" multiple=true` with no `accept` gives `lower-picker-accept`: "needs a literal `accept` … rules/DEFERRED.md admits an image/video picker, widened to the types the app's `file_handlers` declare: "Still no picker for any file, and no camera."" `accept="*/*"` and `accept="application/octet-stream"` give `bake-picker-accept` with the same rule.
- Refused: a Canvas 2D surface that calls `ctx.getImageData(0, 0, 16, 16)` logs `canvas 1 (accent) draw threw: TypeError: n.getImageData is not a function` (web). On macOS: `draw threw: TypeError: undefined is not a function`, and `state.canvas[0].error` is the same.
- Policy: `rules/DEFERRED.md:640-642`: "Still refused: … readback (`getImageData`, `toDataURL`, `toBlob`), `ctx.filter`, and an app-visible `OffscreenCanvas`." LLP 1056: "Readback is refused … The agent reads pixels with `screenshot`."
- No image codec is callable from a data module. From reading `Picker.swift:1-9`: the image `input` turns HEIC into JPEG only when `accept` names images, with no quality or size choice.

## Expected behavior
- Web standard: `<input type="file" multiple>` with no `accept` offers any file. `File.arrayBuffer()` gives the bytes. `createImageBitmap(blob)` decodes, `OffscreenCanvas.getContext('2d').getImageData()` reads pixels, and `OffscreenCanvas.convertToBlob({ type: 'image/jpeg', quality })` re-encodes.
- Proposal A (needs a DEFERRED waiver): `input type="file"` with no `accept` (or `*/*`) is admitted. It delivers `list<Picked>` with `app:/tmp/picked/` copies, as the image picker does: NSOpenPanel with no type filter on macOS, a document picker on iOS, a plain file input on the web.
- Proposal B (needs a DEFERRED waiver): a data-module image API on file bytes, not window pixels. For example `storage.image.decode(path, { maxEdge })` returns `{ width, height, rgba: Uint8Array }`, and `storage.image.encode(rgba, { type: 'image/jpeg', quality })` returns bytes. Or the web names `createImageBitmap` and `OffscreenCanvas` (`getImageData`, `convertToBlob`), detached from any on-screen canvas. Apple: ImageIO and Core Graphics. Web: the browser's. Linux: the `image` crate or refused.
- Option C: no change. Document `showOpenFilePicker` as the any-file path, and keep image work in native modules.

## Reproduction
Minimal app (`app.ts`: `grants = 'fs.read doc:/'`; `readBytes(path)` answers `{ name, size: (await storage.fs.readFile(path)).byteLength }`; surface `accent` fills 64×64 then calls `ctx.getImageData(0, 0, 16, 16)`):
```contract
shape Read
  name: string
  size: number

component X30AnyFilePixels
  mutation read as shape Read
  state handle = ""
  action open
    showOpenFilePicker("opened")
  action opened(value: string)
    handle = value
    send read = readBytes(value)
  view
    main testId="root" padding=16 display="flex" flex-direction="column" gap=8
      input id="opened" display="none" change=opened
      button "Open any file" press=open testId="open"
      text `handle=${handle}` testId="handle"
      match read
        case some(r)
          text `read ${r.name}: ${r.size} bytes` testId="bytes"
        case none
          text "nothing read" testId="bytes"
      canvas surface=accent() width=64 height=64 testId="accent"
```
| Step | Command / action | Host | Actual | Expected |
|---|---|---|---|---|
| 1 | `bun scripts/exact.mjs new target/repro/x30-any-file-pixels`, then the app above, `web-build`, `mac` | — | builds | builds |
| 2 | `agent web "clock data" "tap open" "type @opened …/archive.xyz" "clock data" tree logs` | Chrome | `read archive.xyz: 3000 bytes` | same (supported) |
| 3 | The same with `agent macos` | macOS 26.6 | `read archive.xyz: 3000 bytes` | same (supported) |
| 4 | Add `input type="file" multiple=true` with no `accept`, then `contract build --json` | any | `lower-picker-accept` | accepted; any file offered |
| 5 | Steps 2–3 logs for the `accent` surface | Chrome, macOS | `draw threw: TypeError: … getImageData …` | 16×16 RGBA returned (proposal B) |

### Evidence
The files are attached at the end of this issue, in folded sections.
- `evidence/X30/repro.log`: revision, both agent drives (tree, logs, `state.canvas`), the three `accept` refusals, the DEFERRED and LLP 1056 text, the `NativeModule` type, and the source lines cited.
- `evidence/X30/app/`: the minimal app's source files.

## Acceptance criteria
- Option A: a minimal app's `input type="file" multiple` with no `accept` takes `type @attach <any file>` on web and macOS, and `change` delivers its `app:/tmp/picked/` path and size. In an attended macOS run the panel lets the person choose any file.
- Option B: a data module decodes a PNG to RGBA and returns its alpha-weighted 16×16 average. A fully transparent PNG returns none. HEIC decodes on Apple. A PNG re-encodes as JPEG under a byte budget. The same numbers come from the web (Chrome) and macOS, within a stated tolerance.
- Option C: the decision is recorded in `rules/DEFERRED.md`, and the docs name `showOpenFilePicker` as the any-file path.

## Notes
- **A decision is needed before implementation.** Both refusals are DEFERRED rules ("Still no picker for any file"; readback and `OffscreenCanvas`).
- Not run: a real NSOpenPanel (the agent substitutes the answer), drop and paste of an arbitrary file, iOS.
- Not in this issue: making a resource re-read from inside data-module code. The generated `NativeModule` type is `available`, `call`, `later`, `watch`. In-flight state can be shown in Contract with `pending(mutation)`. No case was reproduced here where that is not enough.
- Related: LLP 1069.002 (media picker), LLP 1069.010 (document pickers), LLP 1056 (Canvas 2D).

---

<details><summary>Evidence: <code>repro.log</code></summary>

````text
exact2 4c893fef6 (origin/main 2026-10-05); macOS 26.6.2; Chrome via scripts/agent.mjs
test file: .exact/files/archive.xyz = 3000 random bytes (head -c 3000 /dev/urandom), an extension no app declares

## 1. Supported: showOpenFilePicker of any type + storage.fs.readFile (no file_handlers in app.json)
$ bun exact.mjs agent web "clock data" "tap open" "type @opened <abs>/archive.xyz" "clock data" tree logs
{"clock":0,"settled":true}
{"at":[210,25],"tapped":4,"target":"open","delivery":"platform","carrier":"web","mode":"agent","clock":0,"epoch":2,"incarnation":1}
{"ticket":1,"capability":"open-file","answered":"value","delivery":"substituted","clock":0,"epoch":3,"incarnation":1}
{"clock":0,"settled":true}
epoch 4 · incarnation 1 · clock 0 ms · 7 nodes
View#2 [root]
  TextInput#3 value="" (change)
  Pressable#4 [open] [focused] (press)
    Text#5 "Open any file"
  Text#6 [handle] "handle=doc:/1/archive.xyz"
  Text#8 [bytes] "read archive.xyz: 3000 bytes"
  Canvas#1 [accent]
t=0 boot: 8 nodes, epoch 1
t=0 canvas 1 (accent) draw threw: TypeError: n.getImageData is not a function
t=0 command showOpenFilePicker
t=0 device open-file 1 held (agent)
t=0 device open-file 1 answered: 1 item
t=0 showOpenFilePicker: chosen
t=0 reply 2; wall 4 ms
$ bun exact.mjs agent macos "clock data" "tap open" "type @opened <abs>/archive.xyz" "clock data" tree logs
{"answered":"value","capability":"open-file","delivery":"substituted","node":2,"ticket":1,"epoch":3,"incarnation":1,"clock":0}
{"epoch":3,"settled":true,"incarnation":1,"clock":0}
epoch 3 · incarnation 1 · clock 0 ms · 7 nodes
View#1 [root]
  TextInput#2 (change)
  Pressable#3 [open] [focused] (press)
    Text#4 "Open any file"
  Text#5 [handle] "handle=doc:/1/archive.xyz"
  Text#8 [bytes] "read archive.xyz: 3000 bytes"
  Canvas#7 [accent]
t=0 boot: 7 nodes, epoch 1
t=0 canvas 7 (accent) draw threw: TypeError: undefined is not a function
t=0 command showOpenFilePicker("opened")
t=0 press view 3 (open) → epoch 1 (+0 −0 ~0)
t=0 device open-file 1 held (agent)
t=0 device open-file 1 answered: 1 item
t=0 showOpenFilePicker: chosen
t=0 continuation 2 (read): executor token 1
t=0 change view 2 (opened) → epoch 2 (+0 −0 ~1)
t=0 fulfil 2 (read) [HTTP 204, 0 bytes; wall 0 ms] → epoch 3 (+1 −1 ~1)
Source (not run with a real panel): host/apple/Sources/ExactKit/DocumentPickers.swift:122-124,197-198 — the NSOpenPanel filters only by the manifest's file_handlers; none declared means no filter. host/web/documents-glue.js:228 passes only { multiple } to showOpenFilePicker.
    /// The manifest's `file_handlers`, as the bake wrote them into
    /// `CFBundleDocumentTypes`: the only types a picker offers (D2).
    static var declaredTypes: [UTType] { documentTypes.filter { $0 != .folder } }
            let types = Picker.declaredTypes
            if open.canChooseFiles && !types.isEmpty { open.allowedContentTypes = types }

## 2. Refused: input type=file with no accept, or accept=*/* or application/octet-stream
$ bun exact.mjs contract build .exact/probe/file.contract --json   # adds: input type="file" multiple=true [accept=…] id="attach"
== input type=file multiple
[{"col":13,"end_col":17,"file":".exact/probe/file.contract","id":"lower-picker-accept","line":17,"message":"`input type=\"file\"` needs a literal `accept` (`accept=\"image/*\"`): rules/DEFERRED.md admits an image/video picker, widened to the types the app's `file_handlers` declare: \"Still no picker for any file, and no camera.\"","related":[]}]

== input type=file multiple accept="*/*"
[{"col":39,"end_col":45,"file":".exact/probe/file.contract","id":"bake-picker-accept","line":17,"message":"`accept` names `*/*`, which is neither an image or video type nor one the app opens (the app's `file_handlers` declares no types): rules/DEFERRED.md admits an image/video picker, widened to the types the app's `file_handlers` declare: \"Still no picker for any file, and no camera.\"","related":[]}]

== input type=file multiple accept="application/octet-stream"
[{"col":39,"end_col":45,"file":".exact/probe/file.contract","id":"bake-picker-accept","line":17,"message":"`accept` names `application/octet-stream`, which is neither an image or video type nor one the app opens (the app's `file_handlers` declares no types): rules/DEFERRED.md admits an image/video picker, widened to the types the app's `file_handlers` declare: \"Still no picker for any file, and no camera.\"","related":[]}]


## 3. Refused: pixel readback in a Canvas 2D surface (draw calls ctx.getImageData(0,0,16,16))
web: logs
t=0 canvas 1 (accent) draw threw: TypeError: n.getImageData is not a function
macOS: logs
t=0 canvas 7 (accent) draw threw: TypeError: undefined is not a function
macOS: state.canvas
[{"view": 7, "surface": "accent", "context": "2d", "artifact": "data", "lifetime": 1, "generation": 1, "applied": 1, "requested": 1, "pending": false, "animating": false, "draws": 1, "ops": 2, "bytes": 88, "width": 128, "height": 128, "scale": 2, "stretch": false, "colorSpace": "srgb", "colorType": "unorm8", "lastDraw": 0, "brokenImages": [], "error": "TypeError: undefined is not a function", "refused": null}]

## 4. Policy text
rules/DEFERRED.md:640-642:
path exists after than before. Still refused: a drawing language in Contract
(SVG is the declarative one), readback (`getImageData`, `toDataURL`,
`toBlob`), `ctx.filter`, and an app-visible `OffscreenCanvas`.
llp/1056 (line 387):
387:**Readback is refused:** `getImageData`, `toDataURL`, `toBlob`. The executor is not where the pixels are. A synchronous read would put the host's rasterizer in the executor's turn. The agent reads pixels with `screenshot`.

## 5. Data-module interface (generated app.contract.d.ts)
101:export interface NativeModule { readonly available: boolean; call(request: Record<string, unknown>): Record<string, unknown>; later(request: Record<string, unknown>): Promise<Record<string, unknown>>; watch(topic: string): void }

## 6. Image transcode on the picker (source, not run): host/apple/Sources/ExactKit/Picker.swift:1-9
// @ref LLP 1069.002 — the file picker on Apple. `showPicker(id)` presents
// PHPicker on iOS (no permission, no usage string) or a document picker for
// the manifest's non-media types, and a filtered NSOpenPanel sheet on macOS
// (D8). Native hosts don't enforce user activation (ruled, Q2). Each chosen
// file is copied into `app:/tmp/picked/` before `change` fires (D4), a HEIC
// photo becoming a JPEG when `accept` names images but not HEIC (D6,
// Safari's rule), with its pixel size and a video's duration (D3). Under
// the agent the request is held for `type @t <path>…` / `tap @t cancel`
// (D9), whose files take the same path.
````
</details>

<details><summary>Evidence: minimal app sources (<code>app/</code>)</summary>

`app.contract`

````contract
// X30: pick a file of any type and read its bytes; read pixels of a drawn image.
shape Read
  name: string
  size: number

component X30AnyFilePixels
  mutation read as shape Read
  state handle = ""
  action open
    showOpenFilePicker("opened")
  action opened(value: string)
    handle = value
    send read = readBytes(value)
  view
    main testId="root" padding=16 display="flex" flex-direction="column" gap=8
      input id="opened" display="none" change=opened
      button "Open any file" press=open testId="open"
      text `handle=${handle}` testId="handle"
      match read
        case some(r)
          text `read ${r.name}: ${r.size} bytes` testId="bytes"
        case none
          text "nothing read" testId="bytes"
      canvas surface=accent() width=64 height=64 testId="accent"
````

`app.json`

````json
{
  "$schema": "../../../scripts/app.schema.json",
  "name": "X30 Any File Pixels",
  "short_name": "X30 Any File Pixels",
  "id": "com.example.x30-any-file-pixels",
  "start_url": "/",
  "display": "standalone",
  "app": {
    "id": "com.example.x30-any-file-pixels",
    "name": "X30 Any File Pixels"
  },
  "host": {
    "ios": {
      "minimumOS": "17.0",
      "deviceFamily": [
        "iphone",
        "ipad"
      ]
    },
    "macos": {
      "minimumOS": "14.0",
      "window": {
        "width": 900,
        "height": 700
      }
    },
    "web": {}
  },
  "deploy": {
    "store": {
      "web": "0",
      "macos": "0",
      "ios": "0",
      "linux": "0"
    }
  }
}
````

`app.test.contract`

````contract
test "opens"
  expect tree has "open"
````

`app.ts`

````ts
import type { Answer, Ctx2D, Frame } from './app.contract.d.ts';
export const appId = 'com.example.x30-any-file-pixels';
export const grants = 'fs.read doc:/';
export const answer: Answer = (async (source: string, [path]: string[], _s: unknown, storage: any) => {
  if (source !== 'readBytes') throw new Error(source);
  const bytes = await storage.fs.readFile(path);
  return { name: path.split('/').pop(), size: bytes.byteLength };
}) as unknown as Answer;
export const surfaces: Record<string, number> = { accent: 0 };
export function draw(_s: string, _a: unknown[], ctx: Ctx2D, _f: Frame): boolean {
  ctx.fillStyle = '#3366ff';
  ctx.fillRect(0, 0, 64, 64);
  // The web's way to average a thumbnail: read the pixels back.
  const data = (ctx as any).getImageData(0, 0, 16, 16);
  console.log(`getImageData answered ${data?.data?.length}`);
  return false;
}
````

</details>

## Discussion at transfer

### daehyeon-mun — 2026-10-07T07:49:35Z

## Decision needed
**[Policy]**: `rules/DEFERRED.md` refuses this by name, so it needs a waiver before implementation. Re-checked on main `78286adc1` (2026-10-07). It still reproduces. Reading any file through `showOpenFilePicker` + `storage.fs.readFile` still works.

**Blocked by:**
- `rules/DEFERRED.md:175`: "Still no picker for any file".
- `rules/DEFERRED.md:688`: readback (`getImageData`, `toDataURL`, `toBlob`) and an app-visible `OffscreenCanvas`.

**Options:**
- **A.** `input type="file"` with no `accept` (or `*/*`) is admitted: NSOpenPanel without a type filter, a document picker on iOS, a plain file input on the web.
- **B.** A data-module image decode/encode API on file bytes, not window pixels: ImageIO on Apple, the browser on the web.
- **C.** No change. Document `showOpenFilePicker` as the any-file path, and keep image work in native modules.

**Recommendation:** admit A. It is the web's own control and the picker already exists. Choose C for B until a second consumer needs image codecs.

**Cost:** A small (the picker plumbing exists); B large (a codec API on four hosts).

### ccheever — 2026-10-08T08:07:50Z

**Decision: Keep image codecs/readback deferred; propose only the any-type file-input exception.**

Keep open with the bounded scope below.

The existing showOpenFilePicker path already supplies arbitrary file bytes. Making input type=file accept the same files is a small ergonomic change; a cross-host image codec/readback subsystem is separate. Charlie confirmed that image codecs stay deferred.

Any-type input still requires a DEFERRED waiver or trade. Until that ruling, use the shipped picker. Keep re-encoding/average-color logic in a native module; do not treat one waiver as Canvas readback admission.
