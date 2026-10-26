# The app

`Sources/*.swift` is the app; `Resources/` is copied to the bundle's root (read a file with
`Bundle.main.url(forResource:withExtension:)`). `app.env` names it. There is no Xcode project:
`./build.sh` compiles every Swift file in `Sources/` with `swiftc` for the iOS simulator, installs the
app on `$SIM` and, with `--run`, launches it. Extra Info.plist keys go in `EXTRA_PLIST` in `app.env`,
as raw plist XML. To drive and test the app, use `xcrun simctl` (screenshots, launch, terminate,
appearance, content size) and XCTest-free checks of your own; no extra tools are installed.
