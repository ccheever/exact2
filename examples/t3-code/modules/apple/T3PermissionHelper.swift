// Reference MacPermissionHelper and MacSettingsWindow (apps/desktop/src/permissions):
// while the person grants a macOS privacy permission, a small panel docks inside
// System Settings' content column with "↑ Drag T3 Code into the list above" and the
// app as a drag source (a click reveals it in Finder). It follows the Settings
// window, hides while Settings is covered, and closes when the grant is detected or
// Settings closes. Tracking reads window metadata only; it never requests a grant.
import AppKit

enum T3MacPermission: String {
    case screenRecording = "screen-recording", accessibility
    /// Reference MAC_PERMISSION_TITLES.
    var title: String { self == .screenRecording ? "Screen Recording" : "Accessibility" }
    /// Reference MAC_PERMISSION_SETTINGS_URLS.
    var settingsURL: URL { URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_" + (self == .screenRecording ? "ScreenCapture" : "Accessibility"))! }
}

/// What one look at System Settings found (reference SettingsWindow, plus the
/// watcher's "unavailable"). Frames are CG global coordinates, top-left origin.
enum T3SettingsReading: Equatable {
    case unavailable, absent
    case window(CGRect, frontmost: Bool)
}

enum T3SettingsWindow {
    static let bundleId = "com.apple.systempreferences"
    /// Reference SETTINGS_WINDOW_SCRIPT: Settings' first on-screen layer-0 window of
    /// at least 500×350, by bundle identifier rather than a localized title.
    static func find(in windows: [[String: Any]], pid: pid_t, frontmost: pid_t?) -> T3SettingsReading {
        for window in windows {
            guard (window[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == pid, (window[kCGWindowLayer as String] as? NSNumber)?.intValue == 0,
                  let bounds = window[kCGWindowBounds as String] as? [String: Any], let frame = CGRect(dictionaryRepresentation: bounds as CFDictionary),
                  frame.width >= 500, frame.height >= 350 else { continue }
            return .window(frame, frontmost: frontmost == pid)
        }
        return .absent
    }
    static func read() -> T3SettingsReading {
        guard let app = NSRunningApplication.runningApplications(withBundleIdentifier: bundleId).first else { return .absent }
        guard let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else { return .unavailable }
        return find(in: windows, pid: app.processIdentifier, frontmost: NSWorkspace.shared.frontmostApplication?.processIdentifier)
    }
    /// Reference settingsHelperBounds: centred in Settings' content column (right of
    /// its 216 pt sidebar), 16 pt above the bottom edge. Rounds as Math.round does.
    static func helperBounds(_ settings: CGRect) -> CGRect {
        let sidebar: CGFloat = 216, inset: CGFloat = 16, round = { (value: CGFloat) in (value + 0.5).rounded(.down) }
        let width = min(560, settings.width - sidebar - inset * 2)
        return CGRect(x: round(settings.minX + sidebar + (settings.width - sidebar - width) / 2), y: round(settings.minY + settings.height - 140 - inset), width: round(width), height: 140)
    }
}

/// Reference watchMacSettingsWindow: one poll reporting only changes (the first
/// reading always), every 0.5 s while Settings is frontmost and every 1 s otherwise.
final class T3SettingsWindowWatcher {
    private let read: () -> T3SettingsReading
    private let changed: (T3SettingsReading) -> Void
    private var previous: T3SettingsReading?
    private var timer: Timer?
    private var stopped = false
    init(read: @escaping () -> T3SettingsReading = T3SettingsWindow.read, changed: @escaping (T3SettingsReading) -> Void) { self.read = read; self.changed = changed }
    func start() { tick() }
    func stop() { stopped = true; timer?.invalidate(); timer = nil }
    private func tick() {
        guard !stopped else { return }
        let reading = read()
        if reading != previous { previous = reading; changed(reading) }
        guard !stopped else { return }
        let frontmost: Bool = { if case .window(_, true) = reading { return true }; return false }()
        timer = Timer.scheduledTimer(withTimeInterval: frontmost ? 0.5 : 1, repeats: false) { [weak self] _ in self?.tick() }
    }
}

/// Reference shell.showItemInFolder, asked by the active app: the reference's helper is an ordinary window, so the click
/// that reveals finds T3 Code active (the window server activates an app on a click in its window; there the first
/// click only activates), and a reveal asked by the active app brings Finder's window to the front (the helper then
/// hides, System Settings being covered). The panel activates too (T3PermissionPanel), so a click reaches here with
/// T3 Code active, and T3 Code yields activation to Finder, asks for the reveal and asks Finder to activate. A request
/// to activate itself from an inactive app is refused under real input (realinput-1010c RC-1: two clicks left Finder
/// behind System Settings with T3 Code inactive), so it is only a fallback for a press that is no click (VoiceOver's
/// press). Nothing waits for Finder's window: the reveal reaches Finder as a request, and the activation request
/// follows it at once.
struct T3FinderReveal {
    static let finder = "com.apple.finder"
    // Seams for the AppKit test; the defaults are the real system.
    var isActive: () -> Bool = { NSApp.isActive }
    var activateSelf: () -> Void = { NSApp.activate(ignoringOtherApps: true) }
    var yield: (String) -> Void = { NSApp.yieldActivation(toApplicationWithBundleIdentifier: $0) }
    var select: (URL) -> Void = { NSWorkspace.shared.activateFileViewerSelecting([$0]) }
    var activate: (String) -> Void = { _ = NSRunningApplication.runningApplications(withBundleIdentifier: $0).first?.activate(options: []) }
    func reveal(_ url: URL) {
        if !isActive() { activateSelf() }
        yield(Self.finder)
        select(url)
        activate(Self.finder)
    }
}

/// Owns at most one helper panel; closing it releases its timer, watcher and observer.
final class T3PermissionHelper {
    private(set) var panel: T3PermissionPanel?
    private(set) var permission: T3MacPermission?
    private weak var owner: NSWindow?
    private var grantTimer: Timer?
    private var stopTracking: () -> Void = {}
    private var ownerObserver: NSObjectProtocol?
    private var settings: T3SettingsReading = .absent
    private var foundSettings = false
    // Seams for the AppKit tests; the defaults are the real system.
    var watch: (@escaping (T3SettingsReading) -> Void) -> () -> Void = { changed in
        let watcher = T3SettingsWindowWatcher(changed: changed); watcher.start(); return watcher.stop
    }
    var primaryScreenHeight: () -> CGFloat = { NSScreen.screens.first?.frame.maxY ?? 0 }
    var reveal: (URL) -> Void = { T3FinderReveal().reveal($0) }
    var returnTo: (NSWindow) -> Void = { owner in NSApp.activate(ignoringOtherApps: true); owner.makeKeyAndOrderFront(nil) }
    /// Every end of a helper (grant, close, Settings closed); the module refreshes its status.
    var finished: () -> Void = {}

    /// Reference macAppBundlePath: the outer app bundle of the executable, never the
    /// bare binary (a test runner has none, so it shows no helper).
    static func appBundle(executable: String) -> URL? {
        let pattern = try! NSRegularExpression(pattern: #"^(.+\.app)/Contents/MacOS/[^/]+$"#)
        guard let match = pattern.firstMatch(in: executable, range: NSRange(executable.startIndex..., in: executable)),
              let range = Range(match.range(at: 1), in: executable) else { return nil }
        return URL(fileURLWithPath: String(executable[range]), isDirectory: true)
    }

    func close() {
        grantTimer?.invalidate(); grantTimer = nil
        stopTracking(); stopTracking = {}
        if let ownerObserver { NotificationCenter.default.removeObserver(ownerObserver) }
        ownerObserver = nil
        panel?.onBlur = {}; panel?.orderOut(nil); panel?.close(); panel = nil; permission = nil
        settings = .absent; foundSettings = false
    }

    /// Reference MacPermissionHelper.show. Opens nothing for a permission already
    /// granted or outside an app bundle; the panel stays hidden until Settings shows.
    func show(_ permission: T3MacPermission, owner: NSWindow?, isGranted: @escaping () -> Bool, bundle: URL? = T3PermissionHelper.appBundle(executable: Bundle.main.executablePath ?? ""), icon: NSImage? = NSApp.applicationIconImage) {
        close()
        guard !isGranted(), let bundle, let icon, icon.isValid else { return }
        let panel = T3PermissionPanel(permission: permission, bundle: bundle, icon: icon)
        // Reference nativeTheme.themeSource: the helper takes the app's chosen appearance.
        panel.appearance = owner?.appearance
        self.panel = panel; self.permission = permission; self.owner = owner
        panel.content.onClose = { [weak self] in self?.finish() }
        panel.content.onReveal = { [weak self] in self?.reveal(bundle) }
        panel.onEscape = { [weak self] in self?.finish() }
        panel.onBlur = { [weak self] in self?.sync() }
        if let owner {
            ownerObserver = NotificationCenter.default.addObserver(forName: NSWindow.willCloseNotification, object: owner, queue: .main) { [weak self] _ in self?.close(); self?.finished() }
        }
        // Reference: TCC is polled once a second only while the helper is open.
        grantTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.check(isGranted) }
        stopTracking = watch { [weak self] reading in self?.settingsChanged(reading) }
    }
    func check(_ isGranted: () -> Bool) { if panel != nil, isGranted() { finish() } }
    func settingsChanged(_ reading: T3SettingsReading) {
        settings = reading
        if case .window = reading { foundSettings = true }
        sync()
    }
    private func sync() {
        guard let panel else { return }
        switch settings {
        case .unavailable: panel.orderOut(nil)
        case .absent: if foundSettings { finish() } else { panel.orderOut(nil) }
        case let .window(frame, frontmost):
            guard frontmost || panel.isKeyWindow else { panel.orderOut(nil); return }
            let target = T3SnapshotFeedback.screenFrame(T3SettingsWindow.helperBounds(frame), screenHeight: primaryScreenHeight())
            if panel.frame != target { panel.setFrame(target, display: true) }
            if !panel.isVisible { panel.orderFrontRegardless() }
        }
    }
    /// Reference finish: the helper closes and onboarding's window comes back.
    private func finish() {
        let owner = self.owner
        close()
        if let owner { returnTo(owner) }
        finished()
    }
}

/// Reference helper window (an ordinary focusable BrowserWindow): frameless, transparent,
/// shadowless, always on top, out of the window cycle. Showing it never activates T3 Code
/// (orderFrontRegardless, the reference's showInactive); a click in it does, as in any
/// window without .nonactivatingPanel. The row takes that activating click (acceptsFirstMouse,
/// a declared difference: the reference's first click only activates), so a click on the
/// row reveals the app from the active app (T3FinderReveal) and a press that starts a drag
/// makes the panel key, which keeps it shown while System Settings is not frontmost (sync,
/// the reference's `!frontmost && !isFocused`).
final class T3PermissionPanel: NSPanel {
    let content: T3PermissionHelperView
    var onEscape: () -> Void = {}
    var onBlur: () -> Void = {}
    init(permission: T3MacPermission, bundle: URL, icon: NSImage) {
        content = T3PermissionHelperView(frame: NSRect(x: 0, y: 0, width: 560, height: 140), bundle: bundle, icon: icon)
        super.init(contentRect: content.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        title = "Set up \(permission.title)"
        isOpaque = false; backgroundColor = .clear; hasShadow = false
        level = .floating; hidesOnDeactivate = false; isReleasedWhenClosed = false
        isExcludedFromWindowsMenu = true; collectionBehavior = [.fullScreenAuxiliary, .ignoresCycle]
        content.autoresizingMask = [.width, .height]
        contentView = content
    }
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
    // Ordering out or in under a still pointer sends no exit or entry (a Finder reveal
    // hides the panel under it), so the hover state is read again from the pointer.
    override func orderOut(_ sender: Any?) { super.orderOut(sender); content.syncHover() }
    override func orderFrontRegardless() { super.orderFrontRegardless(); content.syncHover() }
    // Reference preload: Escape anywhere in the panel closes it.
    override func cancelOperation(_ sender: Any?) { onEscape() }
    override func keyDown(with event: NSEvent) { if event.keyCode == 53 { onEscape() } else { super.keyDown(with: event) } }
    override func resignKey() { super.resignKey(); onBlur() }
}

/// The reference page: a 136 pt card (24 pt radius, 1 pt line, 20 pt padding) with
/// the header, the 52 pt app row and a close button that shows on hover.
final class T3PermissionHelperView: NSView {
    static let header = "↑ Drag T3 Code into the list above"
    struct Palette { let base, row, text, line: NSColor }
    /// Reference :root tokens, light and dark.
    static func palette(dark: Bool) -> Palette {
        let hex = { (value: Int) in NSColor(srgbRed: CGFloat(value >> 16 & 255) / 255, green: CGFloat(value >> 8 & 255) / 255, blue: CGFloat(value & 255) / 255, alpha: 1) }
        return dark ? Palette(base: hex(0x242424), row: hex(0x383838), text: hex(0xf5f5f5), line: hex(0x484848))
            : Palette(base: hex(0xffffff), row: hex(0xe7e7e7), text: hex(0x292929), line: hex(0xe3e3e3))
    }
    let title = NSTextField(labelWithString: T3PermissionHelperView.header)
    let appRow: T3PermissionHelperAppRow
    let closeButton = T3PermissionHelperClose()
    var onClose: () -> Void = {} { didSet { closeButton.onPress = onClose } }
    var onReveal: () -> Void = {} { didSet { appRow.onReveal = onReveal } }
    private var hovering = false { didSet { updateClose() } }
    init(frame: NSRect, bundle: URL, icon: NSImage) {
        appRow = T3PermissionHelperAppRow(bundle: bundle, icon: icon)
        super.init(frame: frame)
        title.font = .systemFont(ofSize: 15, weight: .semibold); title.lineBreakMode = .byClipping; title.maximumNumberOfLines = 1
        title.setAccessibilityRole(.staticText)
        closeButton.alphaValue = 0
        closeButton.focusChanged = { [weak self] in self?.updateClose() }
        for view in [title, appRow, closeButton] as [NSView] { addSubview(view) }
        updateColors(); needsLayout = true
    }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }
    override func setFrameSize(_ newSize: NSSize) { super.setFrameSize(newSize); needsLayout = true }
    var dark: Bool { effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua }
    var card: NSRect { bounds.insetBy(dx: 2, dy: 2) }
    override func layout() {
        super.layout()
        // Border 1 + padding 20; the header's 22 pt line, 20 pt gap, then the row.
        let inner = card.insetBy(dx: 21, dy: 21), line = title.fittingSize.height
        title.frame = NSRect(x: inner.minX, y: inner.minY + (22 - line) / 2, width: inner.width, height: line)
        appRow.frame = NSRect(x: inner.minX, y: inner.minY + 42, width: inner.width, height: 52)
        closeButton.frame = NSRect(x: card.maxX - 1 - 8 - 22, y: card.minY + 1 + 6, width: 22, height: 22)
    }
    private func updateClose() { closeButton.alphaValue = hovering || closeButton.focused ? 1 : 0 }
    private func updateColors() {
        let palette = Self.palette(dark: dark)
        title.textColor = palette.text; appRow.palette = palette; closeButton.palette = palette
        needsDisplay = true
    }
    override func viewDidChangeEffectiveAppearance() { super.viewDidChangeEffectiveAppearance(); updateColors() }
    override func draw(_ dirtyRect: NSRect) {
        let palette = Self.palette(dark: dark), path = NSBezierPath(roundedRect: card.insetBy(dx: 0.5, dy: 0.5), xRadius: 23.5, yRadius: 23.5)
        palette.base.setFill(); path.fill()
        palette.line.setStroke(); path.lineWidth = 1; path.stroke()
    }
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(rect: card, options: [.mouseEnteredAndExited, .activeAlways], owner: self))
    }
    override func mouseEntered(with event: NSEvent) { hovering = true }
    override func mouseExited(with event: NSEvent) { hovering = false }
    /// The pointer in screen coordinates (a seam for the AppKit tests).
    var pointer: () -> NSPoint = { NSEvent.mouseLocation }
    /// Reference #panel:hover, from where the pointer is; never while the panel is hidden.
    func syncHover() {
        guard let window, window.isVisible else { hovering = false; return }
        hovering = window.convertToScreen(convert(card, to: nil)).contains(pointer())
    }
    var closeVisible: Bool { closeButton.alphaValue > 0 }
}

/// The reference "#app" button: drag it to drag the app bundle into the list (a
/// failed drag reveals it in Finder); a click reveals it in Finder.
final class T3PermissionHelperAppRow: NSView, NSDraggingSource {
    let bundle: URL
    let icon: NSImage
    let label = NSTextField(labelWithString: "T3 Code")
    var onReveal: () -> Void = {}
    var palette = T3PermissionHelperView.palette(dark: false) { didSet { label.textColor = palette.text; needsDisplay = true } }
    private(set) var pressed: NSEvent?
    private(set) var dragging = false
    /// Reference #app:active: the press lasts until mouseUp or the drag's end.
    var grabbing: Bool { pressed != nil }
    init(bundle: URL, icon: NSImage) {
        self.bundle = bundle; self.icon = icon
        super.init(frame: .zero)
        label.font = .systemFont(ofSize: 16, weight: .semibold)
        addSubview(label)
        setAccessibilityElement(true); setAccessibilityRole(.button)
        setAccessibilityLabel("Drag T3 Code to System Settings, or click to reveal in Finder")
    }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }
    override func setFrameSize(_ newSize: NSSize) { super.setFrameSize(newSize); needsLayout = true }
    override func layout() {
        super.layout()
        let line = label.fittingSize.height
        label.frame = NSRect(x: 12 + 32 + 12, y: (bounds.height - line) / 2, width: max(0, bounds.width - 68), height: line)
    }
    override func draw(_ dirtyRect: NSRect) {
        palette.row.setFill(); NSBezierPath(roundedRect: bounds, xRadius: 10, yRadius: 10).fill()
        icon.draw(in: NSRect(x: 12, y: (bounds.height - 32) / 2, width: 32, height: 32), from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: [.interpolation: NSImageInterpolation.high.rawValue])
    }
    override func hitTest(_ point: NSPoint) -> NSView? { frame.contains(point) ? self : nil }
    // Declared difference (realinput-1010c RC-1): the reference's window keeps Electron's macOS default
    // acceptFirstMouse false, so its first click from System Settings only activates T3 Code; here the activating
    // click also reaches the row (a click reveals, a press drags), as RC-1 asks. The close button does the same.
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(rect: bounds, options: [.mouseEnteredAndExited, .activeAlways], owner: self))
    }
    // Reference cursor: grab, grabbing while pressed.
    override func mouseEntered(with event: NSEvent) { (grabbing ? NSCursor.closedHand : NSCursor.openHand).set() }
    override func mouseExited(with event: NSEvent) { if !grabbing { NSCursor.arrow.set() } }
    override func mouseDown(with event: NSEvent) { pressed = event; dragging = false; NSCursor.closedHand.set() }
    override func mouseDragged(with event: NSEvent) {
        guard let pressed, !dragging else { return }
        let from = pressed.locationInWindow, to = event.locationInWindow
        guard hypot(to.x - from.x, to.y - from.y) >= 3 else { return }
        dragging = true
        if !startDrag(event) { onReveal() }
    }
    override func mouseUp(with event: NSEvent) {
        defer { pressed = nil; dragging = false; NSCursor.openHand.set() }
        if pressed != nil, !dragging, bounds.contains(convert(event.locationInWindow, from: nil)) { onReveal() }
    }
    override func accessibilityPerformPress() -> Bool { onReveal(); return true }
    /// Reference startDrag({ file: bundle, icon }): the bundle's file URL with the 64 pt icon.
    @discardableResult func startDrag(_ event: NSEvent) -> Bool {
        guard window != nil else { return false }
        let item = NSDraggingItem(pasteboardWriter: bundle as NSURL), point = convert(event.locationInWindow, from: nil)
        item.setDraggingFrame(NSRect(x: point.x - 32, y: point.y - 32, width: 64, height: 64), contents: icon)
        beginDraggingSession(with: [item], event: event, source: self)
        return true
    }
    /// Copy or link only: a drop on the Trash must never move the app.
    static func operations(_ context: NSDraggingContext) -> NSDragOperation { context == .outsideApplication ? [.copy, .link, .generic] : [] }
    func draggingSession(_ session: NSDraggingSession, sourceOperationMaskFor context: NSDraggingContext) -> NSDragOperation { Self.operations(context) }
    /// AppKit sends the row no mouseUp after a drag, so the drag's end releases the press,
    /// whether the drop was taken, refused or made elsewhere.
    func draggingSession(_ session: NSDraggingSession, endedAt screenPoint: NSPoint, operation: NSDragOperation) { dragEnded(at: screenPoint) }
    func dragEnded(at screenPoint: NSPoint) {
        pressed = nil; dragging = false
        guard let window, window.isVisible else { return }
        (bounds.contains(convert(window.convertPoint(fromScreen: screenPoint), from: nil)) ? NSCursor.openHand : NSCursor.arrow).set()
    }
}

