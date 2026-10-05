#if os(macOS)
import AppKit

/// The reference desktop menu (apps/desktop/src/window/DesktopApplicationMenu.ts) laid over
/// the host's: Edit gains Paste as Text (⇧⌘V) and Speech, View gains Actual Size, Zoom In
/// and Zoom Out (the main window's zoom, Electron's 0.5-level steps). Settings… (⌘,) is the
/// host's placement of the app's Meta+, command. ⌘Q from the keyboard goes through the
/// Quit shortcut setting (QuitHold.ts); Quit from the menu stays immediate. Check for
/// Updates... sits under About and in Help, as the reference's darwin template places it.
final class T3Menus: NSObject, NSMenuItemValidation, NSMenuDelegate {
    private weak var window: NSWindow?
    private var monitor: Any?
    private var frameObserver: NSObjectProtocol?
    private(set) var zoomLevel: Double = 0
    let quit = T3QuitHold()
    private lazy var overlay = T3QuitOverlay()
    /// confirmQuit from the client settings ("direct", "hold" or "double-click").
    var quitMode = "hold"
    /// Why this build cannot check for updates, or nil when it links an update store.
    var updatesDisabledReason: () -> String? = { T3Menus.updatesDisabledReason() }
    /// How the "Updates unavailable" box is shown (the tests record it instead).
    var presentAlert: (NSAlert) -> Void = { _ = $0.runModal() }
    private lazy var appUpdatesItem = makeUpdatesItem()
    private lazy var helpUpdatesItem = makeUpdatesItem()
    /// Lane r8-keys: File, View, Edit › Undo and the host's extra menus as the reference bar has them (R8KeysMenus.swift).
    let keys = R8KeysMenus()

    override init() {
        super.init()
        quit.mode = { [weak self] in self?.quitMode ?? "hold" }
        quit.notify = { [weak self] down, mode in
            guard let self, let window = self.window else { return }
            if down { self.overlay.show(mode: mode, over: window) } else { self.overlay.release() }
        }
        quit.conceal = { NSApp.windows.forEach { $0.orderOut(nil) } }
        quit.quit = { NSApp.terminate(nil) }
    }

