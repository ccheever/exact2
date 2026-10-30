# @exact/child-process

Programs a desktop app's data code starts, with Node's `child_process`
names (LLP 1133.007). macOS and Linux run them; iOS, tvOS, Android,
Windows and the web answer every name with `NotSupportedError`.

```ts
import { spawn, execFile, processStatus } from "@exact/child-process";
export const grants = "process.spawn path:t3";

// A resource's answer: the child lives while the resource holds it.
return spawn("path:t3", ["--serve"], {
  encoding: "utf8",
  exactRestart: { when: "always", delay: 500 },
  exactStream(event, child) { … },
});

// A one-shot run inside an answer.
const { stdout } = await execFile("path:t3", ["--version"], { encoding: "utf8" });
```

- Each program needs a `process.spawn` grant line naming it.
- Only a person's input or the app's own code starts, writes to, ends or
  signals a child; a tool's call, a wake or a device event is refused
  `person-required`.
- Under the agent nothing runs unless the drive names it:
  `bun exact.mjs agent macos --process path:t3=script:<file.json> …`.

`docs/reference.md`'s "Child processes" has the options, the events and
the limits. `test/` holds the stage drives' fixture app (`bun fixture.mjs`).
