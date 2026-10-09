import AppKit

// The permission helper beside System Settings (reference MacPermissionHelper.test.ts and
// MacSettingsWindow.test.ts): real AppKit panels, a fake Settings window feed, no grants.
private func spin(_ seconds: TimeInterval) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
private func check(_ value: Bool, _ name: String) { guard value else { fatalError(name) } }

func runPermissionHelperChecks() {
    _ = NSApplication.shared; NSApp.setActivationPolicy(.accessory)
    var passed = 0
    func expect(_ value: Bool, _ name: String) { check(value, name); passed += 1 }

    // Placement (reference settingsHelperBounds): inside the content column, 16 pt above the bottom.
    let docked = T3SettingsWindow.helperBounds(CGRect(x: 367, y: 100, width: 723, height: 719))
    expect(docked == CGRect(x: 599, y: 663, width: 475, height: 140), "docks inside a 723×719 Settings window: \(docked)")
    let moved = T3SettingsWindow.helperBounds(CGRect(x: -800, y: 200, width: 723, height: 719))
    expect(moved == CGRect(x: -568, y: 763, width: 475, height: 140), "follows Settings onto a display left of the main one: \(moved)")
    for x in [-900, 20] as [CGFloat] {
        for width in [668, 1000] as [CGFloat] {
            let helper = T3SettingsWindow.helperBounds(CGRect(x: x, y: 30, width: width, height: 700))
            expect(helper.minX >= x + 216 && helper.maxX <= x + width - 16 && helper.maxY == 714, "stays in the content column of a \(width) pt window at x \(x)")
        }
    }
    expect(T3SettingsWindow.helperBounds(CGRect(x: 0, y: 0, width: 1400, height: 800)).width == 560, "never wider than 560 pt")
    expect(T3SnapshotFeedback.screenFrame(docked, screenHeight: 1000) == CGRect(x: 599, y: 197, width: 475, height: 140), "CG top-left bounds become AppKit bottom-left frames")

    // The Settings window from the window list (reference SETTINGS_WINDOW_SCRIPT).
    func entry(_ pid: Int32, layer: Int = 0, _ frame: CGRect) -> [String: Any] {
        [kCGWindowOwnerPID as String: NSNumber(value: pid), kCGWindowLayer as String: NSNumber(value: layer), kCGWindowBounds as String: frame.dictionaryRepresentation as NSDictionary as! [String: Any]]
    }
    let main = CGRect(x: 10, y: 20, width: 723, height: 719)
    let list = [entry(7, main), entry(9, CGRect(x: 0, y: 0, width: 300, height: 200)), entry(9, layer: 3, main), entry(9, main)]
    expect(T3SettingsWindow.find(in: list, pid: 9, frontmost: 9) == .window(main, frontmost: true), "takes Settings' own large layer-0 window and its frontmost state")
    expect(T3SettingsWindow.find(in: list, pid: 9, frontmost: 7) == .window(main, frontmost: false), "reports a covered Settings window as not frontmost")
    expect(T3SettingsWindow.find(in: list, pid: 11, frontmost: 11) == .absent, "another app's windows are never Settings")
    expect(T3SettingsWindow.find(in: [entry(9, CGRect(x: 0, y: 0, width: 499, height: 600))], pid: 9, frontmost: 9) == .absent, "a sheet narrower than 500 pt is not the Settings window")

    // The watcher reports only changes, polls faster while Settings is frontmost, and stops.
    var reading = T3SettingsReading.absent, seen: [T3SettingsReading] = []
    let watcher = T3SettingsWindowWatcher(read: { reading }, changed: { seen.append($0) })
    watcher.start(); expect(seen == [.absent], "the first reading is always reported")
    spin(1.1); expect(seen == [.absent], "an unchanged reading is not reported again")
    reading = .window(main, frontmost: true); spin(1.1)
    expect(seen == [.absent, .window(main, frontmost: true)], "a new Settings window is reported within a second")
    reading = .window(main.offsetBy(dx: 40, dy: 0), frontmost: true); spin(0.6)
    expect(seen.count == 3, "a frontmost Settings window is polled every 0.5 s")
    watcher.stop(); reading = .unavailable; spin(1.1)
    expect(seen.count == 3, "a stopped watcher reports nothing more")

    // App bundle (reference macAppBundlePath).
    expect(T3PermissionHelper.appBundle(executable: "/Applications/T3 Code.app/Contents/MacOS/T3 Code")?.path == "/Applications/T3 Code.app", "resolves a bundle with spaces")
    expect(T3PermissionHelper.appBundle(executable: "/usr/local/bin/t3-tests") == nil, "a bare executable has no bundle to drag")
    expect(T3PermissionHelper.appBundle(executable: "/Applications/T3 Code.app/other/MacOS/T3 Code") == nil, "only Contents/MacOS executables count")

    // The helper itself, with a fake Settings feed and a fake Finder.
    let bundle = URL(fileURLWithPath: "/Applications/T3 Code (Exact).app", isDirectory: true)
    let icon = NSImage(size: NSSize(width: 64, height: 64), flipped: false) { rect in NSColor.systemIndigo.setFill(); NSBezierPath(roundedRect: rect, xRadius: 14, yRadius: 14).fill(); return true }
    var feed: ((T3SettingsReading) -> Void)?, stops = 0, revealed: [URL] = [], returned = 0, finished = 0, granted = false
    let helper = T3PermissionHelper()
    helper.watch = { changed in feed = changed; return { stops += 1 } }
    helper.primaryScreenHeight = { 1000 }
    helper.reveal = { revealed.append($0) }
    helper.returnTo = { _ in returned += 1 }
    helper.finished = { finished += 1 }
    let owner = NSWindow(contentRect: CGRect(x: 100, y: 100, width: 400, height: 300), styleMask: [.titled, .closable], backing: .buffered, defer: false)
    owner.isReleasedWhenClosed = false
    func open(_ permission: T3MacPermission = .screenRecording, bundle: URL? = bundle) { helper.show(permission, owner: owner, isGranted: { granted }, bundle: bundle, icon: icon) }

    granted = true; open(); expect(helper.panel == nil && feed == nil, "a permission already granted opens nothing")
    granted = false; open(bundle: nil); expect(helper.panel == nil, "outside an app bundle there is nothing to drag, so no helper")
    open()
    guard let panel = helper.panel else { fatalError("helper panel opens") }
    expect(!panel.isVisible, "the panel waits for the Settings window before it shows")
    expect(panel.title == "Set up Screen Recording" && panel.level == .floating && !panel.hidesOnDeactivate && !panel.hasShadow && !panel.isOpaque, "a frameless, always-on-top panel titled for its permission")
    expect(panel.styleMask.contains(.nonactivatingPanel), "using the panel never activates T3 Code over System Settings")
    let content = panel.content
    content.pointer = { NSPoint(x: -10_000, y: -10_000) } // the real pointer may rest where the panel docks
    expect(content.title.stringValue == "↑ Drag T3 Code into the list above" && content.appRow.label.stringValue == "T3 Code", "reference header and app row text")
    expect(content.appRow.accessibilityLabel() == "Drag T3 Code to System Settings, or click to reveal in Finder" && content.closeButton.accessibilityLabel() == "Close permission helper", "reference accessible names")
    content.layoutSubtreeIfNeeded()
    expect(content.appRow.frame == NSRect(x: 23, y: 65, width: 514, height: 52) && content.closeButton.frame == NSRect(x: 527, y: 9, width: 22, height: 22), "reference 20 pt padding, 52 pt row and close corner: \(content.appRow.frame) \(content.closeButton.frame)")

    feed?(.window(CGRect(x: 367, y: 100, width: 723, height: 719), frontmost: true))
    expect(panel.isVisible && panel.frame == CGRect(x: 599, y: 197, width: 475, height: 140), "docks under the frontmost Settings window: \(panel.frame)")
    content.layoutSubtreeIfNeeded()
    expect(content.appRow.frame.width == 475 - 46, "the row follows the docked width")
    feed?(.window(CGRect(x: -800, y: 200, width: 723, height: 719), frontmost: true))
    expect(panel.isVisible && panel.frame == CGRect(x: -568, y: 97, width: 475, height: 140), "follows Settings when it moves: \(panel.frame)")
    feed?(.window(CGRect(x: -800, y: 200, width: 723, height: 719), frontmost: false))
    expect(!panel.isVisible && helper.panel === panel, "hides, but stays open, while another app covers Settings")
    feed?(.window(CGRect(x: 367, y: 100, width: 723, height: 719), frontmost: true)); feed?(.unavailable)
    expect(!panel.isVisible && helper.panel === panel, "hides when Settings cannot be read")
    feed?(.window(CGRect(x: 367, y: 100, width: 723, height: 719), frontmost: true))
    expect(panel.isVisible, "and shows again on the next good reading")

    // Hover shows the close button; a click on the row reveals the app in Finder.
    let enter = NSEvent.enterExitEvent(with: .mouseEntered, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: panel.windowNumber, context: nil, eventNumber: 0, trackingNumber: 0, userData: nil)!
    expect(!content.closeVisible, "the close button is hidden until the panel is hovered")
    content.mouseEntered(with: enter); expect(content.closeVisible, "hovering the panel shows the close button")
    content.mouseExited(with: enter); expect(!content.closeVisible, "leaving the panel hides it again")
    // Ordering out or in under a still pointer sends no exit or entry: the hover is read again.
    content.pointer = { NSPoint(x: 700, y: 250) }; content.mouseEntered(with: enter)
    panel.orderOut(nil); expect(!content.closeVisible, "the panel hidden under the pointer (a Finder reveal) drops its hover")
    panel.orderFrontRegardless(); expect(content.closeVisible, "back under the pointer, × shows again")
    panel.orderOut(nil); content.pointer = { NSPoint(x: 10, y: 10) }; panel.orderFrontRegardless()
    expect(!content.closeVisible, "back with the pointer elsewhere, × stays hidden")
    // AppKit sends no mouseUp after a drag: the drag's end releases the press.
    let row = content.appRow
    row.mouseDown(with: NSEvent.mouseEvent(with: .leftMouseDown, location: NSPoint(x: 120, y: 60), modifierFlags: [], timestamp: 0, windowNumber: panel.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!)
    expect(row.grabbing, "pressing the row grabs (closed hand)")
    row.dragEnded(at: NSPoint(x: 10, y: 10))
    expect(!row.grabbing && !row.dragging && helper.panel === panel && revealed.isEmpty, "a drag that ends without a grant releases the press (open hand over the row again) and reveals nothing")
    expect(content.appRow.accessibilityPerformPress() && revealed == [bundle], "a click on T3 Code reveals the running app bundle in Finder")
    expect(helper.panel === panel, "revealing in Finder keeps the helper open")
    let mask = T3PermissionHelperAppRow.operations(.outsideApplication)
    expect(mask.contains(.copy) && !mask.contains(.move) && !mask.contains(.delete) && T3PermissionHelperAppRow.operations(.withinApplication).isEmpty, "the drag offers the bundle to copy or link, never to move or trash")
    expect((bundle as NSURL).writableTypes(for: NSPasteboard(name: .drag)).contains(.fileURL), "the drag carries the bundle as a file URL")

    // Every way out (reference finish): the grant, the close button, Escape, Settings closing.
    _ = content.closeButton.accessibilityPerformPress()
    expect(helper.panel == nil && !panel.isVisible && returned == 1 && finished == 1 && stops == 1, "close returns to the owner window and releases the tracking")
    open(.accessibility); feed?(.window(main, frontmost: true))
    expect(helper.panel?.title == "Set up Accessibility", "Accessibility has its own helper")
    let escape = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: 0, context: nil, characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53)!
    helper.panel?.keyDown(with: escape)
    expect(helper.panel == nil && returned == 2, "Escape closes it")
    open(); feed?(.window(main, frontmost: true)); feed?(.absent)
    expect(helper.panel == nil && returned == 3, "Settings closing closes the helper and returns to the owner")
    open(); let first = helper.panel; open()
    expect(first?.isVisible == false && helper.panel !== first && stops == 4, "only one helper at a time; a new one releases the old")
    granted = true; spin(1.2)
    expect(helper.panel == nil && returned == 4 && finished == 4, "the grant is detected within a second and closes the helper")
    granted = false; open(); owner.close()
    expect(helper.panel == nil && returned == 4 && finished == 5, "closing the owner window closes the helper without reopening it")
    helper.close()

    if let directory = ProcessInfo.processInfo.environment["T3_PERMISSION_HELPER_EVIDENCE"] {
        for (dark, hover) in [(false, false), (false, true), (true, false), (true, true)] {
            let appIcon = NSImage(contentsOfFile: (ProcessInfo.processInfo.environment["T3_APP_DIR"] ?? "") + "/assets/icon.png") ?? NSApp.applicationIconImage!
            let view = T3PermissionHelperView(frame: NSRect(x: 0, y: 0, width: 560, height: 140), bundle: bundle, icon: appIcon)
            view.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
            if hover { view.mouseEntered(with: enter) }
            view.layoutSubtreeIfNeeded()
            let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds)!
            view.cacheDisplay(in: view.bounds, to: rep)
            try? rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: "\(directory)/helper-\(dark ? "dark" : "light")\(hover ? "-hover" : "").png"))
        }
    }
    print("\(passed) permission helper placement, tracking, drag, reveal and close checks passed (no grant requested)")
}
