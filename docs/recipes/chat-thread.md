# Recipe: a chat thread

A transcript that opens at the newest message and follows new ones, over a composer.

```contract
routes nav
  home "/"

shape Message
  id: string
  body: string
  mine: bool

shape Reply
  ok: bool
  error: string

component Chat
  state draft = ""
  resource thread = loadThread() as shape list<Message> else empty()
  mutation sent as shape Reply refreshes thread then afterSend
  derive sendError = match sent { case some(r) => r.error, case none => "" }
  action edit(v: string)
    draft = v
  action post
    if trim(draft) != "" and not pending(sent)
      send sent = sendMessage(trim(draft))
  action afterSend
    if sendError == ""
      draft = ""
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" interactive-widget="resizes-content" testId="app"
      width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="transcript" position="absolute" inset=0
          header padding="8px 16px"
            text "Ada" role="heading" aria-level=2
          list id="transcript" virtualized=true scroll-start="end" scrollFollowEnd=true
            flex=1 min-height=0 padding="8px 0"
            each m in thread key=m.id
              row justify-content=(m.mine ? "flex-end" : "flex-start") padding="3px 12px"
                text m.body padding="8px 12px" border-radius=18 max-width="75%" overflow-wrap="anywhere"
                  background-color=(m.mine ? "AccentColor" : "-exact-secondary-background")
                  color=(m.mine ? "AccentColorText" : "-exact-label") testId=`message-${m.id}`
          row gap=8 padding="8px 12px" align-items="center"
            input value=draft input=edit submit=post placeholder="Message"
              enterkeyhint="send" testId="composer" flex=1 min-width=0
            button press=post disabled=(trim(draft) == "" or pending(sent)) aria-label="Send" testId="send"
              image "symbol:send"
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.chat'; // exact new writes this line
export const grants = '';

const messages: Result<'loadThread'> = [];

const sources: Sources = {
  loadThread: () => messages,
  sendMessage: ([body]) => {
    messages.push({ id: String(messages.length + 1), body, mine: true });
    return { ok: true, error: '' };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "a sent message lands, and the composer clears"
  type "composer" "Yes, at noon"
  tap "send"
  clock data
  expect text "composer" == ""
  expect text "message-1" == "Yes, at noon"
```

- `scroll-start="end"` opens at the newest row; `scrollFollowEnd=true` follows new rows
  while the reader is there. `interactive-widget="resizes-content"` on the root keeps the
  composer above the keyboard.
- The draft clears in `then` on an ok reply; a refusal keeps it and says why above the
  composer ([after-a-write](after-a-write.md)).
