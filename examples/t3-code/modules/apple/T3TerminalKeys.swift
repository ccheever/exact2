#if os(macOS)
import AppKit

/// Key events for the agent's input to a terminal (T3TerminalView.agentInput): the agent's
/// web-named chords (`Meta+K`, `Control+C`, `ArrowUp`) and typed characters, as the US layout's
/// key codes and the characters AppKit would report.
enum T3TerminalKeys {
    struct Key { let code: UInt16; let characters: String; let plain: String; let flags: NSEvent.ModifierFlags }
    struct Binding {
        let chord: String, command: String
        let key: Key
        func matches(_ event: NSEvent) -> Bool {
            guard event.modifierFlags.intersection([.command, .control, .option, .shift]) == key.flags.intersection([.command, .control, .option, .shift]) else { return false }
            // Preserve Latin remappings (Dvorak/Colemak); fall back to physical positions only
            // for non-Latin text and Option's symbols, as keybindings.resolveEventKeys does.
            let text = (event.modifierFlags.contains(.option) ? event.characters : event.charactersIgnoringModifiers)?.lowercased() ?? ""
            if text.count == 1, let scalar = text.unicodeScalars.first, (97...122).contains(scalar.value),
               key.plain.count == 1, key.plain.first?.isLetter == true { return text == key.plain.lowercased() }
            return event.keyCode == key.code
        }
    }

    static func bindings(_ json: String?) -> [Binding] {
        guard let data = json?.data(using: .utf8), let entries = (try? JSONSerialization.jsonObject(with: data)) as? [[String: String]] else { return [] }
        return entries.compactMap { entry in
            guard let text = entry["chord"], let command = entry["command"], !command.isEmpty, let key = chord(text) else { return nil }
            return Binding(chord: text, command: command, key: key)
        }
    }

    /// Ghostty's macOS paste shortcuts are Command-V and Command-Shift-V. Read the
    /// native pasteboard before WebKit attempts synchronous DOM paste permission IPC.
    static func isPaste(_ event: NSEvent) -> Bool {
        let text = event.modifierFlags.contains(.option) ? event.characters : event.charactersIgnoringModifiers
        return event.modifierFlags.contains(.command) && text?.lowercased() == "v"
    }

    /// AppKit must not turn the terminal's readline keys into menu commands. These are the
    /// same data as ThreadTerminalDrawer.handleBeforeKey, matched by physical key for Korean.
    static func input(_ event: NSEvent) -> String? {
        let flags = event.modifierFlags.intersection([.command, .control, .option, .shift])
        if let clear = chord("Meta+K"), Binding(chord: "Meta+K", command: "", key: clear).matches(event) { return "\u{c}" }
        if let clear = chord("Control+L"), Binding(chord: "Control+L", command: "", key: clear).matches(event) { return "\u{c}" }
        switch (event.keyCode, flags) {
        case (123, .option): return "\u{1b}b"
        case (124, .option): return "\u{1b}f"
        case (123, .command): return "\u{1}"
        case (124, .command): return "\u{5}"
        case (51, .command): return "\u{15}"
        default: return nil
        }
    }

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

/// The same command buttons handle menu/keyboard/click actions. A terminal's native key policy
/// reaches them directly instead of reimplementing global UI commands in a second dispatcher.
enum T3TerminalCommandKey {
    private final class Target {
        weak var element: ExactElement?
        init(_ element: ExactElement) { self.element = element }
    }
    private static var targets: [ObjectIdentifier: Target] = [:]
    static func install(_ element: ExactElement) {
        guard element.hatch == .t3TerminalKey else { return }
        targets[ObjectIdentifier(element)] = Target(element)
    }
    static func remove(_ element: ExactElement) { targets[ObjectIdentifier(element)] = nil }
    static func click(command: String, window: NSWindow?) -> Bool {
        guard let window else { return false }
        for (id, target) in targets {
            guard let element = target.element else { targets[id] = nil; continue }
            guard element.isLive, element.data[.anchor] == command, element.view?.window === window else { continue }
            element.click()
            return true
        }
        return false
    }
}

/// Development harness bytes for a `t3-terminal` view's `fixture` prop (spike S2–S5): `render`
/// draws colours, wide and emoji cells, Nerd glyphs, an alternate-screen round trip and 2,000
/// scrollback lines; `loopback` echoes typed bytes back as a line discipline would (Return as
/// CR LF, Backspace erases); `flood` writes 5 MB of numbered lines.
enum T3TerminalFixtures {
    private static var loopbacks: Set<ObjectIdentifier> = []

    static func start(_ name: String, on view: T3TerminalView) {
        switch name {
        case "render": view.resetAndWrite(render)
        case "loopback":
            view.resetAndWrite("\u{1b}[1mT3 terminal harness\u{1b}[0m (loopback)\r\n$ ")
            loopbacks.insert(ObjectIdentifier(view))
            view.onData = { [weak view] data in
                guard let view else { return }
                view.write(data.replacingOccurrences(of: "\r", with: "\r\n$ ").replacingOccurrences(of: "\u{7f}", with: "\u{8} \u{8}"))
            }
        case "flood": view.resetAndWrite(""); flood(view)
        default: break
        }
    }

    static func stop(_ view: T3TerminalView) { loopbacks.remove(ObjectIdentifier(view)); view.onData = nil }

    static var render: String {
        var out = ""
        for line in 1...2000 { out += "scrollback line \(line)\r\n" }
        out += "\u{1b}[?1049h\u{1b}[2J\u{1b}[Halternate screen\u{1b}[?1049l"
        out += "\u{1b}[1mbold\u{1b}[0m \u{1b}[3mitalic\u{1b}[0m \u{1b}[4munderline\u{1b}[0m \u{1b}[9mstrike\u{1b}[0m\r\n"
        out += (0..<16).map { "\u{1b}[48;5;\($0)m  " }.joined() + "\u{1b}[0m\r\n"
        out += (0..<24).map { "\u{1b}[38;2;\(255 - $0 * 10);\($0 * 10);128m█" }.joined() + "\u{1b}[0m truecolor\r\n"
        out += "wide: 界面 한국어 emoji: 🙂🚀 nerd: \u{e0b0}\u{e0a0}\u{f07b}\u{f121}\r\n"
        out += "\u{1b}]8;;https://t3.codes\u{1b}\\link\u{1b}]8;;\u{1b}\\ path: src/main.ts:12:3\r\n$ "
        return out
    }

    /// 5 MB in 64 KB writes on the main queue, each line numbered so a dropped byte shows.
    static func flood(_ view: T3TerminalView, total: Int = 5 * 1024 * 1024) {
        var line = 0, written = 0
        func step() {
            guard written < total else { view.write("\r\nflood done \(line) lines \(written) bytes\r\n"); return }
            var chunk = ""
            while chunk.utf8.count < 64 * 1024 {
                line += 1
                chunk += "flood \(String(format: "%07d", line)) " + String(repeating: "x", count: 48) + "\r\n"
            }
            written += chunk.utf8.count
            view.write(chunk)
            DispatchQueue.main.async(execute: step)
        }
        step()
    }
}
#endif
