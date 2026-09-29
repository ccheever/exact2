// `<ghostty-terminal tab=…>` on macOS: a libghostty surface per tab, owned by
// the module (`TerminalStore`) so it outlives the view that shows it — switch
// tabs and come back, the same process is still there. The Contract's node
// is a container (`TerminalInstance.view`); the tab's `SurfaceView` moves
// into whichever container names it. Keys, mouse and scroll go to libghostty
// the way Ghostty's own macOS app sends them, without IME composition.
import AppKit
import Carbon
import CoreText
import GhosttyKit
import IOSurface
import Metal
import QuartzCore
import UniformTypeIdentifiers
import UserNotifications

/// The one libghostty app in the process: `ghostty_init`, the app's own
/// configuration (the person's Ghostty config is not read), and the
/// runtime callbacks.
final class GhosttyRuntime {
    /// Made once, started before anyone can see it: a runtime callback fired
    /// during `ghostty_app_new` reaches the instance through `userdata`, never
    /// through this property (which would re-enter its own initialisation).
    static let shared: GhosttyRuntime = { let r = GhosttyRuntime(); r.start(); return r }()
    private(set) var app: ghostty_app_t?
    private(set) var failure: String?
    /// The terminal's background from the user's configuration, as `#rrggbb`,
    /// so the Contract can paint the pane around the surface to match.
    fileprivate(set) var background = ""
    fileprivate var lightConfig: ghostty_config_t?
    fileprivate var darkConfig: ghostty_config_t?
    fileprivate func config(for scheme: String) -> ghostty_config_t? { scheme == "dark" ? darkConfig : lightConfig }

    /// The pane paints this configuration's background around the surface.
    fileprivate func useBackground(of config: ghostty_config_t) {
        var color = ghostty_config_color_s()
        let key = "background"
        guard ghostty_config_get(config, &color, key, UInt(key.utf8.count)) else { return }
        let hex = String(format: "#%02x%02x%02x", color.r, color.g, color.b)
        guard hex != background else { return }
        background = hex
        NotificationCenter.default.post(name: .ochoTerminalColors, object: nil)
    }
    /// Which surface view a `userdata` is.
    fileprivate static func view(_ userdata: UnsafeMutableRawPointer?) -> SurfaceView? {
        userdata.map { Unmanaged<SurfaceView>.fromOpaque($0).takeUnretainedValue() }
    }
    private static func runtime(_ userdata: UnsafeMutableRawPointer?) -> GhosttyRuntime? {
        userdata.map { Unmanaged<GhosttyRuntime>.fromOpaque($0).takeUnretainedValue() }
    }

    private init() {}

