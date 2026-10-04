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
        kVK_Home: "Home", kVK_End: "End", kVK_PageUp: "PageUp", kVK_PageDown: "PageDown", kVK_ForwardDelete: "Delete",
        kVK_CapsLock: "CapsLock", kVK_ANSI_KeypadEnter: "NumpadEnter",
    ]
    #endif

    /// UIKeyboardHIDUsage's USB keyboard page. Letters, digits and F1–F12 are contiguous.
    static func hid(_ usage: Int) -> String {
        if (4...29).contains(usage) { return "Key" + String(UnicodeScalar(65 + usage - 4)!) }
        if (30...38).contains(usage) { return "Digit\(usage - 29)" }
        if (58...69).contains(usage) { return "F\(usage - 57)" }
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
            || (code.hasPrefix("F") && Int(code.dropFirst()) != nil)
    }
    static func device(_ name: String) -> (code: String, key: String)? {
        var code = name
        if ["Shift", "Control", "Alt", "Meta"].contains(name) { code += "Left" }
        let known = (4...100).map(hid) + (224...231).map(hid)
        guard code != "Unidentified", known.contains(code) else { return nil }
        return (code, key(code))
    }
}
