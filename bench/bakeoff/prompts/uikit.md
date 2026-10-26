# Your toolkit: UIKit

Build it with UIKit, programmatically (no storyboards; SwiftUI only where UIKit cannot do something). Your working
directory holds a skeleton: read its `README.md`, and replace its SwiftUI `App.swift` with a UIKit app delegate and
scene delegate (declare the scene manifest in `EXTRA_PLIST` in `app.env`). Put the name and bundle id above in
`app.env`. `SIM={SIM} ./build.sh --run` builds, installs and launches the app. Leave the finished app installed on
that simulator.
