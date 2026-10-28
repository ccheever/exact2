// @exact/global-shortcuts on macOS (LLP 1133.025 D3): a chord registered
// system-wide with Carbon's `RegisterEventHotKey` while something reads it,
// and a modifier pair (`Shift+Shift`) heard by a `.flagsChanged` monitor.
// The app's composed module calls it outside the agent; under the agent the
// runner's substitute answers, unless the drive binds it.
//
// One chord, many readers: the process registers a chord once and counts
// its readers, each an instance (a session) and an `id`. A press goes to
// the readers of one session: the key window's if it reads the chord, else
// the oldest reader's (the status session is the host's to know, so this
// module does not prefer it). The RFC puts the one hot-key handler in the
// host's package context (D11 item 2); it lives here, process-wide, until a
// second package needs it.
import AppKit
import ApplicationServices
import Carbon.HIToolbox
import ExactPackage

public final class GlobalShortcutsModule: ExactPackage {
    private let context: ExactPackageContext
    /// The chords this session reads, by `id`.
    private var reading: [String: String] = [:]

    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            let keys = args["keys"] as? String ?? ""
            switch name {
            case "globalShortcut":
                let id = args["id"] as? String ?? ""
                let (registered, reason) = HotKeys.shared.read(keys, id: id, by: self)
                if registered { reading[id] = keys }
                context.log("global-shortcuts: \(keys) for \(id): \(registered ? "registered" : "not registered (\(reason ?? ""))")")
                reply(["id": id, "keys": keys, "registered": registered, "reason": reason ?? NSNull()])
            case "checkGlobalShortcut":
                let (registered, reason) = HotKeys.shared.check(keys)
                reply(["keys": keys, "registered": registered, "reason": reason ?? NSNull()])
            case "globalShortcutStatus":
                reply(status())
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "globalShortcutStatusSync" ? status() : ["error": "NotSupportedError"]
    }

    private func status() -> [String: Any] {
        ["available": true, "apple": ["accessibilityTrusted": AXIsProcessTrusted()]]
    }

    /// No reader asks with these arguments any longer: this session's
    /// reading of that chord for that id ends.
    public func release(_ name: String, args: [String: Any]) {
        guard name == "globalShortcut", let id = args["id"] as? String, let keys = args["keys"] as? String,
              reading[id] == keys else { return }
        reading[id] = nil
        HotKeys.shared.release(keys, id: id, by: self)
    }

    /// No reader of `globalShortcut` is left in this session.
    public func release(_ name: String) {
        guard name == "globalShortcut" else { return }
        for (id, keys) in reading { HotKeys.shared.release(keys, id: id, by: self) }
        reading = [:]
    }

    public func destroy(_ done: @escaping () -> Void) {
        release("globalShortcut")
        done()
    }

    /// A press of `keys`, heard by this session for `ids`.
    fileprivate func pressed(_ keys: String, ids: [String]) {
        for id in ids { context.emit("globalshortcut", [id, ["id": id, "keys": keys]]) }
    }

    /// Whether this session's window is the key window.
    fileprivate var isKey: Bool { context.presentation?.window.isKeyWindow == true }
}

/// The process's hot keys: one Carbon registration per chord, one monitor
/// for every pair, and the readers of each.
private final class HotKeys {
    static let shared = HotKeys()

    private struct Reader { weak var module: GlobalShortcutsModule?; let id: String; let order: Int }
    private final class Chord {
        var ref: EventHotKeyRef?
        let number: UInt32
        var readers: [Reader] = []
        init(number: UInt32) { self.number = number }
    }
    private var chords: [String: Chord] = [:]
    private var next: UInt32 = 1
    private var order = 0
    private var handler: EventHandlerRef?
    private var monitors: [Any] = []
    /// The modifier keys down now, by key code, for pairs.
    private var down: Set<UInt16> = []
    private static let signature: OSType = 0x4558_4354 // 'EXCT'

    /// `keys` read for `id` by `module`: registered, or why not.
    func read(_ keys: String, id: String, by module: GlobalShortcutsModule) -> (Bool, String?) {
        if let chord = chords[keys] {
            if !chord.readers.contains(where: { $0.module === module && $0.id == id }) { add(chord, id: id, by: module) }
            return (true, nil)
        }
        let number = next
        next += 1
        let chord = Chord(number: number)
        if let pair = Self.pair(keys) {
            guard AXIsProcessTrusted() else { return (false, "not-allowed") }
            _ = pair
            installMonitors()
        } else {
            let (ref, reason) = register(keys, number: number)
            guard let ref else { return (false, reason) }
            chord.ref = ref
            installHandler()
        }
        chords[keys] = chord
        add(chord, id: id, by: module)
        return (true, nil)
    }