    private func start() {
        // Themes live in Ghostty's resources: the installed app's, else a
        // checkout's build. libghostty finds them through this variable.
        if ProcessInfo.processInfo.environment["GHOSTTY_RESOURCES_DIR"] == nil {
            let home = FileManager.default.homeDirectoryForCurrentUser.path
            for dir in ["/Applications/Ghostty.app/Contents/Resources/ghostty", "\(home)/Applications/Ghostty.app/Contents/Resources/ghostty", "\(home)/Developer/ghostty/zig-out/share/ghostty"]
            where FileManager.default.fileExists(atPath: dir + "/themes") {
                setenv("GHOSTTY_RESOURCES_DIR", dir, 1)
                break
            }
        }
        if ghostty_init(UInt(CommandLine.argc), CommandLine.unsafeArgv) != 0 { failure = "ghostty_init failed"; return }
        // The app's own configuration, never the person's Ghostty config: the
        // terminal is a pane of this app, and its look follows the app's
        // light or dark appearance — one whole configuration per appearance,
        // pushed to every surface as the appearance changes.
        func configuration(theme: String) -> ghostty_config_t {
            let config = ghostty_config_new()!
            let text = "theme = \(theme)\n" + overrides
            let path = FileManager.default.temporaryDirectory.appendingPathComponent("ocho-ghostty-\(getpid())-\(theme.replacingOccurrences(of: " ", with: "-")).conf")
            try? text.write(to: path, atomically: true, encoding: .utf8)
            path.path.withCString { ghostty_config_load_file(config, $0) }
            ghostty_config_finalize(config)
            return config
        }
        let overrides = """
        confirm-close-surface = false
        window-padding-x = 12
        window-padding-y = 8
        window-padding-balance = true
        macos-titlebar-style = hidden
        quit-after-last-window-closed = false
        """
        lightConfig = configuration(theme: "One Half Light")
        darkConfig = configuration(theme: "One Half Dark")
        let config = lightConfig!
        var color = ghostty_config_color_s()
        let key = "background"
        if ghostty_config_get(config, &color, key, UInt(key.utf8.count)) {
            background = String(format: "#%02x%02x%02x", color.r, color.g, color.b)
        }
        var runtime = ghostty_runtime_config_s()
        runtime.userdata = Unmanaged.passUnretained(self).toOpaque()
        runtime.supports_selection_clipboard = false
        runtime.wakeup_cb = { userdata in
            guard let r = GhosttyRuntime.runtime(userdata) else { return }
            DispatchQueue.main.async { r.tick() }
        }
        runtime.action_cb = { _, target, action in GhosttyRuntime.action(target, action) }
        runtime.read_clipboard_cb = { userdata, _, state in
            guard let view = GhosttyRuntime.view(userdata), let surface = view.surface else { return false }
            guard let text = NSPasteboard.general.string(forType: .string) else { return false }
            text.withCString { ghostty_surface_complete_clipboard_request(surface, $0, state, false) }
            return true
        }
        runtime.confirm_read_clipboard_cb = { userdata, text, state, _ in
            guard let view = GhosttyRuntime.view(userdata), let surface = view.surface, let text else { return }
            ghostty_surface_complete_clipboard_request(surface, text, state, true)
        }
        runtime.write_clipboard_cb = { _, _, content, count, _ in
            guard let content, count > 0 else { return }
            for i in 0..<count {
                let item = content[i]
                guard let mime = item.mime, let data = item.data else { continue }
                if String(cString: mime).hasPrefix("text/plain") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(String(cString: data), forType: .string)
                    break
                }
            }
        }
        runtime.close_surface_cb = { userdata, _ in
            guard let view = GhosttyRuntime.view(userdata) else { return }
            DispatchQueue.main.async { view.onExit?() }
        }
        guard let app = ghostty_app_new(&runtime, config) else { failure = "ghostty_app_new failed"; return }
        self.app = app
        ghostty_app_set_focus(app, NSApp.isActive)
        NotificationCenter.default.addObserver(forName: NSApplication.didBecomeActiveNotification, object: nil, queue: .main) { _ in ghostty_app_set_focus(app, true) }
        NotificationCenter.default.addObserver(forName: NSApplication.didResignActiveNotification, object: nil, queue: .main) { _ in ghostty_app_set_focus(app, false) }
        NotificationCenter.default.addObserver(forName: NSTextInputContext.keyboardSelectionDidChangeNotification, object: nil, queue: .main) { _ in ghostty_app_keyboard_changed(app) }
    }

    func tick() { if let app { ghostty_app_tick(app) } }

    private static func action(_ target: ghostty_target_s, _ action: ghostty_action_s) -> Bool {
        guard target.tag == GHOSTTY_TARGET_SURFACE, let surface = target.target.surface else { return false }
        let view = view(ghostty_surface_userdata(surface))
        switch action.tag {
        case GHOSTTY_ACTION_SET_TITLE:
            if let title = action.action.set_title.title { view?.onTitle?(String(cString: title)) }
            return true
        case GHOSTTY_ACTION_MOUSE_SHAPE:
            view?.mouseShape = action.action.mouse_shape
            return true
        case GHOSTTY_ACTION_COLOR_CHANGE:
            let change = action.action.color_change
            if change.kind == GHOSTTY_ACTION_COLOR_KIND_BACKGROUND {
                let hex = String(format: "#%02x%02x%02x", change.r, change.g, change.b)
                DispatchQueue.main.async {
                    guard GhosttyRuntime.shared.background != hex else { return }
                    GhosttyRuntime.shared.background = hex
                    NotificationCenter.default.post(name: .ochoTerminalColors, object: nil)
                }
            }
            return true
        case GHOSTTY_ACTION_RING_BELL:
            return true
        default:
            return false
        }
    }
}

/// The NSView libghostty draws into; one per tab, alive while the tab is.
final class SurfaceView: NSView {
    private(set) var surface: ghostty_surface_t?
    var onExit: (() -> Void)?
    var onTitle: ((String) -> Void)?
    var mouseShape = GHOSTTY_MOUSE_SHAPE_DEFAULT { didSet { window?.invalidateCursorRects(for: self) } }
    private var trackingArea: NSTrackingArea?
    private var strings: [UnsafeMutablePointer<CChar>] = []

