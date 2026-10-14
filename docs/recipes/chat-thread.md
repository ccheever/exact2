# Recipe: a chat thread

A transcript that opens at the newest message, over a composer.

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
    main navigationKey=`${top(nav).id}` navigationBack="back" interactive-widget="resizes-content"
      width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="transcript" position="absolute" inset=0
          header
            text "Ada" role="heading" aria-level=2
          list id="transcript" appearance="none" virtualized=true scroll-start="end" scrollFollowEnd=true flex=1 min-height=0
            each m in thread key=m.id
              row justify-content=(m.mine ? "flex-end" : "flex-start") padding="3px 12px"
                row position="relative" overflow="hidden" border-radius=18 max-width="75%"
                  view position="absolute" inset=0 opacity=(m.mine ? 0.35 : 1)
                    background-color=(m.mine ? "AccentColor" : "-exact-fill")
                  text m.body padding="8px 12px" position="relative" overflow-wrap="anywhere" testId=`message-${m.id}`
          row gap=8 padding="8px 12px" align-items="center"
            input value=draft input=edit submit=post placeholder="Message"
              enterkeyhint="send" testId="composer" flex=1 min-width=0
            button press=post disabled=(trim(draft) == "" or pending(sent)) buttonStyle="filled" aria-label="Send" testId="send"
              image "symbol:send"
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.chat';
export const grants = '';

const messages: Result<'loadThread'> = [];

const sources: Sources = {
  loadThread: () => messages,
  sendMessage: ([body]) => {
    messages.push({ id: String(messages.length + 1), body, mine: true });
    return { error: '' };
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

- `appearance="none"`: bubbles are not cells. `scroll-start="end"` opens at the newest row,
  `scrollFollowEnd=true` follows new ones, `interactive-widget="resizes-content"` keeps the
  composer above the keyboard. A refusal keeps the draft ([after-a-write](after-a-write.md)).
- The sent bubble is the accent at 0.35 under the label's ink; solid, `check` fails it.
