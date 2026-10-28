# @exact/clipboard: what a person checks (LLP 1133.006 §8, D3, D5)

The fixture is `packages/clipboard/test/fixture`, made into an app by
`fixture.mjs` (`EXACT_CLIPBOARD_FIXTURE=<dir>` keeps it). Build it from the
commit under test. Each step says what you should see.

## macOS, the real pasteboard

Run the fixture with `EXACT_DEVICE=real` (`bun exact.mjs agent macos`
with that variable set, or a plain `bun exact.mjs mac` build, which has no
agent).

1. Press **png**. In Preview, File > New from Clipboard opens an 8 × 8 red
   image.
2. Press **apng**. New from Clipboard opens an 8 × 8 **green** image: frame
   0 of the animation, not its red default image.
3. Press **both**. Paste into Messages: the red image arrives; paste into
   TextEdit as plain text (Edit > Paste and Match Style): the URL arrives.
4. Press **text**. Paste into TextEdit: "Copied from the fixture".
5. Run `osascript -e 'clipboard info'` after step 1 and record the types
   listed (D5: what `writeObjects([NSImage])` writes, and `public.png`).

## iOS, on a phone

6. Press **png**, then paste into Notes: an 8 × 8 red image. No banner
   appears for the write (D5).

## The web: when a write may happen (D3)

7. In Safari and in Firefox, open the fixture's web build from a served
   origin and press **png**. Record whether it writes or rejects
   `NotAllowedError`, and the browser's version.
8. Repeat with a delay before the write: in `app.ts`, await a 1 s timer
   before `write`. Record whether each browser still writes.