    init(command: [String], environment: [String: String]) {
        super.init(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        autoresizingMask = [.width, .height]
        guard let app = GhosttyRuntime.shared.app else { return }
        var config = ghostty_surface_config_s()
        config.platform_tag = GHOSTTY_PLATFORM_MACOS
        config.platform = ghostty_platform_u(macos: ghostty_platform_macos_s(nsview: Unmanaged.passUnretained(self).toOpaque()))
        config.userdata = Unmanaged.passUnretained(self).toOpaque()
        config.scale_factor = Double(NSScreen.main?.backingScaleFactor ?? 2)
        config.font_size = 0
        config.context = GHOSTTY_SURFACE_CONTEXT_WINDOW
        config.wait_after_command = false
        let shell = command.map { Self.quoted($0) }.joined(separator: " ")
        let commandC = strdup(shell)!
        strings.append(commandC)
        config.command = UnsafePointer(commandC)
        var env: [ghostty_env_var_s] = []
        for (k, v) in environment.sorted(by: { $0.key < $1.key }) {
            let kc = strdup(k)!, vc = strdup(v)!
            strings.append(kc); strings.append(vc)
            env.append(ghostty_env_var_s(key: UnsafePointer(kc), value: UnsafePointer(vc)))
        }
        env.withUnsafeMutableBufferPointer { buffer in
            config.env_vars = buffer.baseAddress
            config.env_var_count = buffer.count
            surface = ghostty_surface_new(app, &config)
        }
        updateTrackingAreas()
    }
    required init?(coder: NSCoder) { nil }

    deinit {
        if let surface { ghostty_surface_free(surface) }
        for s in strings { free(s) }
    }

    /// A shell word: single-quoted when it needs it.
    private static func quoted(_ word: String) -> String {
        if !word.isEmpty && word.unicodeScalars.allSatisfy({ CharacterSet.alphanumerics.contains($0) || "-_./=:@~,+".unicodeScalars.contains($0) }) { return word }
        return "'" + word.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override var isFlipped: Bool { false }

    override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        if ok, let surface { ghostty_surface_set_focus(surface, true) }
        return ok
    }
    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok, let surface { ghostty_surface_set_focus(surface, false) }
        return ok
    }

    /// The app's appearance ("light" or "dark"), from the Contract: the
    /// theme pair in the configuration follows it.
    var scheme = "light" { didSet { if scheme != oldValue { syncAppearance() } } }
    func applyScheme() { syncAppearance() }

    private func syncAppearance() {
        guard let surface, let config = GhosttyRuntime.shared.config(for: scheme) else { return }
        ghostty_surface_update_config(surface, config)
        ghostty_surface_set_color_scheme(surface, scheme == "dark" ? GHOSTTY_COLOR_SCHEME_DARK : GHOSTTY_COLOR_SCHEME_LIGHT)
        GhosttyRuntime.shared.useBackground(of: config)
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard let window else { if let surface { ghostty_surface_set_occlusion(surface, false) }; return }
        if let surface { ghostty_surface_set_occlusion(surface, true) }
        syncAppearance()
        viewDidChangeBackingProperties()
        sync()
        // The tab the person switched to owns the keyboard.
        DispatchQueue.main.async { [weak self] in
            guard let self, self.window === window, window.firstResponder !== self else { return }
            window.makeFirstResponder(self)
        }
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        sync()
    }

    /// Tell libghostty the framebuffer size in pixels.
    private func sync() {
        guard let surface, bounds.width > 0, bounds.height > 0 else { return }
        let px = convertToBacking(bounds.size)
        ghostty_surface_set_size(surface, UInt32(max(1, px.width)), UInt32(max(1, px.height)))
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        guard let window else { return }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        layer?.contentsScale = window.backingScaleFactor
        CATransaction.commit()
        guard let surface else { return }
        ghostty_surface_set_content_scale(surface, window.backingScaleFactor, window.backingScaleFactor)
        sync()
    }