    func install(_ element: ExactElement) {
        guard element.hook == .t3Composer, let window = element.view?.window else { return }
        attach(window)
        augment(NSApp.mainMenu)
        if monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .flagsChanged]) { [weak self] event in
                guard let self else { return event }
                return self.quit.handle(event) ? nil : event
            }
        }
    }

    /// The main window whose content zooms and over which the quit hint shows.
    func attach(_ window: NSWindow) {
        guard self.window !== window else { return }
        self.window = window
        if let frameObserver { NotificationCenter.default.removeObserver(frameObserver) }
        window.contentView?.postsFrameChangedNotifications = true
        frameObserver = NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: window.contentView, queue: .main) { [weak self] _ in
            self?.applyZoom()
        }
    }

    /// Adds the reference items the host's menu lacks; safe to repeat after the host rebuilds it.
    func augment(_ bar: NSMenu?) {
        guard let bar else { return }
        if let edit = bar.items.first(where: { $0.submenu?.title == "Edit" })?.submenu, edit.item(withTitle: "Paste as Text") == nil {
            let paste = edit.indexOfItem(withTitle: "Paste")
            let item = NSMenuItem(title: "Paste as Text", action: #selector(NSTextView.pasteAsPlainText(_:)), keyEquivalent: "v")
            item.keyEquivalentModifierMask = [.command, .shift]
            edit.insertItem(item, at: paste >= 0 ? paste + 1 : edit.numberOfItems)
            let selectAll = edit.indexOfItem(withTitle: "Select All")
            if selectAll > 0, edit.item(at: selectAll - 1)?.isSeparatorItem == false { edit.insertItem(.separator(), at: selectAll) }
            edit.addItem(.separator())
            let speech = NSMenu(title: "Speech")
            speech.addItem(withTitle: "Start Speaking", action: #selector(NSTextView.startSpeaking(_:)), keyEquivalent: "")
            speech.addItem(withTitle: "Stop Speaking", action: #selector(NSTextView.stopSpeaking(_:)), keyEquivalent: "")
            edit.addItem(withTitle: "Speech", action: nil, keyEquivalent: "").submenu = speech
        }
        if let view = bar.items.first(where: { $0.submenu?.title == "View" })?.submenu, view.item(withTitle: "Actual Size") == nil {
            let items: [(String, Selector, String, Bool)] = [("Actual Size", #selector(actualSize(_:)), "0", false), ("Zoom In", #selector(zoomIn(_:)), "=", false),
                                                            ("Zoom In", #selector(zoomIn(_:)), "+", true), ("Zoom Out", #selector(zoomOut(_:)), "-", false)]
            for (index, (title, action, key, hidden)) in items.enumerated() {
                let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
                item.target = self
                item.isHidden = hidden
                item.allowsKeyEquivalentWhenHidden = hidden
                view.insertItem(item, at: index)
            }
            view.insertItem(.separator(), at: items.count)
        }
        augmentUpdates(bar)
        keys.arrange(bar)
    }

    // MARK: Check for Updates...

    /// DesktopUpdates getAutoUpdateDisabledReason: a bundle without an update store (its
    /// receipt's composition "embedded", app.json deploy.store "0") has no update feed.
    static func updatesDisabledReason(receipt: URL? = Bundle.main.url(forResource: "receipt", withExtension: "json")) -> String? {
        let unavailable = "Automatic updates are not available because no update feed is configured."
        guard let receipt, let data = try? Data(contentsOf: receipt),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return unavailable }
        return json["composition"] as? String == "updating" ? nil : unavailable
    }
    /// handleCheckForUpdatesMenuClick's disabled branch: an informational box, no parent window.
    static func updatesUnavailableAlert(_ reason: String) -> NSAlert {
        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = "Automatic updates are not available right now."
        alert.informativeText = reason
        alert.addButton(withTitle: "OK")
        return alert
    }
    private func makeUpdatesItem() -> NSMenuItem {
        let item = NSMenuItem(title: "Check for Updates...", action: #selector(checkForUpdates(_:)), keyEquivalent: "")
        item.target = self
        return item
    }
    @objc func checkForUpdates(_ sender: Any?) {
        // A build with an update store would run the host's check; this module has no hook into it.
        guard let reason = updatesDisabledReason() else { return }
        presentAlert(Self.updatesUnavailableAlert(reason))
    }
    /// The app menu's item under About, and a Help menu after Window holding the other.
    /// A build with an update store keeps neither: the module cannot run the host's check.
    private func augmentUpdates(_ bar: NSMenu) {
        guard updatesDisabledReason() != nil else { return }
        if let app = bar.items.first?.submenu, app.items.first?.action == #selector(NSApplication.orderFrontStandardAboutPanel(_:)), app.index(of: appUpdatesItem) < 0 {
            appUpdatesItem.menu?.removeItem(appUpdatesItem)
            app.insertItem(appUpdatesItem, at: min(1, app.numberOfItems))
            // The host places Settings… at index 2 on each plan batch; order the menu as it opens.
            if app.delegate == nil || app.delegate === self { app.delegate = self }
            Self.orderApplicationMenu(app, updates: appUpdatesItem)
        }
        if bar.items.first(where: { $0.submenu?.title == "Help" }) == nil {
            helpUpdatesItem.menu?.removeItem(helpUpdatesItem)
            let help = NSMenu(title: "Help")
            help.addItem(helpUpdatesItem)
            let item = NSMenuItem(title: "Help", action: nil, keyEquivalent: "")
            item.submenu = help
            let window = bar.items.firstIndex { $0.submenu?.title == "Window" }
            bar.insertItem(item, at: window.map { $0 + 1 } ?? bar.numberOfItems)
            if NSApp != nil { NSApp.helpMenu = help }
        }
    }
    func menuNeedsUpdate(_ menu: NSMenu) { Self.orderApplicationMenu(menu, updates: appUpdatesItem) }
    /// The reference's app menu: About, Check for Updates..., —, Settings..., —, then the
    /// host's Services / Hide / Quit groups with their own separators, none doubled.
    static func orderApplicationMenu(_ menu: NSMenu, updates: NSMenuItem) {
        let items = menu.items
        guard let about = items.first, about !== updates, items.contains(where: { $0 === updates }) else { return }
        let settings = items.first { $0.keyEquivalent == "," && $0.keyEquivalentModifierMask.intersection(.deviceIndependentFlagsMask) == [.command] }
        var rest = items.filter { $0 !== about && $0 !== updates && $0 !== settings }
        var spare: [NSMenuItem] = []
        while let first = rest.first, first.isSeparatorItem { spare.append(rest.removeFirst()) }
        var tail: [NSMenuItem] = []
        for item in rest {
            if item.isSeparatorItem, tail.last?.isSeparatorItem ?? true { spare.append(item); continue }
            tail.append(item)
        }
        while let last = tail.last, last.isSeparatorItem { spare.append(tail.removeLast()) }
        let separator = { spare.isEmpty ? NSMenuItem.separator() : spare.removeFirst() }
        var ordered = [about, updates, separator()]
        if let settings { ordered += [settings, separator()] }
        ordered += tail
        if ordered.count == items.count, zip(ordered, items).allSatisfy({ $0 === $1 }) { return }
        menu.removeAllItems()
        for item in ordered { menu.addItem(item) }
    }

    func validateMenuItem(_ item: NSMenuItem) -> Bool { item.action == #selector(checkForUpdates(_:)) || window != nil }
    @objc func actualSize(_ sender: Any?) { setZoom(0) }
    @objc func zoomIn(_ sender: Any?) { setZoom(zoomLevel + 0.5) }
    @objc func zoomOut(_ sender: Any?) { setZoom(zoomLevel - 0.5) }

    /// Chromium's zoom: a factor of 1.2^level, kept within its 25%–500% range.
    func setZoom(_ level: Double) {
        let minimum = log(0.25) / log(1.2), maximum = log(5.0) / log(1.2)
        zoomLevel = min(maximum, max(minimum, level))
        applyZoom()
    }
    /// The main window's content draws at the zoom factor: its bounds shrink so the page
    /// lays out in a viewport of `frame / factor` CSS points, as a zoomed browser page does.
    func applyZoom() {
        guard let content = window?.contentView else { return }
        let factor = pow(1.2, zoomLevel)
        let target = NSSize(width: content.frame.width / factor, height: content.frame.height / factor)
        let old = content.bounds.size
        guard abs(old.width - target.width) > 0.01 || abs(old.height - target.height) > 0.01 else { return }
        content.setBoundsSize(target)
        content.setBoundsOrigin(.zero)
        // Autoresizing follows frame changes, not bounds: carry the flexible subviews
        // (the page's viewport) to the new bounds so the page lays out zoomed.
        let dw = target.width - old.width, dh = target.height - old.height
        for sub in content.subviews where sub.autoresizesSubviews || !sub.autoresizingMask.isEmpty {
            var frame = sub.frame
            if sub.autoresizingMask.contains(.width) { frame.size.width += dw }
            if sub.autoresizingMask.contains(.height) { frame.size.height += dh }
            if sub.autoresizingMask.contains(.minYMargin) && !sub.autoresizingMask.contains(.height) { frame.origin.y += dh }
            sub.frame = frame
        }
        content.needsLayout = true
        content.layoutSubtreeIfNeeded()
        content.needsDisplay = true
        if let window { T3WindowChrome.positionControls(in: window) }
    }

    /// Open in Finder: the native folder picker, starting where the palette is browsing.
    func pickFolder(startingAt path: String, completion: @escaping (String?) -> Void) {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = true
        panel.prompt = "Add Project"
        let expanded = (path as NSString).expandingTildeInPath
        var isDirectory: ObjCBool = false
        if !expanded.isEmpty, FileManager.default.fileExists(atPath: expanded, isDirectory: &isDirectory) {
            panel.directoryURL = URL(fileURLWithPath: isDirectory.boolValue ? expanded : (expanded as NSString).deletingLastPathComponent)
        }
        let finish: (NSApplication.ModalResponse) -> Void = { response in completion(response == .OK ? panel.url?.path : nil) }
        if let window { panel.beginSheetModal(for: window, completionHandler: finish) } else { finish(panel.runModal()) }
    }

    func destroy() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        if let frameObserver { NotificationCenter.default.removeObserver(frameObserver) }
        frameObserver = nil
        quit.reset()
        overlay.hide()
    }
    deinit { destroy() }
}

/// QuitHold.ts: the quit accelerator waits for a hold (proven by auto-repeat) or a second
/// press. Timers and the clock are injectable for the AppKit tests.
final class T3QuitHold {
    static let holdDuration: TimeInterval = 1.2
    static let doublePress: TimeInterval = 0.5
    static let releaseGrace: TimeInterval = 0.6
    var mode: () -> String = { "hold" }
    var notify: (Bool, String) -> Void = { _, _ in }
    var conceal: () -> Void = {}
    var quit: () -> Void = {}
    var now: () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    var schedule: (TimeInterval, @escaping () -> Void) -> () -> Void = { delay, work in
        let item = DispatchWorkItem(block: work)
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: item)
        return { item.cancel() }
    }
    private var cancelWatchdog: (() -> Void)?
    private var holding = false
    private var current = ""
    private(set) var notified = false
    private var armed = false
    private var quitOnRelease = false
    private var heldSince: TimeInterval = 0
    private var lastPressAt: TimeInterval = 0
    private var lastRepeatAt: TimeInterval = 0
    private var repeatCadence: TimeInterval = 0
    private var commandDown = false

    /// One keyboard event of this app; true when the quit shortcut consumed it.
    func handle(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        switch event.type {
        case .flagsChanged:
            let down = flags.contains(.command)
            defer { commandDown = down }
            if down && !commandDown { return keyDown(key: "meta", command: true, option: flags.contains(.option), shift: flags.contains(.shift), isRepeat: false) }
            if !down && commandDown { keyUp(key: "meta") }
            return false
        case .keyDown:
            let key = (event.charactersIgnoringModifiers ?? "").lowercased()
            return keyDown(key: key, command: flags.contains(.command), option: flags.contains(.option) || flags.contains(.control), shift: flags.contains(.shift), isRepeat: event.isARepeat)
        case .keyUp:
            keyUp(key: (event.charactersIgnoringModifiers ?? "").lowercased())
            return false
        default: return false
        }
    }

    func keyDown(key: String, command: Bool, option: Bool, shift: Bool, isRepeat: Bool) -> Bool {
        if isRepeat && command && key == "q" {
            let time = now()
            repeatCadence = time - (lastRepeatAt == 0 ? heldSince : lastRepeatAt)
            lastRepeatAt = time
        }
        if quitOnRelease {
            // A Q keydown proves the key is still down; it only pushes the quiet period back.
            if key == "q" { quitAfterQuietPeriod() }
            return key == "q"
        }
        if !command || option || shift || key != "q" {
            // Re-pressing ⌘ is the first half of a second quit shortcut.
            if key == "meta" && !option && !shift { return false }
            if !isRepeat { lastPressAt = 0; release() }
            return false
        }
        if isRepeat {
            if current == "hold" && armed && now() - heldSince >= Self.holdDuration {
                armed = false
                quitOnRelease = true
                conceal()
                quitAfterQuietPeriod()
            }
            return true
        }
        let time = now(), previous = lastPressAt
        lastPressAt = time
        if holding || notified { release() }
        // Every mode accepts two presses.
        if previous != 0 && time - previous <= Self.doublePress { quitNow(); return true }
        holding = true
        heldSince = time
        let resolved = mode()
        if resolved == "direct" { quitNow(); return true }
        if resolved == "double-click" {
            current = resolved
            notified = true
            notify(true, resolved)
            watch(Self.doublePress) { [weak self] in self?.release() }
            return true
        }
        current = "hold"
        notified = true
        notify(true, "hold")
        armed = true
        // No auto-repeat by then means the key was released (or repeat is off): don't quit.
        watch(Self.holdDuration + Self.releaseGrace) { [weak self] in self?.cancelWatchdog = nil; self?.release() }
        return true
    }

    func keyUp(key: String) {
        if key == "q" {
            let shouldQuit = quitOnRelease
            release(keepDoublePressHint: true)
            if shouldQuit { quit() }
        } else if key == "meta" {
            if !quitOnRelease { release(keepDoublePressHint: true) } else { quitAfterQuietPeriod() }
        }
    }

    func reset() { lastPressAt = 0; release() }

    private func watch(_ delay: TimeInterval, _ work: @escaping () -> Void) {
        cancelWatchdog?()
        cancelWatchdog = schedule(delay, work)
    }
    private func clearWatchdog() { cancelWatchdog?(); cancelWatchdog = nil }
    private func release(keepDoublePressHint: Bool = false) {
        if !holding && !notified { return }
        let keepHint = keepDoublePressHint && current == "double-click" && notified
        holding = false
        armed = false
        quitOnRelease = false
        lastRepeatAt = 0
        repeatCadence = 0
        if keepHint { return }
        current = ""
        clearWatchdog()
        if notified { notified = false; notify(false, "") }
    }
    private func quitNow() { release(); lastPressAt = 0; quit() }
    private func quitAfterQuietPeriod() {
        clearWatchdog()
        // A slow repeat rate can exceed the fixed grace: wait two observed cadences.
        watch(max(Self.releaseGrace, repeatCadence * 2)) { [weak self] in self?.quitNow() }
    }
}

/// QuitHoldOverlay.tsx: a centred pill 22% from the top of the window, foreground/95 with
/// the background colour's bold 24px text; a released hold hint lingers 1.2s.
final class T3QuitOverlay {
    private var panel: NSPanel?
    private let label = NSTextField(labelWithString: "")
    private let pill = NSView()
    private var pressedMode = "hold"
    private var hideWork: DispatchWorkItem?
    static func message(_ mode: String) -> String { mode == "hold" ? "Hold ⌘Q or press twice to quit" : "Press ⌘Q again to quit" }

    /// The pill view for a mode and appearance (also what the tests render).
    func makePill(mode: String, dark: Bool) -> NSView {
        let foreground = dark ? NSColor(srgbRed: 0xf5 / 255, green: 0xf5 / 255, blue: 0xf5 / 255, alpha: 0.95) : NSColor(srgbRed: 0x27 / 255, green: 0x27 / 255, blue: 0x2a / 255, alpha: 0.95)
        let background = dark ? NSColor(srgbRed: 0x0a / 255, green: 0x0a / 255, blue: 0x0a / 255, alpha: 1) : NSColor(srgbRed: 0xfc / 255, green: 0xfc / 255, blue: 0xfc / 255, alpha: 1)
        label.stringValue = Self.message(mode)
        label.font = .systemFont(ofSize: 24, weight: .bold)
        label.textColor = background
        label.sizeToFit()
        let size = NSSize(width: ceil(label.frame.width) + 64, height: 64)
        pill.frame = NSRect(origin: .zero, size: size)
        pill.wantsLayer = true
        pill.layer?.backgroundColor = foreground.cgColor
        pill.layer?.cornerRadius = size.height / 2
        pill.layer?.shadowColor = NSColor.black.cgColor
        pill.layer?.shadowOpacity = 0.1
        pill.layer?.shadowRadius = 12
        pill.layer?.shadowOffset = CGSize(width: 0, height: -20)
        label.frame = NSRect(x: 32, y: (size.height - label.frame.height) / 2, width: ceil(label.frame.width), height: label.frame.height)
        if label.superview !== pill { pill.addSubview(label) }
        return pill
    }

    func show(mode: String, over window: NSWindow) {
        hideWork?.cancel()
        pressedMode = mode
        let dark = window.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        let view = makePill(mode: mode, dark: dark)
        let content = window.contentRect(forFrameRect: window.frame)
        let size = NSSize(width: view.frame.width + 48, height: view.frame.height + 48)
        let origin = NSPoint(x: content.midX - size.width / 2, y: content.maxY - content.height * 0.22 - view.frame.height - 24)
        let panel = self.panel ?? NSPanel(contentRect: NSRect(origin: origin, size: size), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.ignoresMouseEvents = true
        panel.level = .floating
        panel.setFrame(NSRect(origin: origin, size: size), display: false)
        let host = panel.contentView ?? NSView()
        if view.superview !== host { host.addSubview(view) }
        view.setFrameOrigin(NSPoint(x: 24, y: 24))
        panel.setAccessibilityRole(.staticText)
        panel.setAccessibilityLabel(Self.message(mode))
        if panel.parent !== window { window.addChildWindow(panel, ordered: .above) }
        panel.orderFront(nil)
        self.panel = panel
    }
    func release() {
        hideWork?.cancel()
        if pressedMode == "double-click" { hide(); return }
        let work = DispatchWorkItem { [weak self] in self?.hide() }
        hideWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + T3QuitHold.holdDuration, execute: work)
    }
    func hide() {
        hideWork?.cancel()
        guard let panel else { return }
        panel.parent?.removeChildWindow(panel)
        panel.orderOut(nil)
    }
    var visible: Bool { panel?.isVisible == true }
}
#endif
