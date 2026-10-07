#if os(macOS)
import AppKit
import UniformTypeIdentifiers

/// The reference desktop menu (apps/desktop/src/window/DesktopApplicationMenu.ts) laid over
/// the host's: Edit gains Paste as Text (⇧⌘V; the host files a paste variant after Paste and
/// gives Edit its Speech since exact2 #226, but an app item is a Contract button's press, and
/// this one is the responder chain's `pasteAsPlainText:` as the reference's pasteAndMatchStyle
/// is: an app-declared role item waits on #141), View gains Actual Size, Zoom In
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
        // The client pushes confirmQuit with devicePresentation, so the read answers at once.
        quit.getMode = { [weak self] done in done(self?.quitMode ?? "hold") }
        quit.notify = { [weak self] down, mode in
            guard let self, let window = self.window else { return }
            if down { self.overlay.show(mode: mode, over: window) } else { self.overlay.release() }
        }
        // The window stays key and only turns transparent, so the held keys' repeats cannot reach the next app.
        quit.conceal = { [weak self] in
            T3Menus.concealPendingQuit(self?.window ?? NSApp.keyWindow)
            self?.overlay.hide()
        }
        quit.quit = { NSApp.terminate(nil) }
    }

    func install(_ element: ExactElement) {
        guard element.hatch == .t3Composer, let window = element.view?.window else { return }
        attach(window)
        augment(NSApp.mainMenu)
        if monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .flagsChanged]) { [weak self] event in
                guard let self else { return event }
                if self.quit.handle(event) { return nil }
                return Self.dropsHeldClose(event) ? nil : event
            }
        }
    }

    /// DesktopWindow.ts: a held ⌘W can outlive the panel that took its first press, so its
    /// auto-repeats (no ⌥, no ⇧) never reach the menu's Close Window. Matched by key code 13 as
    /// well, since a non-Latin source (Korean 2-Set) reports ㅈ there: this monitor reads the raw
    /// event, which exact2 #168's physical-key matching of declared chords does not reach (X15).
    static func dropsHeldClose(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard event.type == .keyDown, event.isARepeat, flags.contains(.command), !flags.contains(.option), !flags.contains(.shift) else { return false }
        return event.keyCode == 13 || (event.charactersIgnoringModifiers ?? "").lowercased() == "w"
    }

    /// concealPendingQuitWindow: leave full screen first, then turn the window transparent.
    static func concealPendingQuit(_ window: T3ConcealableWindow?) {
        guard let window, window.isConcealable else { return }
        if window.isFullScreenWindow { window.leaveFullScreen() }
        window.alphaValue = 0
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
            // Speech is the host's own Edit ▸ Speech (exact2 #226).
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

    /// WORKSPACE_IMAGE_PREVIEW_EXTENSIONS (packages/shared/src/filePreview.ts), without the dot.
    static let faviconExtensions = ["avif", "gif", "ico", "jpeg", "jpg", "png", "svg", "webp"]

    /// pickProjectFavicon (apps/desktop/src/ipc/methods/window.ts): one image file, starting in the
    /// project's workspace root, as a sheet on the main window. The panel is built by `faviconPanel`
    /// so the tests can read its configuration.
    static func faviconPanel(startingAt path: String) -> NSOpenPanel {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = faviconExtensions.compactMap { UTType(filenameExtension: $0) }
        var isDirectory: ObjCBool = false
        let expanded = (path as NSString).expandingTildeInPath
        if !expanded.isEmpty, FileManager.default.fileExists(atPath: expanded, isDirectory: &isDirectory) {
            panel.directoryURL = URL(fileURLWithPath: isDirectory.boolValue ? expanded : (expanded as NSString).deletingLastPathComponent, isDirectory: true)
        }
        return panel
    }
    /// The picked file's absolute path, or nil on cancel. Under the agent no panel opens: the
    /// isolated data root's imports/ gives its first image file, so a drive can pick without one.
    func pickProjectFavicon(startingAt path: String, importsRoot: URL?, completion: @escaping (String?) -> Void) {
        if let importsRoot {
            let files = (try? FileManager.default.contentsOfDirectory(at: importsRoot, includingPropertiesForKeys: nil)) ?? []
            return completion(files.filter { Self.faviconExtensions.contains($0.pathExtension.lowercased()) }.map(\.path).sorted().first)
        }
        let panel = Self.faviconPanel(startingAt: path)
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

/// What concealPendingQuit needs of a window (the AppKit tests pass a stand-in).
protocol T3ConcealableWindow: AnyObject {
    var isConcealable: Bool { get }
    var isFullScreenWindow: Bool { get }
    func leaveFullScreen()
    var alphaValue: CGFloat { get set }
}
extension NSWindow: T3ConcealableWindow {
    var isConcealable: Bool { isVisible || isMiniaturized }
    var isFullScreenWindow: Bool { styleMask.contains(.fullScreen) }
    func leaveFullScreen() { toggleFullScreen(nil) }
}

/// QuitHold.ts (apps/desktop/src/window/QuitHold.ts, 1e2ecbd975; MIT, see LICENSE-T3): the quit
/// accelerator waits for a hold (proven by auto-repeat) or a second press. The mode is read when
/// the key goes down and may answer later (`getMode`, nil for a failed read, which quits at once);
/// a press superseded before its read settles discards that read. Timers and the clock are
/// injectable for the AppKit tests.
final class T3QuitHold {
    static let holdDuration: TimeInterval = 1.2
    static let doublePress: TimeInterval = 0.5
    static let releaseGrace: TimeInterval = 0.6
    /// A slow repeat rate can exceed the fixed grace: wait two observed cadences.
    static let repeatCadenceMultiplier: TimeInterval = 2
    /// confirmQuit for this press: "direct", "hold" or "double-click"; nil when the read failed.
    var getMode: (@escaping (String?) -> Void) -> Void = { $0("hold") }
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
    /// Set once the mode read answers hold; auto-repeats may only complete the hold when armed.
    private var armed = false
    private var quitOnRelease = false
    private var heldSince: TimeInterval = 0
    private var lastPressAt: TimeInterval = 0
    private var lastRepeatAt: TimeInterval = 0
    private var repeatCadence: TimeInterval = 0
    /// Bumped when a press is superseded or cancelled; a plain key release keeps its pending read.
    private var generation = 0
    private var flags: NSEvent.ModifierFlags = []

    /// One keyboard event of this app; true when the quit shortcut consumed it. A modifier going
    /// down arrives as its own keydown, as Electron's before-input-event reports it.
    func handle(_ event: NSEvent) -> Bool {
        let now = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let command = now.contains(.command), option = now.contains(.option) || now.contains(.control), shift = now.contains(.shift)
        switch event.type {
        case .flagsChanged:
            let before = flags
            flags = now
            // A modifier event is never consumed: AppKit keeps its own modifier state from it.
            if command && !before.contains(.command) { _ = keyDown(key: "meta", command: true, option: option, shift: shift, isRepeat: false) }
            else if !command && before.contains(.command) { keyUp(key: "meta") }
            else if let pressed = [(NSEvent.ModifierFlags.shift, "shift"), (.option, "alt"), (.control, "control")].first(where: { now.contains($0.0) && !before.contains($0.0) }) {
                _ = keyDown(key: pressed.1, command: command, option: option, shift: shift, isRepeat: false)
            }
            return false
        case .keyDown:
            return keyDown(key: Self.key(event), command: command, option: option, shift: shift, isRepeat: event.isARepeat)
        case .keyUp:
            keyUp(key: Self.key(event))
            return false
        default: return false
        }
    }
    /// The event's key; Q's key code stands in when a non-Latin source (Korean 2-Set's ㅂ) types no
    /// Latin letter there, as the r10-connect chords do: the quit hold reads raw key events, which
    /// exact2 #168 (declared chords only) does not reach (X15).
    static func key(_ event: NSEvent) -> String {
        let key = (event.charactersIgnoringModifiers ?? "").lowercased()
        return event.keyCode == 12 && !key.unicodeScalars.contains(where: { $0.isASCII }) ? "q" : key
    }

    func keyDown(key: String, command: Bool, option: Bool, shift: Bool, isRepeat: Bool) -> Bool {
        if isRepeat && command && key == "q" {
            let time = now()
            repeatCadence = time - (lastRepeatAt == 0 ? heldSince : lastRepeatAt)
            lastRepeatAt = time
        }
        if quitOnRelease {
            // A Q keydown proves the key is still down whether or not ⌘ is; it only pushes the quiet period back.
            if key == "q" { quitAfterQuietPeriod() }
            return true
        }
        if !command || option || shift || key != "q" {
            // Re-pressing ⌘ is the first half of a second quit shortcut.
            if key == "meta" && !option && !shift { return false }
            // Other keys cancel the hold and the first tap, even after release.
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
        // A fresh keydown supersedes the current hold or the hint kept after a detected release.
        if holding || notified { release() }
        generation += 1
        // Every mode accepts two presses, before the mode read can delay the second.
        if previous != 0 && time - previous <= Self.doublePress { quitNow(); return true }
        let press = generation
        holding = true
        heldSince = time
        getMode { [weak self] resolved in self?.resolve(resolved, press: press, pressedAt: time) }
        return true
    }

    private func resolve(_ resolved: String?, press: Int, pressedAt: TimeInterval) {
        guard generation == press else { return }
        // A failed settings read must never strand the quit request.
        guard let resolved else { quitNow(); return }
        if resolved == "direct" { quitNow(); return }
        if resolved == "double-click" {
            let remaining = Self.doublePress - (now() - pressedAt)
            if remaining <= 0 { release(); return }
            current = resolved
            notified = true
            notify(true, resolved)
            watch(remaining) { [weak self] in self?.release() }
            return
        }
        // A hold cannot be armed after its physical press has ended.
        guard holding else { return }
        current = resolved
        notified = true
        notify(true, resolved)
        armed = true
        // No auto-repeat by then means the key was released (or repeat is off): don't quit.
        watch(Self.holdDuration + Self.releaseGrace) { [weak self] in self?.cancelWatchdog = nil; self?.release() }
    }

    func keyUp(key: String) {
        if key == "q" {
            let shouldQuit = quitOnRelease
            release(cancelPendingMode: false, keepDoublePressHint: true)
            if shouldQuit { quit() }
        } else if key == "meta" {
            if !quitOnRelease { release(cancelPendingMode: false, keepDoublePressHint: true) } else { quitAfterQuietPeriod() }
        }
    }

    func reset() { lastPressAt = 0; release() }

    private func watch(_ delay: TimeInterval, _ work: @escaping () -> Void) {
        cancelWatchdog?()
        cancelWatchdog = schedule(delay, work)
    }
    private func clearWatchdog() { cancelWatchdog?(); cancelWatchdog = nil }
    private func release(cancelPendingMode: Bool = true, keepDoublePressHint: Bool = false) {
        if cancelPendingMode { generation += 1 }
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
    /// Dismisses any hint first so a cancelled quit cannot leave a stale one.
    private func quitNow() { release(); lastPressAt = 0; quit() }
    private func quitAfterQuietPeriod() {
        clearWatchdog()
        watch(max(Self.releaseGrace, repeatCadence * Self.repeatCadenceMultiplier)) { [weak self] in self?.quitNow() }
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
        label.font = .systemFont(ofSize: T3RootFont.rem(24), weight: .bold)
        label.textColor = background
        label.sizeToFit()
        let size = NSSize(width: ceil(label.frame.width) + T3RootFont.rem(64), height: T3RootFont.rem(64)) // px-8 py-4 around text-2xl
        pill.frame = NSRect(origin: .zero, size: size)
        pill.wantsLayer = true
        pill.layer?.backgroundColor = foreground.cgColor
        pill.layer?.cornerRadius = size.height / 2
        pill.layer?.shadowColor = NSColor.black.cgColor
        pill.layer?.shadowOpacity = 0.1
        pill.layer?.shadowRadius = 12
        pill.layer?.shadowOffset = CGSize(width: 0, height: -20)
        label.frame = NSRect(x: T3RootFont.rem(32), y: (size.height - label.frame.height) / 2, width: ceil(label.frame.width), height: label.frame.height)
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