    override func updateTrackingAreas() {
        if let trackingArea { removeTrackingArea(trackingArea) }
        let area = NSTrackingArea(rect: bounds, options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect], owner: self, userInfo: nil)
        addTrackingArea(area)
        trackingArea = area
        super.updateTrackingAreas()
    }

    override func resetCursorRects() {
        let cursor: NSCursor
        switch mouseShape {
        case GHOSTTY_MOUSE_SHAPE_TEXT: cursor = .iBeam
        case GHOSTTY_MOUSE_SHAPE_POINTER: cursor = .pointingHand
        case GHOSTTY_MOUSE_SHAPE_GRAB, GHOSTTY_MOUSE_SHAPE_GRABBING: cursor = .openHand
        default: cursor = .arrow
        }
        addCursorRect(bounds, cursor: cursor)
    }

    // MARK: keys

    private static func mods(_ flags: NSEvent.ModifierFlags) -> ghostty_input_mods_e {
        var m = GHOSTTY_MODS_NONE.rawValue
        if flags.contains(.shift) { m |= GHOSTTY_MODS_SHIFT.rawValue }
        if flags.contains(.control) { m |= GHOSTTY_MODS_CTRL.rawValue }
        if flags.contains(.option) { m |= GHOSTTY_MODS_ALT.rawValue }
        if flags.contains(.command) { m |= GHOSTTY_MODS_SUPER.rawValue }
        if flags.contains(.capsLock) { m |= GHOSTTY_MODS_CAPS.rawValue }
        return ghostty_input_mods_e(rawValue: m)
    }

    private func key(_ action: ghostty_input_action_e, _ event: NSEvent) -> Bool {
        guard let surface else { return false }
        var ev = ghostty_input_key_s()
        ev.action = action
        ev.keycode = UInt32(event.keyCode)
        ev.mods = Self.mods(event.modifierFlags)
        ev.consumed_mods = Self.mods(event.modifierFlags.subtracting([.control, .command]))
        ev.composing = false
        ev.unshifted_codepoint = 0
        if event.type == .keyDown || event.type == .keyUp, let chars = event.characters(byApplyingModifiers: []), let scalar = chars.unicodeScalars.first {
            ev.unshifted_codepoint = scalar.value
        }
        var text: String? = nil
        // `characters` exists only on key events: AppKit raises on a flagsChanged event (a lone modifier).
        let typed = event.type == .keyDown || event.type == .keyUp
        if action != GHOSTTY_ACTION_RELEASE, typed, let chars = event.characters, !chars.isEmpty {
            if chars.count == 1, let scalar = chars.unicodeScalars.first {
                if scalar.value < 0x20 { text = event.characters(byApplyingModifiers: event.modifierFlags.subtracting(.control)) }
                else if scalar.value >= 0xF700 && scalar.value <= 0xF8FF { text = nil }
                else { text = chars }
            } else { text = chars }
        }
        if let text, let first = text.utf8.first, first >= 0x20 {
            return text.withCString { ptr in
                ev.text = ptr
                return ghostty_surface_key(surface, ev)
            }
        }
        return ghostty_surface_key(surface, ev)
    }

    override func keyDown(with event: NSEvent) {
        _ = key(event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS, event)
    }
    override func keyUp(with event: NSEvent) { _ = key(GHOSTTY_ACTION_RELEASE, event) }
    override func flagsChanged(with event: NSEvent) {
        // A modifier alone: pressed when its flag is now set, released otherwise.
        let flag: NSEvent.ModifierFlags? = {
            switch Int(event.keyCode) {
            case kVK_Shift, kVK_RightShift: return .shift
            case kVK_Control, kVK_RightControl: return .control
            case kVK_Option, kVK_RightOption: return .option
            case kVK_Command, kVK_RightCommand: return .command
            default: return nil
            }
        }()
        guard let flag else { return }
        _ = key(event.modifierFlags.contains(flag) ? GHOSTTY_ACTION_PRESS : GHOSTTY_ACTION_RELEASE, event)
    }
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        // Command chords the app declares are the host's (it saw them first);
        // control chords and the rest are the terminal's.
        guard event.type == .keyDown, window?.firstResponder === self else { return false }
        if event.modifierFlags.contains(.command) { return false }
        return key(event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS, event)
    }

    // MARK: mouse

    private func button(_ state: ghostty_input_mouse_state_e, _ button: ghostty_input_mouse_button_e, _ event: NSEvent) {
        guard let surface else { return }
        _ = ghostty_surface_mouse_button(surface, state, button, Self.mods(event.modifierFlags))
    }
    private func position(_ event: NSEvent) {
        guard let surface else { return }
        let p = convert(event.locationInWindow, from: nil)
        ghostty_surface_mouse_pos(surface, p.x, bounds.height - p.y, Self.mods(event.modifierFlags))
    }
    override func mouseDown(with event: NSEvent) {
        if window?.firstResponder !== self { window?.makeFirstResponder(self) }
        position(event)
        button(GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_LEFT, event)
    }
    override func mouseUp(with event: NSEvent) { button(GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_LEFT, event) }
    override func rightMouseDown(with event: NSEvent) { position(event); button(GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_RIGHT, event) }
    override func rightMouseUp(with event: NSEvent) { button(GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_RIGHT, event) }
    override func otherMouseDown(with event: NSEvent) { position(event); button(GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_MIDDLE, event) }
    override func otherMouseUp(with event: NSEvent) { button(GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_MIDDLE, event) }
    override func mouseMoved(with event: NSEvent) { position(event) }
    override func mouseDragged(with event: NSEvent) { position(event) }
    override func rightMouseDragged(with event: NSEvent) { position(event) }
    override func mouseExited(with event: NSEvent) {
        guard let surface else { return }
        ghostty_surface_mouse_pos(surface, -1, -1, Self.mods(event.modifierFlags))
    }
    override func scrollWheel(with event: NSEvent) {
        guard let surface else { return }
        var x = event.scrollingDeltaX, y = event.scrollingDeltaY
        var mods: Int32 = 0
        if event.hasPreciseScrollingDeltas {
            x *= 2; y *= 2
            mods |= 1
            let momentum: Int32
            switch event.momentumPhase {
            case .began: momentum = 1
            case .stationary: momentum = 2
            case .changed: momentum = 3
            case .ended: momentum = 4
            case .cancelled: momentum = 5
            default: momentum = 0
            }
            mods |= momentum << 1
        }
        ghostty_surface_mouse_scroll(surface, x, y, mods)
    }
}

