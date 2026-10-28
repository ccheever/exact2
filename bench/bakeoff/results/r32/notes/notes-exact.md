# Notes: build notes

Built with Exact from the `list-detail` pattern. The finished app (`dev.bakeoff.notes.bbc08a4`,
"Notes") is installed on simulator EEFC0029-1010-45E4-8242-42CF67E16C96. `verify` (web) and
`verify ios` are both clean: 5 tests pass; there are no findings in light or dark mode; nothing
spills at the largest text size; scrolling is "like UIKit"; and a relaunch keeps the data.

## What the app does
- **List:** the large title "Notes" with the count as its subtitle ("90 Notes"), a compose button
  and the bar's search field. Pinned notes appear first under their own "Pinned" header, then
  "Notes". Both are sorted newest first. Each row shows the title and, on the line under it, the
  date in Notes/Mail style (the time today, then Yesterday, then a weekday, then 10/11/26) and a
  one-line preview of the body.
- **Search** filters by title or body as you type, ignoring case. When nothing matches it shows
  an empty state.
- **Editor:** a pushed screen with the date edited, a borderless large title field and a body
  textarea. Every keystroke is saved (a queued mutation) and makes the note the newest. Return
  in the title moves to the body.
- **New note:** the compose button opens an empty note with the title focused. When you leave a
  note whose title and body are both blank, it is deleted. That also covers an existing note
  that was emptied, as Apple Notes does.
- **Pin, unpin and delete:** a long press on a row opens a menu (Pin Note / Unpin Note, Delete),
  and swiping a row deletes it.
- **Persistence:** the notes are stored in `app:/data/notes.json`. On first launch they come
  from the bundled `data/notes.json`, which `app.ts` imports.

## What was hard or guessed
- **Leaving a note.** The system Back button and the edge swipe don't call your own action. They
  arrive at the root's `navigate` handler with the URL underneath. I found this in
  contract-for-agents.md (the table of what each host draws), so the discard-if-empty logic
  lives in `follow(url)`.
- **Relaunching onto an open note.** A reload can restore the route but not the in-memory
  draft. Discarding is therefore guarded by `draftId == the route's id`, and the editor falls
  back to the saved note's text, so a restored note is never deleted by mistake.
- **"Move to top" with a clock behind the data.** Agent drives run on a virtual clock that starts
  at 2026-01-01, which is older than the shipped notes, so an edited note sank. `saveNote` now
  stores `max(now, newest other note + 1 s)`. The date shown can only be wrong if the device
  clock is behind the data.
- **Swipe actions.** Delete is a one-word attribute, but Pin as a swipe action needs the
  long-form snap-scroll swipe. I put Pin in the long-press menu instead.
- **The body placeholder** rendered tiny on iOS until I gave the textarea `font="-exact-body"`.

## Not done
- There is no leading swipe to pin (pinning is in the long-press menu only).
- The editor has no "Done" or share button in the bar.
- The app is iPhone only: I set `deviceFamily` to iPhone, so there is no iPad split view.

## What would have made this faster
- A "notes" or "document editor" pattern: a list plus a pushed editor that saves as you type and
  discards an empty item on leave.
- A one-attribute leading or trailing swipe action other than Delete (for example `swipePin=`).
- Saying in start-here that the platform Back goes through `navigate`, not `navigationBack`,
  unless a control carries that id.
