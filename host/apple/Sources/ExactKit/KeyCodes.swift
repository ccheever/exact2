// Hardware positions, not layout-dependent characters: KeyboardEvent.code.
import Foundation
#if os(macOS)
import Carbon.HIToolbox
#endif

enum KeyCodes {
    #if os(macOS)
    static let mac: [Int: String] = [
        kVK_ANSI_A: "KeyA", kVK_ANSI_B: "KeyB", kVK_ANSI_C: "KeyC", kVK_ANSI_D: "KeyD",
        kVK_ANSI_E: "KeyE", kVK_ANSI_F: "KeyF", kVK_ANSI_G: "KeyG", kVK_ANSI_H: "KeyH",
        kVK_ANSI_I: "KeyI", kVK_ANSI_J: "KeyJ", kVK_ANSI_K: "KeyK", kVK_ANSI_L: "KeyL",
        kVK_ANSI_M: "KeyM", kVK_ANSI_N: "KeyN", kVK_ANSI_O: "KeyO", kVK_ANSI_P: "KeyP",
        kVK_ANSI_Q: "KeyQ", kVK_ANSI_R: "KeyR", kVK_ANSI_S: "KeyS", kVK_ANSI_T: "KeyT",
        kVK_ANSI_U: "KeyU", kVK_ANSI_V: "KeyV", kVK_ANSI_W: "KeyW", kVK_ANSI_X: "KeyX",
        kVK_ANSI_Y: "KeyY", kVK_ANSI_Z: "KeyZ",
        kVK_ANSI_0: "Digit0", kVK_ANSI_1: "Digit1", kVK_ANSI_2: "Digit2", kVK_ANSI_3: "Digit3",
        kVK_ANSI_4: "Digit4", kVK_ANSI_5: "Digit5", kVK_ANSI_6: "Digit6", kVK_ANSI_7: "Digit7",
        kVK_ANSI_8: "Digit8", kVK_ANSI_9: "Digit9",
        kVK_LeftArrow: "ArrowLeft", kVK_RightArrow: "ArrowRight", kVK_UpArrow: "ArrowUp", kVK_DownArrow: "ArrowDown",
        kVK_Space: "Space", kVK_Return: "Enter", kVK_Escape: "Escape", kVK_Tab: "Tab", kVK_Delete: "Backspace",
        kVK_Shift: "ShiftLeft", kVK_RightShift: "ShiftRight", kVK_Control: "ControlLeft", kVK_RightControl: "ControlRight",
        kVK_Option: "AltLeft", kVK_RightOption: "AltRight", kVK_Command: "MetaLeft", kVK_RightCommand: "MetaRight",
        kVK_ANSI_Minus: "Minus", kVK_ANSI_Equal: "Equal", kVK_ANSI_LeftBracket: "BracketLeft", kVK_ANSI_RightBracket: "BracketRight",
        kVK_ANSI_Backslash: "Backslash", kVK_ANSI_Semicolon: "Semicolon", kVK_ANSI_Quote: "Quote", kVK_ANSI_Grave: "Backquote",
        kVK_ANSI_Comma: "Comma", kVK_ANSI_Period: "Period", kVK_ANSI_Slash: "Slash", kVK_ISO_Section: "IntlBackslash",
        kVK_F1: "F1", kVK_F2: "F2", kVK_F3: "F3", kVK_F4: "F4", kVK_F5: "F5", kVK_F6: "F6",
        kVK_F7: "F7", kVK_F8: "F8", kVK_F9: "F9", kVK_F10: "F10", kVK_F11: "F11", kVK_F12: "F12",
        kVK_F13: "F13", kVK_F14: "F14", kVK_F15: "F15", kVK_F16: "F16", kVK_F17: "F17", kVK_F18: "F18",
        kVK_F19: "F19", kVK_F20: "F20",
        kVK_Home: "Home", kVK_End: "End", kVK_PageUp: "PageUp", kVK_PageDown: "PageDown", kVK_ForwardDelete: "Delete",
        kVK_CapsLock: "CapsLock", kVK_ANSI_KeypadEnter: "NumpadEnter",
    ]

