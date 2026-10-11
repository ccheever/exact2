# Recipe: a chat thread

A transcript that opens at the newest message and follows new ones, with a composer
that stays above the keyboard and clears after each send.

```contract
routes nav
  home "/"

shape Message
  id: string
  body: string
  mine: bool

shape Thread
  messages: list<Message>

shape Ack
  ok: bool

component Chat
  state draft = ""
  resource thread = loadThread() as shape Thread else empty()
  mutation sent as shape Ack queue refreshes thread
  action edit(v: string)
    draft = v
  action post
    let body = trim(draft)
    if body != ""
      send sent = sendMessage(body)
      draft = ""
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" interactive-widget="resizes-content" testId="app"
      width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="transcript" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" align-items="center" padding="8px 16px"
            text "Ada" role="heading" aria-level=2
          list id="transcript" virtualized=true scroll-start="end" scrollFollowEnd=true keyboardDismissMode="interactive"
            flex=1 min-height=0 padding-top=8 padding-bottom=8
            each m in thread.messages key=m.id
              row justify-content=(m.mine ? "flex-end" : "flex-start") padding="3px 12px"
                text m.body padding="8px 12px" border-radius=18 max-width="75%" overflow-wrap="anywhere"
                  background-color=(m.mine ? "AccentColor" : "-exact-secondary-background")
                  color=(m.mine ? "AccentColorText" : "-exact-label") testId=`message-${m.id}`
          row gap=8 padding="8px 12px" align-items="center"
            input value=draft input=edit submit=post placeholder="Message" aria-label="Message"
              enterkeyhint="send" testId="composer" flex=1 min-width=0
            button press=post disabled=(trim(draft) == "") aria-label="Send" testId="send"
              image "symbol:send"
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.chat'; // exact new writes this line
export const grants = '';

const messages: Result<'loadThread'>['messages'] = [{ id: '1', body: 'Are we still on for lunch?', mine: false }];

const sources: Sources = {
  loadThread: () => ({ messages }),
  sendMessage: ([body]) => {
    messages.push({ id: String(messages.length + 1), body, mine: true });
    return { ok: true };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "a sent message lands last and the composer clears"
  type "composer" "Yes, at noon"
  tap "send"
  clock data
  expect text "composer" == ""
  expect text "message-2" == "Yes, at noon"
```

- `scroll-start="end"` opens the list at its newest row, and `scrollFollowEnd=true`
  follows rows added at the end while the reader is there; a reader who scrolled up
  stays put. A jump-to-latest button reads the list's `scroll` event
  ([grammar: events](../contract-grammar.md#events)).
- Clear the draft in the action that sends, and disable Send for blank text.
- `interactive-widget="resizes-content"` on the root keeps the composer above the
  keyboard; `keyboardDismissMode="interactive"` lets a drag pull the keyboard down.
- Bubbles are deliberate design, so they set a shape and colours, in roles that follow
  dark mode. Messages from a server, polled: [rest-backend](rest-backend.md).
