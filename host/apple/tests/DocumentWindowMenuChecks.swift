// Appended to ExactMac/main.swift by menubar.test.mjs. Production windows,
// launch, focus notifications and menu dispatch; no manual menu installation.
readySent = true
var failures: [String] = []
var presses: [String] = []
var focusWindows: [(DocumentWindow, Bool, String)] = []
var focusIndex = 0
func check(_ condition: Bool, _ message: String) {
    if !condition { failures.append(message) }
}
func finishMenuChecks() {
    let current = focusWindows.last!.0
    firstWindow.session.presenter.shortcuts.sync()
    check(DevMenu.session === current.session, "background sync preserves key session")
    let help = NSApp.helpMenu!
    let helpOwner = help.items.compactMap { $0.target as? ShortcutHost }.first!
    DispatchQueue.main.async {
        for _ in 0..<3 {
            helpOwner.sync(); help.update()
            // AppKit's search header is presented before NSMenu.items.
            let shown = help.accessibilityAttributeValue(.children) as? [NSMenuItem] ?? []
            check(shown.first?.view != nil, "Help: AppKit search field leads")
            check(shown.dropFirst().first?.title == "Documentation", "Help: app command below search")
            check(shown.last?.action == #selector(NSApplication.showHelp(_:)), "Help: host command follows app command")
        }
        help.cancelTracking()
        for failure in failures { print("FAIL: \(failure)") }
        print("DocumentWindow menu focus: \(failures.count) failures")
        exit(failures.isEmpty ? 0 : 1)
    }
    NSApp.mainMenu!.items.last!.accessibilityPerformAction(.press)
}
func focusNextWindow() {
    guard focusIndex < focusWindows.count else { finishMenuChecks(); return }
    let (owner, removed, label) = focusWindows[focusIndex]
    app.activate(ignoringOtherApps: true)
    owner.window.makeKeyAndOrderFront(nil)
    let deadline = Date().addingTimeInterval(5)
    // Let the main queue return so AppKit can complete application activation.
    Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { timer in
        guard NSApp.keyWindow === owner.window || Date() >= deadline else { return }
        timer.invalidate()
        print("FOCUS", label, "agent", agentMode, "active", app.isActive, "policy", app.activationPolicy().rawValue,
              "key", owner.window.isKeyWindow, "visible", owner.window.isVisible, "canKey", owner.window.canBecomeKey,
              "appKey", String(describing: NSApp.keyWindow))
        check(NSApp.keyWindow === owner.window, "\(label): real key window")
        let menu = NSApp.mainMenu!.items.compactMap(\.submenu).first!
        menu.update()
        if let updates = menu.items.first(where: { $0.title == "Check for Updates…" }) {
            check(updates.isEnabled, "\(label): chordless item enabled")
            check(updates.target === owner.session.presenter.shortcuts, "\(label): action belongs to key presenter")
            menu.performActionForItem(at: menu.index(of: updates))
            check(presses.last == label, "\(label): menu dispatch reaches key session")
        } else { failures.append("\(label): missing app command") }
        let viewMenu = NSApp.mainMenu!.items.compactMap(\.submenu).first { $0.title == "View" }!
        viewMenu.update()
        let fullScreen = viewMenu.items.first { $0.action == #selector(NSWindow.toggleFullScreen(_:)) }!
        check(fullScreen.isHidden == removed, "\(label): removal policy follows key root")
        focusIndex += 1
        DispatchQueue.main.async { focusNextWindow() }
    }
}
let launchObservation = NotificationCenter.default.addObserver(forName: NSApplication.didFinishLaunchingNotification, object: app, queue: .main) { _ in
    DispatchQueue.main.async {
        if ProcessInfo.processInfo.environment["EXACT_MENU_HELP_ONLY"] == "1" {
            focusWindows = [(firstWindow, true, "main")]
            finishMenuChecks()
            return
        }
        let second = openWindow()
        let otherPlan = try! Data(contentsOf: URL(fileURLWithPath: ProcessInfo.processInfo.environment["EXACT_MENU_OTHER_PLAN"]!))
        _ = second.session.boot(plan: otherPlan, size: size)
        second.attach()
        firstWindow.session.presenter.onPress = { _ in presses.append("main") }
        second.session.presenter.onPress = { _ in presses.append("second") }
        focusWindows = [(firstWindow, true, "main"), (second, false, "second"), (firstWindow, true, "main"), (second, false, "second")]
        focusNextWindow()
    }
}
app.run()
