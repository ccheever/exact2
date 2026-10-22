#if os(macOS)
import AppKit

/// Key events for the agent's input to a Browser page (T3BrowserView.agentInput) and the Browser
/// automation's key presses (T3BrowserAutomationInput): the agent's web-named chords (`Meta+K`,
/// `Control+C`, `ArrowUp`) and typed characters, as the US layout's key codes and the characters
/// AppKit would report.
enum T3Keys {
    struct Key { let code: UInt16; let characters: String; let plain: String; let flags: NSEvent.ModifierFlags }

    private static let letters: [Character: UInt16] = [
        "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9, "b": 11, "q": 12, "w": 13, "e": 14, "r": 15,
        "y": 16, "t": 17, "1": 18, "2": 19, "3": 20, "4": 21, "6": 22, "5": 23, "=": 24, "9": 25, "7": 26, "-": 27, "8": 28, "0": 29,
        "]": 30, "o": 31, "u": 32, "[": 33, "i": 34, "p": 35, "l": 37, "j": 38, "'": 39, "k": 40, ";": 41, "\\": 42, ",": 43, "/": 44,
        "n": 45, "m": 46, ".": 47, "`": 50, " ": 49,
    ]
    private static let shifted: [Character: Character] = [
        "!": "1", "@": "2", "#": "3", "$": "4", "%": "5", "^": "6", "&": "7", "*": "8", "(": "9", ")": "0", "_": "-", "+": "=",
        "{": "[", "}": "]", "|": "\\", ":": ";", "\"": "'", "<": ",", ">": ".", "?": "/", "~": "`",
    ]
    private static func function(_ scalar: Int) -> String { String(UnicodeScalar(UInt32(scalar)).map(Character.init) ?? " ") }
    private static let named: [String: (UInt16, String)] = [
        "Enter": (36, "\r"), "Tab": (48, "\t"), "Escape": (53, "\u{1b}"), "Backspace": (51, "\u{7f}"), "Space": (49, " "),
        "Delete": (117, function(NSDeleteFunctionKey)), "ArrowUp": (126, function(NSUpArrowFunctionKey)),
        "ArrowDown": (125, function(NSDownArrowFunctionKey)), "ArrowLeft": (123, function(NSLeftArrowFunctionKey)),
        "ArrowRight": (124, function(NSRightArrowFunctionKey)), "Home": (115, function(NSHomeFunctionKey)),
        "End": (119, function(NSEndFunctionKey)), "PageUp": (116, function(NSPageUpFunctionKey)),
        "PageDown": (121, function(NSPageDownFunctionKey)), "Backquote": (50, "`"), "Plus": (24, "+"),
        "F1": (122, function(NSF1FunctionKey)), "F2": (120, function(NSF2FunctionKey)), "F3": (99, function(NSF3FunctionKey)),
        "F4": (118, function(NSF4FunctionKey)), "F5": (96, function(NSF5FunctionKey)), "F6": (97, function(NSF6FunctionKey)),
        "F7": (98, function(NSF7FunctionKey)), "F8": (100, function(NSF8FunctionKey)), "F9": (101, function(NSF9FunctionKey)),
        "F10": (109, function(NSF10FunctionKey)), "F11": (103, function(NSF11FunctionKey)), "F12": (111, function(NSF12FunctionKey)),
    ]

    /// A typed character: its key on the US layout (Shift for the shifted ones), or key code 0 with
    /// the text itself for anything the layout has no key for (it arrives as inserted text).
    static func event(for character: Character) -> Key {
        let lower = Character(character.lowercased())
        if let code = letters[lower] {
            let shift = character != lower
            return Key(code: code, characters: String(character), plain: String(lower), flags: shift ? [.shift] : [])
        }
        if let base = shifted[character], let code = letters[base] {
            return Key(code: code, characters: String(character), plain: String(base), flags: [.shift])
        }
        if character == "\n" || character == "\r" { return Key(code: 36, characters: "\r", plain: "\r", flags: []) }
        return Key(code: 0, characters: String(character), plain: String(character), flags: [])
    }

    /// `Meta+Shift+K` → ⌘⇧K on the K key. AppKit reports ⌘ chords' characters unshifted by ⌘,
    /// and a ⌃ letter as its control character.
    static func chord(_ chord: String) -> Key? {
        var parts = chord.split(separator: "+", omittingEmptySubsequences: false).map(String.init)
        if chord.hasSuffix("++") { parts.removeLast(2); parts.append("Plus") }
        guard let last = parts.popLast(), !last.isEmpty else { return nil }
        var flags: NSEvent.ModifierFlags = []
        for modifier in parts {
            switch modifier {
            case "Meta": flags.insert(.command)
            case "Control": flags.insert(.control)
            case "Alt": flags.insert(.option)
            case "Shift": flags.insert(.shift)
            default: return nil
            }
        }
        if let (code, characters) = named[last] {
            if [123, 124, 125, 126, 115, 119, 116, 121, 117].contains(code) { flags.insert([.function, .numericPad]) }
            return Key(code: code, characters: characters, plain: characters, flags: flags)
        }
        guard last.count == 1, let character = last.lowercased().first else { return nil }
        guard let code = letters[character] ?? shifted[character].flatMap({ letters[$0] }) else { return nil }
        let plain = String(character)
        var characters = flags.contains(.shift) ? plain.uppercased() : plain
        if flags.contains(.control), let ascii = character.asciiValue, character.isLetter {
            characters = String(UnicodeScalar(ascii - 96))
        }
        return Key(code: code, characters: characters, plain: plain, flags: flags)
    }
}
#endif