/// The reference "#close" button: a 22 pt circle with "×", shown while the panel is
/// hovered or the button has keyboard focus.
final class T3PermissionHelperClose: NSView {
    var onPress: () -> Void = {}
    var focusChanged: () -> Void = {}
    private(set) var focused = false { didSet { focusChanged() } }
    var palette = T3PermissionHelperView.palette(dark: false) { didSet { needsDisplay = true } }
    override init(frame: NSRect) {
        super.init(frame: frame)
        setAccessibilityElement(true); setAccessibilityRole(.button); setAccessibilityLabel("Close permission helper")
    }
    required init?(coder: NSCoder) { nil }
    override func draw(_ dirtyRect: NSRect) {
        palette.base.setFill(); NSBezierPath(ovalIn: bounds).fill()
        let mark = NSAttributedString(string: "×", attributes: [.font: NSFont.systemFont(ofSize: 18), .foregroundColor: palette.text])
        let size = mark.size()
        mark.draw(at: NSPoint(x: (bounds.width - size.width) / 2, y: (bounds.height - size.height) / 2))
    }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override var acceptsFirstResponder: Bool { true }
    /// Reference :focus-visible: only focus that came from the keyboard shows the button
    /// (AppKit also makes it first responder when the panel orders in).
    override func becomeFirstResponder() -> Bool { focused = NSApp.currentEvent?.type == .keyDown; return true }
    override func resignFirstResponder() -> Bool { focused = false; return true }
    override func keyDown(with event: NSEvent) { if [" ", "\r"].contains(event.charactersIgnoringModifiers) { onPress() } else { super.keyDown(with: event) } }
    override func mouseDown(with event: NSEvent) {}
    override func mouseUp(with event: NSEvent) { if bounds.contains(convert(event.locationInWindow, from: nil)) { onPress() } }
    override func resetCursorRects() { addCursorRect(bounds, cursor: .pointingHand) }
    override func accessibilityPerformPress() -> Bool { onPress(); return true }
}