    private func add(_ chord: Chord, id: String, by module: GlobalShortcutsModule) {
        order += 1
        chord.readers.append(Reader(module: module, id: id, order: order))
    }

    /// Whether `keys` could be registered now, keeping nothing.
    func check(_ keys: String) -> (Bool, String?) {
        if chords[keys] != nil { return (true, nil) }
        if Self.pair(keys) != nil { return AXIsProcessTrusted() ? (true, nil) : (false, "not-allowed") }
        let (ref, reason) = register(keys, number: 0)
        if let ref { UnregisterEventHotKey(ref); return (true, nil) }
        return (false, reason)
    }

    func release(_ keys: String, id: String, by module: GlobalShortcutsModule) {
        guard let chord = chords[keys] else { return }
        chord.readers.removeAll { $0.module == nil || ($0.module === module && $0.id == id) }
        guard chord.readers.isEmpty else { return }
        if let ref = chord.ref { UnregisterEventHotKey(ref) }
        chords[keys] = nil
        if !chords.keys.contains(where: { Self.pair($0) != nil }) { removeMonitors() }
    }

    /// A press of `chord`: once, to one session's readers.
    private func deliver(_ keys: String) {
        guard let chord = chords[keys] else { return }
        chord.readers.removeAll { $0.module == nil }
        let byOrder = chord.readers.sorted { $0.order < $1.order }
        guard let target = byOrder.first(where: { $0.module?.isKey == true })?.module ?? byOrder.first?.module else { return }
        target.pressed(keys, ids: byOrder.filter { $0.module === target }.map(\.id))
    }

    // MARK: Carbon

    private func register(_ keys: String, number: UInt32) -> (EventHotKeyRef?, String?) {
        guard let (code, modifiers) = Self.carbon(keys) else { return (nil, "invalid") }
        // macOS 15: a hot key needs a modifier other than Shift or Option
        // (Option alone again from 15.2).
        if modifiers & UInt32(cmdKey | controlKey) == 0, modifiers != UInt32(optionKey) { return (nil, "reserved") }
        var ref: EventHotKeyRef?
        let status = RegisterEventHotKey(code, modifiers, EventHotKeyID(signature: Self.signature, id: number),
                                         GetApplicationEventTarget(), 0, &ref)
        switch status {
        case noErr: return (ref, nil)
        case OSStatus(eventHotKeyExistsErr): return (nil, "in-use")
        default: return (nil, "reserved")
        }
    }

