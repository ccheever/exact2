macOS agent single-call `tap` can lose its queued mouse-up on Black/Xcode 27.
Separate `tap <target> down` / `tap up` and keyboard activation work; verify
`AgentMouseRelease` queue matching without weakening its foreign-event guard.