    /// What the current ASCII-capable keyboard layout types on a key, with
    /// Shift and Option as given: the Latin character of a key whose input
    /// source types none (Korean 2-Set, Russian), as an app's shortcut hears it.
    static func asciiCharacters(_ keyCode: UInt16, shift: Bool, option: Bool) -> String? {
        guard let source = TISCopyCurrentASCIICapableKeyboardLayoutInputSource()?.takeRetainedValue(),
              let property = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData),
              let bytes = CFDataGetBytePtr(Unmanaged<CFData>.fromOpaque(property).takeUnretainedValue()) else { return nil }
        let state = ((shift ? shiftKey : 0) | (option ? optionKey : 0)) >> 8
        var dead: UInt32 = 0, length = 0
        var characters = [UniChar](repeating: 0, count: 4)
        let status = bytes.withMemoryRebound(to: UCKeyboardLayout.self, capacity: 1) {
            UCKeyTranslate($0, keyCode, UInt16(kUCKeyActionDown), UInt32(state & 0xff), UInt32(LMGetKbdType()),
                           OptionBits(kUCKeyTranslateNoDeadKeysMask), &dead, characters.count, &length, &characters)
        }
        return status == noErr && length > 0 ? String(utf16CodeUnits: characters, count: length) : nil
    }
    #endif

    /// UIKeyboardHIDUsage's USB keyboard page. Letters, digits, F1–F12 and
    /// F13–F24 are contiguous.
    static func hid(_ usage: Int) -> String {
        if (4...29).contains(usage) { return "Key" + String(UnicodeScalar(65 + usage - 4)!) }
        if (30...38).contains(usage) { return "Digit\(usage - 29)" }
        if (58...69).contains(usage) { return "F\(usage - 57)" }
        if (104...115).contains(usage) { return "F\(usage - 91)" }
        return [39: "Digit0", 40: "Enter", 41: "Escape", 42: "Backspace", 43: "Tab", 44: "Space",
                45: "Minus", 46: "Equal", 47: "BracketLeft", 48: "BracketRight", 49: "Backslash", 50: "IntlHash",
                51: "Semicolon", 52: "Quote", 53: "Backquote", 54: "Comma", 55: "Period", 56: "Slash", 57: "CapsLock",
                73: "Insert", 74: "Home", 75: "PageUp", 76: "Delete", 77: "End", 78: "PageDown",
                79: "ArrowRight", 80: "ArrowLeft", 81: "ArrowDown", 82: "ArrowUp", 88: "NumpadEnter", 100: "IntlBackslash",
                224: "ControlLeft", 225: "ShiftLeft", 226: "AltLeft", 227: "MetaLeft",
                228: "ControlRight", 229: "ShiftRight", 230: "AltRight", 231: "MetaRight"][usage] ?? "Unidentified"
    }

    static let characters = ["Space": " ", "Minus": "-", "Equal": "=", "BracketLeft": "[", "BracketRight": "]",
                             "Backslash": "\\", "Semicolon": ";", "Quote": "'", "Backquote": "`", "Comma": ",", "Period": ".", "Slash": "/"]
    static func key(_ code: String) -> String {
        if code.hasPrefix("Key"), code.count == 4 { return String(code.suffix(1)).lowercased() }
        if code.hasPrefix("Digit"), code.count == 6 { return String(code.suffix(1)) }
        for modifier in ["Shift", "Control", "Alt", "Meta"] where code == modifier + "Left" || code == modifier + "Right" { return modifier }
        return characters[code] ?? (code == "NumpadEnter" ? "Enter" : code)
    }
    /// A key whose `KeyboardEvent.key` is a name, not the character it types.
    static func named(_ code: String) -> Bool {
        ["Enter", "NumpadEnter", "Escape", "Tab", "Backspace", "Delete", "Insert", "Home", "End", "PageUp", "PageDown",
         "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "CapsLock"].contains(code)
            || (code.hasPrefix("F") && Int(code.dropFirst()) != nil) || modifier(code)
    }
    /// Shift, Control, Alt or Meta, either side: a key that types nothing.
    static func modifier(_ code: String) -> Bool {
        ["Shift", "Control", "Alt", "Meta"].contains { code == $0 + "Left" || code == $0 + "Right" }
    }
    /// The US punctuation `cdpKey` accepts as a key, by the character it
    /// types, a shifted one on its key as Linux's `driver_key` has it (`!` is
    /// Digit1); the character is what the key types and what `key` hears.
    private static let punctuation = ["-": "Minus", "=": "Equal", "[": "BracketLeft", "]": "BracketRight",
                                      "\\": "Backslash", ";": "Semicolon", "'": "Quote", "`": "Backquote",
                                      ",": "Comma", ".": "Period", "/": "Slash", "+": "Equal",
                                      "_": "Minus", "{": "BracketLeft", "}": "BracketRight", "|": "Backslash",
                                      ":": "Semicolon", "\"": "Quote", "~": "Backquote", "<": "Comma", ">": "Period",
                                      "?": "Slash", "!": "Digit1", "@": "Digit2", "#": "Digit3", "$": "Digit4",
                                      "%": "Digit5", "^": "Digit6", "&": "Digit7", "*": "Digit8", "(": "Digit9", ")": "Digit0"]
    /// A driver's key name as its `KeyboardEvent.code`. One vocabulary on
    /// every host (scripts/agent-keys.mjs `cdpKey`): `p` and `KeyP` are the
    /// same key, `7` and `Digit7` too, and `End` is a named key, not the
    /// letters e-n-d (notes mac-agent-named-keys, platformer canvas-keys).
    static func codeName(_ name: String) -> String {
        if ["Shift", "Control", "Alt", "Meta"].contains(name) { return name + "Left" }
        if name.count == 1, let c = name.first, c.isASCII, c.isLetter { return "Key" + name.uppercased() }
        if name.count == 1, let c = name.first, c.isASCII, c.isNumber { return "Digit" + name }
        if name == " " { return "Space" }
        return punctuation[name] ?? name
    }
    /// What an agent key types. A named key is its AppKit function character
    /// so a field moves the caret instead of inserting the key's name; a
    /// letter keeps the case the driver named; a lone modifier types nothing.
    static func eventText(code: String, raw: String, lone: Bool) -> String {
        if lone { return "" }
        if let text = functionCharacter(code) { return text }
        if raw.count == 1, raw != " " { return raw }
        return key(code)
    }
    /// AppKit's function-key characters (NSUpArrowFunctionKey is U+F700,
    /// NSF1FunctionKey U+F704, NSHomeFunctionKey U+F729).
    static func functionCharacter(_ code: String) -> String? {
        let named = ["ArrowUp": "\u{F700}", "ArrowDown": "\u{F701}", "ArrowLeft": "\u{F702}", "ArrowRight": "\u{F703}",
                     "Insert": "\u{F727}", "Delete": "\u{F728}", "Home": "\u{F729}", "End": "\u{F72B}",
                     "PageUp": "\u{F72C}", "PageDown": "\u{F72D}", "Enter": "\r", "NumpadEnter": "\r",
                     "Escape": "\u{1b}", "Tab": "\t", "Backspace": "\u{7f}", "Space": " ", "CapsLock": ""]
        if let text = named[code] { return text }
        if code.hasPrefix("F"), let n = Int(code.dropFirst()), (1...35).contains(n) {
            return String(UnicodeScalar(0xF703 + n)!)
        }
        return nil
    }
    /// A driver's chord (`Shift+Enter`, `Meta+s`, `+`, `Shift++`) as
    /// `Event::key` reads one: the modifiers' chord prefix, and the key.
    static func split(_ chord: String) -> (held: String, key: String) {
        var held: Set<Substring> = []
        var rest = Substring(chord)
        while let plus = rest.firstIndex(of: "+"), rest.index(after: plus) < rest.endIndex,
              ["Shift", "Control", "Alt", "Meta"].contains(rest[..<plus]) {
            held.insert(rest[..<plus])
            rest = rest[rest.index(after: plus)...]
        }
        return (self.held(shift: held.contains("Shift"), control: held.contains("Control"), alt: held.contains("Alt"), meta: held.contains("Meta")), String(rest))
    }
    /// A chord's modifiers in the order it names them, each with the prefix
    /// held as it goes down, its own included: `Shift+Control+x` is Shift
    /// (`Shift+`), then Control (`Shift+Control+`). Each is its own keydown
    /// before the key, as a keyboard's is (Charlie, 2026-10-07; the web
    /// driver's `modifierEdges`).
    static func modifierPresses(_ chord: String) -> [(key: String, held: String)] {
        var on: Set<Substring> = [], presses: [(key: String, held: String)] = []
        var rest = Substring(chord)
        while let plus = rest.firstIndex(of: "+"), rest.index(after: plus) < rest.endIndex,
              ["Shift", "Control", "Alt", "Meta"].contains(rest[..<plus]) {
            on.insert(rest[..<plus])
            presses.append((String(rest[..<plus]), held(shift: on.contains("Shift"), control: on.contains("Control"), alt: on.contains("Alt"), meta: on.contains("Meta"))))
            rest = rest[rest.index(after: plus)...]
        }
        return presses
    }
    static func device(_ name: String) -> (code: String, key: String)? {
        let code = codeName(name)
        // F13–F24 as the web's driver takes them (CDP has every one).
        let known = (4...100).map(hid) + (104...115).map(hid) + (224...231).map(hid)
        guard code != "Unidentified", known.contains(code) else { return nil }
        return (code, key(code))
    }
}
