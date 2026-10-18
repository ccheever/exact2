# Recipe: a chat thread

A transcript open at its newest message, over Messages' composer.

```contract
routes nav
  home "/"

shape Message
  id: string
  body: string
  mine: bool

shape Reply
  error: string

component Chat
  state draft = ""
  resource thread = loadThread() as shape Message[] else empty()
  mutation sent as shape Reply refreshes thread then afterSend
  derive sendError = match sent { case some(r) => r.error, case none => "" }
  action edit(v: string)
    draft = v
  action post
    if draft.trim() != "" and not pending(sent)
      send sent = sendMessage(draft.trim())
    focus("composer")
  action afterSend
    if sendError == ""
      draft = ""
  view
    main navigationKey=`${top(nav).id}` interactive-widget="resizes-content" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="transcript" position="absolute" inset=0
          header
            h2 "Ada"
          list id="transcript" appearance="none" virtualized=true scroll-start="end" scrollFollowEnd=true flex=1 min-height=0
            for m in thread key=m.id
              row justify-content=(m.mine ? "flex-end" : "flex-start") padding="3px 12px"
                text m.body padding="8px 12px" border-radius=18 max-width="75%" overflow-wrap="break-word"
                  background-color=(m.mine ? "AccentColor" : "-exact-fill") color=(m.mine ? "AccentColorText" : "-exact-label")
                  testId=`message-${m.id}`
          row align-items="flex-end" margin="6px 12px" padding="4px 4px 4px 12px" border-radius=20
            border="1px solid -exact-separator"
            textarea id="composer" value=draft input=edit rows=1 field-sizing="content" max-height=140
              placeholder="Message" aria-label="Message" testId="composer" flex=1 min-width=0
              border="none" background-color="transparent"
            button press=post disabled=(draft.trim() == "" or pending(sent)) buttonStyle="filled"
              retainFocus=true aria-label="Send" testId="send"
              image "symbol:send"
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.chat';
export const grants = '';

const messages: Result<'loadThread'> = [];
const sources: Sources = {
  loadThread: () => messages,
  sendMessage: ([body]) => (messages.push({ id: String(messages.length + 1), body, mine: true }), { error: '' }),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "a sent message lands; the composer clears"
  type "composer" "Yes, at noon"
  tap "send"
  clock data
  expect text "composer" == ""
  expect text "message-1" == "Yes, at noon"
```

- Bubbles are not cells (`appearance="none"`); `scroll-start="end"` opens at the newest;
  `interactive-widget` keeps the composer over the keyboard; Send keeps it up.
- Yours: the solid accent under `AccentColorText` (white on iOS; 3:1 in `check`), no name;
  a group names others'.
