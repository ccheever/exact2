# Final physical Account keyboard traversal

Final combined Apple --bundle build passed. Fresh harmless auth flow on local backend16332, native GUI local16338.

Physical Tab moved focus from Terminal input to Registry agent ID. Physical ShiftTab moved focus to Cancel; the deferred result was confirmed in a fresh AX snapshot. The backend recorded zero PTY input bytes during both traversal operations. The flow was canceled and fixture app stopped.

This supersedes the unresolved Tab limitation in ../gui-native/RESULTS.md. Screenshot final-account.png contains only the harmless prompt. Source fingerprints: ../source-fingerprints-final.json. Sanitized assertions: ../sanitized-runtime-proof.json.
