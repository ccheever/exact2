import AppKit
import Carbon

// Owns only this app's registration and recorder. Checking never asks for grants.
final class T3SnapshotShortcut {
    static let pairs: [String: [CGKeyCode]] = ["shift+shift": [56, 60], "meta+meta": [55, 54], "ctrl+ctrl": [59, 62], "alt+alt": [58, 61]]
    static func canonical(_ value: String) -> String? {
        let input = value.lowercased().trimmingCharacters(in: .whitespacesAndNewlines)
        let aliases = ["command":"meta", "cmd":"meta", "control":"ctrl", "option":"alt", "escape":"esc", "arrowleft":"left", "arrowright":"right", "arrowup":"up", "arrowdown":"down"]
        let tokens = input.split(separator: "+", omittingEmptySubsequences: false).map { aliases[String($0)] ?? String($0) }
        if input.hasSuffix("++") {
            let prefix = String(input.dropLast(2))
            guard let base = canonical(prefix + "+plus") else { return nil }; return base
        }
        if pairs[tokens.joined(separator: "+")] != nil { return tokens.joined(separator: "+") }
        guard let key = tokens.last, !key.isEmpty, tokens.count > 1 else { return nil }
        let modifiers = tokens.dropLast().map { $0 == "mod" ? "meta" : $0 }
        guard modifiers.allSatisfy({ ["meta", "ctrl", "alt", "shift"].contains($0) }), Set(modifiers).count == modifiers.count,
              !["meta", "ctrl", "alt", "shift"].contains(key) else { return nil }
        return (["meta", "ctrl", "alt", "shift"].filter { modifiers.contains($0) } + [key]).joined(separator: "+")
    }
    private static func key(_ event: NSEvent) -> String {
        let named: [UInt16: String] = [36:"enter",48:"tab",49:"space",51:"backspace",53:"esc",117:"delete",123:"left",124:"right",125:"down",126:"up",115:"home",119:"end",116:"pageup",121:"pagedown",122:"f1",120:"f2",99:"f3",118:"f4",96:"f5",97:"f6",98:"f7",100:"f8",101:"f9",109:"f10",103:"f11",111:"f12"]
        return named[event.keyCode] ?? event.charactersIgnoringModifiers?.lowercased() ?? ""
    }
    static func chord(_ text: String) -> (UInt32, UInt32)? {
        guard let canonical = canonical(text), pairs[canonical] == nil else { return nil }
        let tokens = canonical.split(separator: "+").map(String.init), target = tokens.last! == "plus" ? "+" : tokens.last!
        // Derive printable keys from the current keyboard layout, rather than US key codes.
        var code: UInt32?
        var required: UInt32 = 0
        for candidate in UInt16(0)...UInt16(126) {
            if let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: 0, context: nil, characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: candidate), key(event) == target { code = UInt32(candidate); break }
            let source = TISCopyCurrentKeyboardLayoutInputSource().takeRetainedValue()
            if let property = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) {
                let data = unsafeBitCast(property, to: CFData.self)
                let layout = UnsafePointer<UCKeyboardLayout>(OpaquePointer(CFDataGetBytePtr(data)))
                var dead: UInt32 = 0, length = 0; var characters = [UniChar](repeating: 0, count: 8)
                for modifier in [UInt32(0), UInt32(shiftKey >> 8)] {
                    dead = 0
                    if UCKeyTranslate(layout, candidate, UInt16(kUCKeyActionDown), modifier, UInt32(LMGetKbdType()), OptionBits(kUCKeyTranslateNoDeadKeysBit), &dead, 8, &length, &characters) == noErr,
                       String(utf16CodeUnits: characters, count: length).lowercased() == target { code = UInt32(candidate); required = modifier << 8; break }
                }
                if code != nil { break }
            }
        }
        guard let code else { return nil }
        var flags: UInt32 = required
        if tokens.contains("meta") { flags |= UInt32(cmdKey) }
        if tokens.contains("ctrl") { flags |= UInt32(controlKey) }
        if tokens.contains("alt") { flags |= UInt32(optionKey) }
        if tokens.contains("shift") { flags |= UInt32(shiftKey) }
        return (code, flags)
    }
    private var registration: EventHotKeyRef?
    private(set) var installed = ""
    private var handler: EventHandlerRef?
    private var monitor: Any?
    private var blur: NSObjectProtocol?
    private var fired: () -> Void = {}
    private let changed: () -> Void
    private(set) var recording = false
    private(set) var candidate = ""
    private(set) var error = ""
    private(set) var recordEpoch = 0
    init(changed: @escaping () -> Void) { self.changed = changed }
    func check(_ text: String, saved: String) -> String? {
        guard let text = Self.canonical(text) else { return "Add a modifier and a letter, number or function key." }
        if Self.pairs[text] != nil || (registration != nil && text == saved) { return nil }
        guard let (code, flags) = Self.chord(text) else { return "This key is unavailable in the current keyboard layout." }
        var probe: EventHotKeyRef?
        let result = RegisterEventHotKey(code, flags, EventHotKeyID(signature: 0x54335343, id: 2), GetApplicationEventTarget(), 0, &probe)
        if let probe { UnregisterEventHotKey(probe) }
        return result == noErr ? nil : "This shortcut is already used by the system or another app."
    }
    func install(_ text: String, fire: @escaping () -> Void) throws {
        guard let text = Self.canonical(text) else { throw T3Failure(kind: "SnapShot", message: "Choose a valid snapshot shortcut.") }
        if installed == text { fired = fire; return }
        guard Self.pairs[text] == nil else { stopRegistration(); fired = fire; installed = text; return }
        guard let (code, flags) = Self.chord(text) else { throw T3Failure(kind: "SnapShot", message: "This shortcut key is unavailable.") }
        var type = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        let callback: EventHandlerUPP = { _, event, context in
            guard let context, let event else { return OSStatus(eventNotHandledErr) }
            var id = EventHotKeyID()
            guard GetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID), nil, MemoryLayout<EventHotKeyID>.size, nil, &id) == noErr, id.signature == 0x54335343, id.id == 1 else { return OSStatus(eventNotHandledErr) }
            let owner = Unmanaged<T3SnapshotShortcut>.fromOpaque(context).takeUnretainedValue()
            if !owner.recording { owner.fired() }; return noErr
        }
        var newHandler: EventHandlerRef?
        if handler == nil && InstallEventHandler(GetApplicationEventTarget(), callback, 1, &type, Unmanaged.passUnretained(self).toOpaque(), &newHandler) != noErr {
            throw T3Failure(kind: "SnapShot", message: "Could not register the snapshot shortcut handler.")
        }
        var next: EventHotKeyRef?
        guard RegisterEventHotKey(code, flags, EventHotKeyID(signature: 0x54335343, id: 1), GetApplicationEventTarget(), 0, &next) == noErr else {
            if let newHandler { RemoveEventHandler(newHandler) }
            throw T3Failure(kind: "SnapShot", message: "This shortcut is already used by the system or another app.")
        }
        if let registration { UnregisterEventHotKey(registration) }
        registration = next; if let newHandler { handler = newHandler }; fired = fire; installed = text
    }

    func startRecording() {
        // Reference recorder: a second press while recording keeps the session.
        guard !recording else { return }
        cancel(); recording = true; candidate = ""; error = ""; recordEpoch += 1
        let epoch = recordEpoch
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .flagsChanged]) { [weak self] event in
            guard let self, self.recording, event.window?.isKeyWindow == true else { return event }
            if let input = event.window?.firstResponder as? NSTextView, input.hasMarkedText() { return event }
            if event.type == .keyDown && event.keyCode == 53 { self.cancel(); return nil }
            if event.type == .flagsChanged {
                if let pair = Self.pairs.first(where: { $0.value.allSatisfy { CGEventSource.keyState(.combinedSessionState, key: $0) } }) { self.finish(pair.key); return nil }
                return event
            }
            if event.isARepeat || event.keyCode == 48 { return event }
            let flags = event.modifierFlags
            var tokens: [String] = []
            if flags.contains(.command) { tokens.append("meta") }; if flags.contains(.control) { tokens.append("ctrl") }
            if flags.contains(.option) { tokens.append("alt") }; if flags.contains(.shift) { tokens.append("shift") }
            guard !tokens.isEmpty else { return nil }
            tokens.append(Self.key(event)); if let next = Self.canonical(tokens.joined(separator: "+")) { self.finish(next) }
            return nil
        }
        blur = NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification, object: nil, queue: .main) { [weak self] _ in self?.cancel() }
        changed()
        DispatchQueue.main.asyncAfter(deadline: .now() + 30) { [weak self] in self?.expireRecording(epoch: epoch) }
    }
    func expireRecording(epoch: Int) { if recordEpoch == epoch { cancel() } }
    private func finish(_ text: String) { cancel(); candidate = text; changed() }
    func cancel() { recordEpoch += 1; recording = false; candidate = ""; if let monitor { NSEvent.removeMonitor(monitor) }; monitor = nil; if let blur { NotificationCenter.default.removeObserver(blur) }; blur = nil; changed() }
    func stopRegistration() { if let registration { UnregisterEventHotKey(registration) }; registration = nil; installed = ""; if let handler { RemoveEventHandler(handler) }; handler = nil }
    func destroy() { cancel(); stopRegistration() }
}