/// The terminals, by tab: made on first sight, kept while the tab is open.
final class TerminalStore {
    private weak var module: OchoModule?
    private var views: [String: SurfaceView] = [:]
    private(set) var exited: Set<String> = []

    init(module: OchoModule) { self.module = module }

    func view(for tab: Tab) -> SurfaceView? {
        if let v = views[tab.id] { return v }
        guard let module, let binary = module.fleet.binary, GhosttyRuntime.shared.app != nil else { return nil }
        var env = tab.env
        env["TERM_PROGRAM"] = "ocho"
        let view = SurfaceView(command: [binary] + tab.command, environment: env)
        view.onExit = { [weak self, weak module] in
            self?.exited.insert(tab.id)
            module?.announce()
            NotificationCenter.default.post(name: .ochoTerminalExited, object: tab.id)
        }
        views[tab.id] = view
        return view
    }

    func close(tab: String) {
        if let v = views.removeValue(forKey: tab) { v.removeFromSuperview() }
        exited.remove(tab)
    }

    func destroyAll() {
        for (_, v) in views { v.removeFromSuperview() }
        views.removeAll()
    }
}

extension Notification.Name {
    static let ochoTerminalExited = Notification.Name("ocho.terminal.exited")
    static let ochoTerminalColors = Notification.Name("ocho.terminal.colors")
}

/// The Contract's `<ghostty-terminal>` node: a container the tab's surface
/// moves into. Props: `tab`. Events: `load` once a surface is shown,
/// `message` "exited" when its process ends, "unavailable" with no terminal.
final class TerminalInstance: ExactNativeInstance {
    private weak var module: OchoModule?
    private let container = TerminalContainer(frame: .zero)
    private var tabId = ""
    private var observer: NSObjectProtocol?

    init(module: OchoModule, props: [String: String], events: ExactNativeEvents) throws {
        self.module = module
        super.init(events: events)
        observer = NotificationCenter.default.addObserver(forName: .ochoTerminalExited, object: nil, queue: .main) { [weak self] note in
            guard let self, (note.object as? String) == self.tabId else { return }
            self.events.message("exited")
        }
        try setProps(props)
    }

    override var view: ExactNativeView { container }

    override func setProps(_ props: [String: String]) throws {
        let next = props["tab"] ?? ""
        let appearance = props["scheme"] ?? "light"
        if next == tabId {
            (container.subviews.first as? SurfaceView)?.scheme = appearance
            return
        }
        tabId = next
        container.subviews.forEach { $0.removeFromSuperview() }
        guard let module, !next.isEmpty else { return }
        guard let tab = module.tabs.tab(next) else { events.message("missing"); return }
        guard let surface = module.terminals.view(for: tab) else {
            events.message(GhosttyRuntime.shared.failure ?? "unavailable")
            return
        }
        surface.scheme = appearance
        surface.applyScheme()
        surface.frame = container.bounds
        container.addSubview(surface)
        if module.terminals.exited.contains(next) { events.message("exited") }
        events.load()
    }

    override func destroy() {
        if let observer { NotificationCenter.default.removeObserver(observer) }
        container.subviews.forEach { $0.removeFromSuperview() }
    }
}

final class TerminalContainer: NSView {
    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        autoresizesSubviews = true
    }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }
    override func layout() {
        super.layout()
        for v in subviews { v.frame = bounds }
    }
    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        for v in subviews { v.frame = bounds }
    }
}
