// App-declared aria-keyshortcuts on iPadOS (gallery F18): a hardware
// keyboard's chord presses the button that declares it, as on the web and
// macOS (ShortcutsMac). The session's view offers one UIKeyCommand per
// chord, and takes the focus when nothing else holds it so the commands are
// heard; a text field or area keeps its typing (only a Command or Control
// chord, or Escape, is a shortcut there, as on the web), and the scope rule
// is every host's (`Shortcuts.swift`). The agent's `type <id> key …` reaches
// the same buttons (AgentIOS).
#if os(iOS)
import UIKit

private struct ChordIOS {
    /// The web's key name, and the modifiers as `KeyCodes.held` writes them.
    let key: String, held: String
    /// UIKit's input for the chord's key command; `nil` for F13–F24, which
    /// UIKit names no input for (it has F1–F12): their buttons take the
    /// driver's keys (AgentIOS), and a hardware keyboard's reach `key`
    /// handlers only (a declared deviation, docs/contract-grammar.md).
    let input: String?, flags: UIKeyModifierFlags

    private static let named: [String: String] = [
        "Enter": "\r", "Tab": "\t", "Escape": UIKeyCommand.inputEscape, "Space": " ", "Backspace": "\u{8}",
        "Delete": UIKeyCommand.inputDelete, "ArrowUp": UIKeyCommand.inputUpArrow, "ArrowDown": UIKeyCommand.inputDownArrow,
        "ArrowLeft": UIKeyCommand.inputLeftArrow, "ArrowRight": UIKeyCommand.inputRightArrow, "Home": UIKeyCommand.inputHome,
        "End": UIKeyCommand.inputEnd, "PageUp": UIKeyCommand.inputPageUp, "PageDown": UIKeyCommand.inputPageDown, "Plus": "+",
        "F1": UIKeyCommand.f1, "F2": UIKeyCommand.f2, "F3": UIKeyCommand.f3, "F4": UIKeyCommand.f4, "F5": UIKeyCommand.f5, "F6": UIKeyCommand.f6,
        "F7": UIKeyCommand.f7, "F8": UIKeyCommand.f8, "F9": UIKeyCommand.f9, "F10": UIKeyCommand.f10, "F11": UIKeyCommand.f11, "F12": UIKeyCommand.f12,
    ]

    /// A chord as ARIA spells one (`Shift+ArrowRight`, `Meta+=`, `Meta++`).
    init?(_ text: Substring) {
        var parts = text.split(separator: "+", omittingEmptySubsequences: false)
        let last: Substring
        if text == "+" { last = "Plus"; parts.removeAll() }
        else if parts.count >= 3, parts.suffix(2).allSatisfy(\.isEmpty) { last = "Plus"; parts.removeLast(2) }
        else if let part = parts.popLast() { last = part } else { return nil }
        let function = Int(last.dropFirst()).map { (13...24).contains($0) && last == "F\($0)" } == true
        guard last.count == 1 || Self.named[String(last)] != nil || function else { return nil }
        var flags: UIKeyModifierFlags = []
        for part in parts {
            switch part {
            case "Meta": flags.insert(.command)
            case "Control": flags.insert(.control)
            case "Alt": flags.insert(.alternate)
            case "Shift": flags.insert(.shift)
            default: return nil
            }
        }
        key = last == "Plus" ? "+" : last == "Space" ? " " : last.count == 1 ? last.lowercased() : String(last)
        input = last.count == 1 ? last.lowercased() : Self.named[String(last)]
        self.flags = flags
        held = KeyCodes.held(flags)
    }

    func matches(key name: String, held: String) -> Bool {
        held == self.held && (name.count == 1 ? name.lowercased() == key : name == key)
    }
}

extension Presenter {
    private func chords(_ view: NodeView) -> [ChordIOS] {
        (view.props["accessibilityKeyShortcuts"] ?? "").split(whereSeparator: \.isWhitespace).compactMap(ChordIOS.init)
    }
    /// The buttons that declare a chord and can be pressed now.
    private var shortcutButtons: [NodeView] {
        chrome.ids("accessibilityKeyShortcuts").sorted().compactMap { views[$0] }
            .filter { $0.isButton && $0.handlers.contains("press") && !$0.disabled && $0.accessibilityVisible }
    }
    /// Whether the focus is a text field or area, whose typing a plain key is.
    private var editingText: Bool { focusedNode.map { $0.field != nil || $0.textArea != nil } ?? false }

    /// The button a key (the web's name, modifiers as `KeyCodes.held`)
    /// presses as its shortcut, the focus being `focus`; nil when none takes it.
    func shortcut(key: String, held: String, focus: NodeView?) -> NodeView? {
        if editingText && key != "Escape" && !held.contains("Meta+") && !held.contains("Control+") { return nil }
        return shortcutButtons.first { node in
            chords(node).contains { $0.matches(key: key, held: held) } && shortcutAdmits(node, key: key, held: held, focus: focus)
        }
    }

    /// One command per chord the buttons declare, those a text field's
    /// typing would take left out while one has the focus.
    func shortcutCommands(_ action: Selector) -> [UIKeyCommand] {
        var seen = Set<String>(), commands: [UIKeyCommand] = []
        for node in shortcutButtons {
            for chord in chords(node) {
                guard let input = chord.input, seen.insert(chord.held + input).inserted else { continue }
                if editingText && chord.key != "Escape" && chord.flags.isDisjoint(with: [.command, .control]) { continue }
                let command = UIKeyCommand(input: input, modifierFlags: chord.flags, action: action)
                // A shortcut on an arrow or Escape is the app's, not the system's scrolling or dismissal.
                command.wantsPriorityOverSystemBehavior = true
                commands.append(command)
            }
        }
        return commands
    }

    /// A key command's chord, pressing its button (once; a repeat is not a press).
    func performShortcut(_ command: UIKeyCommand) {
        guard let input = command.input else { return }
        let held = KeyCodes.held(command.modifierFlags)
        // Every input is named back to ARIA's key first, a one-character one
        // too (`"\r"` is Enter, review B2); UIKit gives Backspace and Delete
        // the same input, so either name a button declares is its key.
        let names = ChordIOS.names(of: input)
        for name in names.isEmpty ? [input] : names {
            guard let chord = ChordIOS(Substring(name)) else { continue }
            if let node = shortcut(key: chord.key, held: held, focus: focusedNode) { press(node.id); return }
        }
    }
}

private extension ChordIOS {
    /// The ARIA names of a key command's input (`UIKeyCommand.inputEscape` → `Escape`), in name order.
    static func names(of input: String) -> [String] { named.filter { $0.value == input }.map(\.key).sorted() }
}
#endif
