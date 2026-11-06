surfaces: ios
agent: Codex gpt-6-astra
task: admit the first composer after native preference initialization

### Rough

On a fresh simulator, pairing and project selection succeeded but New Task
showed a held composer. A normal relaunch made it editable. The native question
journal had turned a missing preference file into a nonempty document without
the supported version. Shared loading completed, but editor hydration correctly
refused the document. A later ordinary save supplied the version.

The native read boundary now initializes only a genuinely absent, empty
document without retained attachment pins. Existing invalid documents and
missing preferences beside retained pins keep their recovery restrictions.
No TypeScript admission guard changed.

### Lean in

The existing Foundation fixture executes the production question journal,
attachment inventory and POSIX storage. Its new first-read check failed before
the fix. The fixture also exports the actual first-read document for the
existing TypeScript editor tests. `T3_NATIVE_PREFERENCE_DOCUMENT` selects the
ignored native export. Without it, that integration check reports a skip.

The Foundation compile uses `T3MobileQueuedEdit.swift`,
`T3QuestionAttachmentDisk.swift`, `T3QuestionAttachments.swift`,
`T3QuestionProcess.swift`, `T3QuestionConsumption.swift`,
`T3QuestionCommands.swift`, `T3QuestionCommandWire.swift`, and
`tests/QuestionCommandTests.swift`. It also includes unchanged `T3Failure` and
`T3Endpoint` declarations extracted from the iOS-guarded `T3Protocol.swift`.
The ignored evidence retains their source hashes. Run the executable with
`--first-read-output <path>` to export the document, then pass that path as
`T3_NATIVE_PREFERENCE_DOCUMENT` to `bun test new-task-editor.test.ts`.

### Needed

- persistence: provided by the existing native preference mutex and durable
  writer; initialization must preserve the document version across adapters.
- text editing: provided; admission depends on successful preference hydration.

### Checkpoints

- native component execution: smooth; 89 checks pass after the failing-before
  version assertion, including retained pins and invalid existing documents.
- editor and recovery tests: smooth; 133 tests and 1,172 assertions pass using
  the actual native first-read bytes. The editor accepts Unicode without a
  restart in this fixture.
- strict application TypeScript: smooth.
- normal native build and fresh-start device acceptance: pending for this fix.
