import AppKit

// Which helper snapshot setup and requestPermissions dock, outside agent mode (reference
// DesktopSnapShot setup and requestPermissions, and their tests). Every grant probe,
// prompt, System Settings opener and the helper itself are fakes: nothing reaches TCC.
func runPermissionRequestChecks() {
    _ = NSApplication.shared; NSApp.setActivationPolicy(.accessory)
    var passed = 0
    func expect(_ value: Bool, _ name: String) { guard value else { fatalError(name) }; passed += 1 }
    let owner = NSWindow(contentRect: CGRect(x: 100, y: 100, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
    owner.isReleasedWhenClosed = false
    var screen = false, accessibility = false, promptGrantsScreen = false
    var calls: [String] = [], opened: [URL] = [], helpers: [(permission: T3MacPermission, owner: NSWindow?, isGranted: () -> Bool)] = []
    let snap = T3SnapShot(directory: URL(fileURLWithPath: FileManager.default.currentDirectoryPath + "/target/t3-tests/snapshot/permissions"), agent: false, changed: { _ in })
    snap.permissions = T3SnapshotPermissionSystem(
        screenRecording: { screen }, accessibility: { calls.append("accessibility"); return accessibility },
        promptAccessibility: { calls.append("prompt-accessibility"); return accessibility },
        requestScreenRecording: { calls.append("prompt-screen"); screen = screen || promptGrantsScreen; return screen },
        openSettings: { opened.append($0) }, focusedWindow: { owner })
    snap.showHelper = { helpers.append(($0, $1, $2)) }
    let screenSettings = T3MacPermission.screenRecording.settingsURL, accessibilitySettings = T3MacPermission.accessibility.settingsURL
    func run(_ request: [String: Any], screen hasScreen: Bool, accessibility hasAccessibility: Bool, promptGrants: Bool = false) -> [String: Any] {
        screen = hasScreen; accessibility = hasAccessibility; promptGrantsScreen = promptGrants; calls = []; opened = []; helpers = []
        var reply: [String: Any] = [:]
        snap.perform(request) { reply = $0 }
        return reply
    }
    func request(_ include: Bool, screen: Bool, accessibility: Bool, promptGrants: Bool = false) -> Bool {
        let reply = run(["op": "snapshotRequestPermissions", "includeAccessibility": include], screen: screen, accessibility: accessibility, promptGrants: promptGrants)
        return reply["ok"] as? Bool == true && (reply["value"] as? [String: Any])?["requested"] as? Bool == true
    }
    func setup(_ action: String, screen: Bool, accessibility: Bool) -> Bool {
        run(["op": "snapshotSetup", "action": action], screen: screen, accessibility: accessibility)["ok"] as? Bool == true
    }
    func docked() -> [T3MacPermission] { helpers.map(\.permission) }

    // requestPermissions (setup's Continue, Include app text).
    expect(request(true, screen: false, accessibility: false), "requestPermissions answers outside agent mode")
    expect(calls == ["prompt-accessibility", "prompt-screen"] && opened == [screenSettings], "both missing: the Accessibility prompt, then Screen Recording's prompt and its Settings pane, never Accessibility's: \(calls) \(opened)")
    expect(docked() == [.screenRecording] && helpers[0].owner === owner, "both missing: Screen Recording's helper docks first, for the focused window")
    expect(request(false, screen: false, accessibility: false) && calls == ["prompt-screen"] && opened == [screenSettings] && docked() == [.screenRecording], "without app text Accessibility is never read or prompted: \(calls)")
    expect(request(true, screen: true, accessibility: false) && calls == ["prompt-accessibility", "accessibility"] && opened == [accessibilitySettings] && docked() == [.accessibility], "Screen Recording granted: Accessibility's pane opens and its helper docks: \(calls) \(opened)")
    expect(request(true, screen: false, accessibility: false, promptGrants: true) && opened == [accessibilitySettings] && docked() == [.accessibility], "a Screen Recording prompt that grants opens no Screen Recording pane; Accessibility follows")
    expect(request(false, screen: true, accessibility: false) && calls.isEmpty && opened.isEmpty && helpers.isEmpty, "without app text and with Screen Recording granted nothing opens or docks")
    expect(request(true, screen: true, accessibility: true) && calls == ["prompt-accessibility", "accessibility"] && opened.isEmpty && helpers.isEmpty, "with both grants present no pane opens and no helper docks")

    // setup's Allow buttons: each requests only its own grant and docks its own helper.
    expect(setup("allow-accessibility", screen: true, accessibility: false) && calls == ["prompt-accessibility"] && opened == [accessibilitySettings] && docked() == [.accessibility], "Allow Accessibility prompts, opens its pane and docks its helper: \(calls)")
    let accessibilityHelper = helpers[0]
    expect(!accessibilityHelper.isGranted() && helpers[0].owner === owner, "its helper polls the Accessibility grant, still missing")
    accessibility = true; screen = false
    expect(accessibilityHelper.isGranted(), "and closes once Accessibility is granted (the Screen Recording grant does not count)")
    expect(setup("allow-screen-recording", screen: false, accessibility: false) && calls == ["prompt-screen"] && opened == [screenSettings] && docked() == [.screenRecording], "Allow Screen Recording prompts once, opens its pane and docks its helper: \(calls)")
    let screenHelper = helpers[0]
    accessibility = true; expect(!screenHelper.isGranted(), "its helper polls the Screen Recording grant, still missing")
    screen = true; expect(screenHelper.isGranted(), "and closes once Screen Recording is granted")
    expect(setup("allow-accessibility", screen: false, accessibility: true) && opened.isEmpty && helpers.count == 1 && helpers[0].isGranted(), "Allow with Accessibility present opens no pane; the helper sees the grant and opens nothing")
    expect(setup("allow-screen-recording", screen: true, accessibility: false) && calls.isEmpty && opened.isEmpty && helpers.count == 1 && helpers[0].isGranted(), "Allow with Screen Recording present neither prompts nor opens a pane; the helper sees the grant and opens nothing")

    // realinput-1010-fixes RI-3: the setup rows (snapshotState) and the helper's grant poll read the same check (reference
    // currentMacPermissions and permissionGranted: getMediaAccessStatus("screen") and isTrustedAccessibilityClient(false)
    // on both sides), so the helper cannot close on a grant the Screen Recording row does not show.
    func rows() -> (screen: Bool, accessibility: Bool) {
        var reply: [String: Any] = [:]
        snap.perform(["op": "snapshotState", "owner": ""]) { reply = $0 }
        let value = reply["value"] as? [String: Any] ?? [:]
        return (value["screenRecording"] as? Bool ?? false, value["accessibility"] as? Bool ?? false)
    }
    _ = setup("allow-screen-recording", screen: false, accessibility: false)
    let poll = helpers[0].isGranted
    expect(!poll() && rows().screen == false, "Screen Recording missing: the helper keeps polling and the row reads Allow")
    screen = true
    expect(poll() && rows().screen == true, "the grant the helper's poll sees is the grant the row shows")
    _ = setup("allow-accessibility", screen: true, accessibility: false)
    let accessibilityPoll = helpers[0].isGranted
    expect(!accessibilityPoll() && rows().accessibility == false, "Accessibility missing on both")
    accessibility = true
    expect(accessibilityPoll() && rows().accessibility == true, "and granted on both")

    snap.destroy()
    print("\(passed) snapshot permission request checks passed (fake grants, no TCC prompt)")
}
