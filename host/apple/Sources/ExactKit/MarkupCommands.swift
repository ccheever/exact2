// The Markdown command seam carries source edits and UTF-16 selections.
// @ref LLP 1045 D2, D6 — native undo owns edits; the runner sees source and
// compact toolbar state, never a range or attributed document.
import Foundation
import CExact

struct MarkupEdit {
    let source: String
    let selection: NSRange

    init?(source: String, object: [String: Any]) {
        guard let replacements = object["replacements"] as? [[Any]],
              let selection = object["selection"] as? [Int], selection.count == 2 else { return nil }
        let original = source as NSString
        var edits: [(NSRange, String)] = []
        var end = 0
        for item in replacements {
            guard item.count == 3, let a = item[0] as? Int, let b = item[1] as? Int,
                  let text = item[2] as? String, a >= end, b >= a, b <= original.length,
                  Self.isBoundary(a, in: original), Self.isBoundary(b, in: original) else { return nil }
            edits.append((NSRange(location: a, length: b - a), text))
            end = b
        }
        let result = NSMutableString(string: source)
        for (range, text) in edits.reversed() { result.replaceCharacters(in: range, with: text) }
        guard selection[0] >= 0, selection[1] >= selection[0], selection[1] <= result.length,
              Self.isBoundary(selection[0], in: result), Self.isBoundary(selection[1], in: result) else { return nil }
        self.source = result as String
        self.selection = NSRange(location: selection[0], length: selection[1] - selection[0])
    }

    private static func isBoundary(_ offset: Int, in source: NSString) -> Bool {
        offset == 0 || offset == source.length || !UTF16.isTrailSurrogate(source.character(at: offset))
    }
}

enum MarkupCommands {
    private static func read(_ call: (UnsafeMutablePointer<UnsafePointer<UInt8>?>, UnsafeMutablePointer<Int>) -> UInt64) -> Data? {
        var json: UnsafePointer<UInt8>?
        var count = 0
        let handle = call(&json, &count)
        defer { exact_markup_free(handle) }
        guard handle != 0, let json, count >= 0 else { return nil }
        return Data(bytes: json, count: count)
    }

    static func edit(_ source: String, selection: NSRange, command: String, argument: String = "") -> MarkupEdit? {
        var text = source, command = command, argument = argument
        let data = text.withUTF8 { text in command.withUTF8 { command in argument.withUTF8 { argument in
            read { out, count in
                exact_markup_edit(text.baseAddress, text.count, UInt32(clamping: selection.location), UInt32(clamping: NSMaxRange(selection)),
                                  command.baseAddress, command.count, argument.baseAddress, argument.count, out, count)
            }
        } } }
        guard let data, let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return MarkupEdit(source: source, object: object)
    }

    static func plain(_ source: String) -> String? {
        var text = source
        let data = text.withUTF8 { text in read { out, count in
            exact_markup_plain(text.baseAddress, text.count, out, count)
        } }
        return data.flatMap { String(data: $0, encoding: .utf8) }
    }

    static func selection(_ source: String, range: NSRange) -> String? {
        var text = source
        let data = text.withUTF8 { text in read { out, count in
            exact_markup_selection(text.baseAddress, text.count, UInt32(clamping: range.location), UInt32(clamping: NSMaxRange(range)), out, count)
        } }
        return data.flatMap { String(data: $0, encoding: .utf8) }
    }
}

extension Presenter {
    /// A link sheet may have moved focus; the editor keeps its own bookmark.
    func formatElement(_ args: [Any]) {
        guard (2...3).contains(args.count), let name = args[0] as? String, let command = args[1] as? String,
              args.count == 2 || args[2] is String,
              let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }),
              target.props["markup"] == "markdown", !target.disabled, !target.inert,
              target.props["editable"] != "false", target.window != nil else { return }
        var ancestor = target.superview
        if target.isHidden || target.props["inert"] == "true" { return }
        while let view = ancestor {
            if view.isHidden || (view as? NodeView)?.props["inert"] == "true" { return }
            ancestor = view.superview
        }
        target.formatMarkup(command, argument: args.count == 3 ? (args[2] as! String) : "")
    }
}