    private func installHandler() {
        guard handler == nil else { return }
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(GetApplicationEventTarget(), { _, event, _ in
            var key = EventHotKeyID()
            GetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID),
                              nil, MemoryLayout<EventHotKeyID>.size, nil, &key)
            guard key.signature == HotKeys.signature else { return OSStatus(eventNotHandledErr) }
            let shared = HotKeys.shared
            if let keys = shared.chords.first(where: { $0.value.number == key.id })?.key {
                DispatchQueue.main.async { shared.deliver(keys) }
            }
            return noErr
        }, 1, &spec, nil, &handler)
    }

    // MARK: Pairs

    /// The two key codes of a pair's modifier (`Shift+Shift`), or nil.
    private static func pair(_ keys: String) -> (UInt16, UInt16)? {
        switch keys {
        case "Shift+Shift": (UInt16(kVK_Shift), UInt16(kVK_RightShift))
        case "Control+Control": (UInt16(kVK_Control), UInt16(kVK_RightControl))
        case "Alt+Alt": (UInt16(kVK_Option), UInt16(kVK_RightOption))
        case "Meta+Meta": (UInt16(kVK_Command), UInt16(kVK_RightCommand))
        default: nil
        }
    }

    /// `.flagsChanged` only, in this app and in others: which modifier keys
    /// are down, and nothing else.
    private func installMonitors() {
        guard monitors.isEmpty else { return }
        let heard: (NSEvent) -> Void = { [weak self] event in self?.flags(event) }
        if let global = NSEvent.addGlobalMonitorForEvents(matching: .flagsChanged, handler: heard) { monitors.append(global) }
        if let local = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged, handler: { heard($0); return $0 }) { monitors.append(local) }
    }

    private func removeMonitors() {
        monitors.forEach(NSEvent.removeMonitor)
        monitors = []
        down = []
    }

    private func flags(_ event: NSEvent) {
        let code = event.keyCode
        let wasDown = down.contains(code)
        // A modifier key's flagsChanged toggles it; the device flags say which side.
        if wasDown { down.remove(code) } else { down.insert(code) }
        guard !wasDown else { return }
        for keys in chords.keys {
            guard let (left, right) = Self.pair(keys), [left, right].contains(code), down.contains(left), down.contains(right) else { continue }
            deliver(keys)
        }
    }

    // MARK: Spelling

    /// `aria-keyshortcuts` to Carbon's virtual key and modifiers.
    private static func carbon(_ keys: String) -> (UInt32, UInt32)? {
        let parts = keys.split(separator: "+").map(String.init)
        guard let key = parts.last, let code = code(key) else { return nil }
        var modifiers: UInt32 = 0
        for name in parts.dropLast() {
            switch name {
            case "Meta": modifiers |= UInt32(cmdKey)
            case "Control": modifiers |= UInt32(controlKey)
            case "Alt": modifiers |= UInt32(optionKey)
            case "Shift": modifiers |= UInt32(shiftKey)
            default: return nil
            }
        }
        return (UInt32(code), modifiers)
    }

    private static let letters: [Character: Int] = [
        "A": kVK_ANSI_A, "B": kVK_ANSI_B, "C": kVK_ANSI_C, "D": kVK_ANSI_D, "E": kVK_ANSI_E, "F": kVK_ANSI_F,
        "G": kVK_ANSI_G, "H": kVK_ANSI_H, "I": kVK_ANSI_I, "J": kVK_ANSI_J, "K": kVK_ANSI_K, "L": kVK_ANSI_L,
        "M": kVK_ANSI_M, "N": kVK_ANSI_N, "O": kVK_ANSI_O, "P": kVK_ANSI_P, "Q": kVK_ANSI_Q, "R": kVK_ANSI_R,
        "S": kVK_ANSI_S, "T": kVK_ANSI_T, "U": kVK_ANSI_U, "V": kVK_ANSI_V, "W": kVK_ANSI_W, "X": kVK_ANSI_X,
        "Y": kVK_ANSI_Y, "Z": kVK_ANSI_Z, "0": kVK_ANSI_0, "1": kVK_ANSI_1, "2": kVK_ANSI_2, "3": kVK_ANSI_3,
        "4": kVK_ANSI_4, "5": kVK_ANSI_5, "6": kVK_ANSI_6, "7": kVK_ANSI_7, "8": kVK_ANSI_8, "9": kVK_ANSI_9,
        "-": kVK_ANSI_Minus, "=": kVK_ANSI_Equal, "[": kVK_ANSI_LeftBracket, "]": kVK_ANSI_RightBracket,
        ";": kVK_ANSI_Semicolon, "'": kVK_ANSI_Quote, ",": kVK_ANSI_Comma, ".": kVK_ANSI_Period,
        "/": kVK_ANSI_Slash, "\\": kVK_ANSI_Backslash, "`": kVK_ANSI_Grave,
    ]
    private static let named: [String: Int] = [
        "Space": kVK_Space, "Enter": kVK_Return, "Escape": kVK_Escape, "Tab": kVK_Tab, "Backspace": kVK_Delete,
        "Delete": kVK_ForwardDelete, "ArrowUp": kVK_UpArrow, "ArrowDown": kVK_DownArrow, "ArrowLeft": kVK_LeftArrow,
        "ArrowRight": kVK_RightArrow, "Home": kVK_Home, "End": kVK_End, "PageUp": kVK_PageUp, "PageDown": kVK_PageDown,
        "F1": kVK_F1, "F2": kVK_F2, "F3": kVK_F3, "F4": kVK_F4, "F5": kVK_F5, "F6": kVK_F6, "F7": kVK_F7, "F8": kVK_F8,
        "F9": kVK_F9, "F10": kVK_F10, "F11": kVK_F11, "F12": kVK_F12, "F13": kVK_F13, "F14": kVK_F14, "F15": kVK_F15,
        "F16": kVK_F16, "F17": kVK_F17, "F18": kVK_F18, "F19": kVK_F19, "F20": kVK_F20,
    ]
    private static func code(_ key: String) -> Int? {
        if let code = named[key] { return code }
        guard key.count == 1, let c = key.uppercased().first else { return nil }
        return letters[c]
    }
}
