#if os(macOS)
import Foundation

/// The composer's text rules, ported from T3's composer-logic.ts,
/// composer-list-continuation.ts, composerPromptHistory.ts and the shared
/// inline-token collectors. Offsets are UTF-16, as the web's string indices
/// and NSTextView's ranges both are. Pure, so the AppKit tests cover them.
struct T3ComposerTrigger: Equatable {
    let kind: String
    let query: String
    let start: Int
    let end: Int
    var dictionary: [String: Any] { ["kind": kind, "query": query, "start": start, "end": end] }
}

/// One inline chip's source span (a file link or @path, a `$skill`, a
/// t3-context reference or an assistant citation).
struct T3ComposerChip: Equatable {
    let kind: String      // mention | skill | context | citation
    let start: Int
    let end: Int
    /// Display label: a basename, a skill name, a context label or the quote.
    let label: String
    /// The mention's path, the context's kind ("thread", "review-comment"…), or "".
    let detail: String
    var range: NSRange { NSRange(location: start, length: end - start) }
}

struct T3ComposerListEdit: Equatable {
    let start: Int
    let end: Int
    let replacement: String
}

enum T3ComposerText {
    static func isBlank(_ unit: unichar) -> Bool { unit == 32 || unit == 10 || unit == 9 || unit == 13 }

    private static func regex(_ pattern: String) -> NSRegularExpression {
        // The patterns are literals; a failure is a programming error caught by the tests.
        try! NSRegularExpression(pattern: pattern, options: [])
    }
    private static let pullRequestToken = regex("^#([\\p{L}\\p{N}][\\p{L}\\p{N}_-]*)?$")
    private static let currencyPrefix = regex("^\\p{Sc}")
    private static let slashLine = regex("^/(\\S*)$")

    /// detectComposerTrigger: a `/` command at the start of a line, then the
    /// whitespace-delimited token before the caret (`#pr`, a currency sign, `@path`).
    static func trigger(_ text: NSString, cursor rawCursor: Int) -> T3ComposerTrigger? {
        let cursor = max(0, min(text.length, rawCursor))
        var lineStart = 0
        if cursor > 0 {
            let found = text.range(of: "\n", options: .backwards, range: NSRange(location: 0, length: cursor))
            if found.location != NSNotFound { lineStart = found.location + 1 }
        }
        let linePrefix = text.substring(with: NSRange(location: lineStart, length: cursor - lineStart))
        if linePrefix.hasPrefix("/"), let match = slashLine.firstMatch(in: linePrefix, range: NSRange(location: 0, length: (linePrefix as NSString).length)) {
            let query = (linePrefix as NSString).substring(with: match.range(at: 1))
            return T3ComposerTrigger(kind: "slash-command", query: query, start: lineStart, end: cursor)
        }
        var index = cursor - 1
        while index >= 0, !isBlank(text.character(at: index)) { index -= 1 }
        let tokenStart = index + 1
        let token = text.substring(with: NSRange(location: tokenStart, length: cursor - tokenStart))
        let tokenRange = NSRange(location: 0, length: (token as NSString).length)
        if let match = pullRequestToken.firstMatch(in: token, range: tokenRange) {
            let group = match.range(at: 1)
            return T3ComposerTrigger(kind: "pull-request", query: group.location == NSNotFound ? "" : (token as NSString).substring(with: group), start: tokenStart, end: cursor)
        }
        if let match = currencyPrefix.firstMatch(in: token, range: tokenRange) {
            return T3ComposerTrigger(kind: "skill", query: (token as NSString).substring(from: match.range.length), start: tokenStart, end: cursor)
        }
        guard token.hasPrefix("@") else { return nil }
        return T3ComposerTrigger(kind: "path", query: String(token.dropFirst()), start: tokenStart, end: cursor)
    }

    // MARK: Inline chips (composer-editor-mentions.ts, composerInlineTokens.ts)

    private static let contextLink = regex("(!?)\\[([^\\]\\n]{0,512})\\]\\((t3-context://v1/[^\\s)]{1,200})\\)")
    private static let citationLink = regex("\\[Assistant quote\\]\\((t3-citation://v1/[^\\s)]{1,64000})\\)")
    private static let fileLink = regex("(^|\\s)\\[((?:\\\\.|[^\\]\\\\]){0,512})\\]\\(([^)\\s]+)\\)(?=\\s)")
    private static let atMention = regex("(^|\\s)@(?:\"((?:\\\\.|[^\"\\\\])*)\"|([^\\s@\"]+))(?=\\s)")
    private static let scopedPackage = regex("^[a-z0-9][a-z0-9._-]*/[a-z0-9][a-z0-9._-]*(?:/[^\\s@\"]+)*$")
    private static let uriScheme = regex("^[A-Za-z][A-Za-z0-9+.-]*:")
    private static let windowsDrive = regex("^[A-Za-z]:[\\\\/]")
    private static let skillToken = regex("(^|\\s)\\p{Sc}(?![0-9][0-9_]*(?:[kKmMbBtT]|[eE][0-9]+)?(?:\\s|$))(?=[a-zA-Z0-9:_-]*[a-zA-Z])([a-zA-Z0-9][a-zA-Z0-9:_-]*)(?=\\s)")
    private static let contextKind = regex("^[a-z][a-z0-9-]{0,39}$")
    private static let contextId = regex("^[a-z0-9_-]{1,128}$")

    private static func matches(_ expression: NSRegularExpression, _ value: String) -> Bool {
        expression.firstMatch(in: value, range: NSRange(location: 0, length: (value as NSString).length)) != nil
    }
    static func unescapeLabel(_ value: String) -> String {
        var out = "", escaped = false
        for character in value {
            if escaped { out.append(character); escaped = false } else if character == "\\" { escaped = true } else { out.append(character) }
        }
        return out
    }
    static func basename(_ path: String) -> String {
        let cut = max(path.lastIndex(of: "/").map { path.distance(from: path.startIndex, to: $0) } ?? -1,
                      path.lastIndex(of: "\\").map { path.distance(from: path.startIndex, to: $0) } ?? -1)
        return cut >= 0 ? String(path.dropFirst(cut + 1)) : path
    }

    /// Every chip in source order; overlapping plain tokens yield to links.
    static func chips(_ string: String) -> [T3ComposerChip] {
        let text = string as NSString
        let all = NSRange(location: 0, length: text.length)
        var links: [T3ComposerChip] = []
        if string.contains("](t3-context:") {
            for match in contextLink.matches(in: string, range: all) {
                let href = text.substring(with: match.range(at: 3))
                let parts = href.dropFirst("t3-context://v1/".count).split(separator: "/", omittingEmptySubsequences: false).map(String.init)
                guard parts.count == 2, matches(contextKind, parts[0]), matches(contextId, parts[1]) else { continue }
                links.append(T3ComposerChip(kind: "context", start: match.range.location, end: NSMaxRange(match.range),
                    label: text.substring(with: match.range(at: 2)), detail: parts[0]))
            }
        }
        if string.contains("[Assistant quote](t3-citation:") {
            for match in citationLink.matches(in: string, range: all) {
                let href = text.substring(with: match.range(at: 1))
                let quote = URLComponents(string: href)?.queryItems?.first(where: { $0.name == "text" })?.value ?? "Assistant quote"
                links.append(T3ComposerChip(kind: "citation", start: match.range.location, end: NSMaxRange(match.range), label: quote, detail: ""))
            }
        }
        var tokens: [T3ComposerChip] = []
        for match in fileLink.matches(in: string, range: all) {
            let label = unescapeLabel(text.substring(with: match.range(at: 2)))
            let encoded = text.substring(with: match.range(at: 3))
            let path = encoded.removingPercentEncoding ?? encoded
            let external = matches(uriScheme, path) && !matches(windowsDrive, path)
            guard !path.isEmpty, !external, label == basename(path) else { continue }
            let start = match.range(at: 1).location + match.range(at: 1).length
            tokens.append(T3ComposerChip(kind: "mention", start: start, end: NSMaxRange(match.range), label: label, detail: path))
        }
        for match in atMention.matches(in: string, range: all) {
            let quoted = match.range(at: 2)
            let path = quoted.location != NSNotFound ? unescapeLabel(text.substring(with: quoted)) : text.substring(with: match.range(at: 3))
            if path.isEmpty || (quoted.location == NSNotFound && matches(scopedPackage, path)) { continue }
            let start = match.range(at: 1).location + match.range(at: 1).length
            tokens.append(T3ComposerChip(kind: "mention", start: start, end: NSMaxRange(match.range), label: basename(path), detail: path))
        }
        for match in skillToken.matches(in: string, range: all) {
            let start = match.range(at: 1).location + match.range(at: 1).length
            tokens.append(T3ComposerChip(kind: "skill", start: start, end: NSMaxRange(match.range), label: text.substring(with: match.range(at: 2)), detail: ""))
        }
        let kept = tokens.filter { token in !links.contains { token.start < $0.end && token.end > $0.start } }
        var result: [T3ComposerChip] = []
        for chip in (kept + links).sorted(by: { $0.start < $1.start }) where (result.last?.end ?? 0) <= chip.start {
            result.append(chip)
        }
        return result
    }

    static func chip(containing offset: Int, in chips: [T3ComposerChip]) -> T3ComposerChip? {
        chips.first { offset > $0.start && offset < $0.end }
    }

    // MARK: Lists (composer-list-continuation.ts)

    private enum Marker { case ordered(indent: String, number: String, delimiter: String), task(indent: String), bullet(indent: String, bullet: String) }
    private static let orderedMarker = regex("^([0-9]+)([.)])((?:[ \\t]+)|\\s*$)")
    private static let taskMarker = regex("^-\\s\\[[ xX]\\]((?:[ \\t]+)|\\s*$)")
    private static let bulletMarker = regex("^([-*+])((?:[ \\t]+)|\\s*$)")

    private static func parseMarker(_ line: String) -> (marker: Marker, end: Int)? {
        let ns = line as NSString
        var indentLength = 0
        while indentLength < ns.length, ns.character(at: indentLength) == 32 || ns.character(at: indentLength) == 9 { indentLength += 1 }
        let indent = ns.substring(to: indentLength)
        let rest = ns.substring(from: indentLength)
        let restRange = NSRange(location: 0, length: (rest as NSString).length)
        if let m = orderedMarker.firstMatch(in: rest, range: restRange) {
            return (.ordered(indent: indent, number: (rest as NSString).substring(with: m.range(at: 1)), delimiter: (rest as NSString).substring(with: m.range(at: 2))), indentLength + m.range.length)
        }
        if let m = taskMarker.firstMatch(in: rest, range: restRange) { return (.task(indent: indent), indentLength + m.range.length) }
        if let m = bulletMarker.firstMatch(in: rest, range: restRange) {
            return (.bullet(indent: indent, bullet: (rest as NSString).substring(with: m.range(at: 1))), indentLength + m.range.length)
        }
        return nil
    }
    private static func nextMarker(_ marker: Marker) -> String {
        switch marker {
        case let .ordered(indent, number, delimiter):
            guard let value = Int(number), value < Int.max else { return "\(indent)\(number)\(delimiter) " }
            var next = String(value + 1)
            while next.count < number.count { next = "0" + next }
            return "\(indent)\(next)\(delimiter) "
        case let .task(indent): return "\(indent)- [ ] "
        case let .bullet(indent, bullet): return "\(indent)\(bullet) "
        }
    }
    private static func line(_ text: NSString, at cursor: Int) -> (start: Int, end: Int, text: String) {
        var start = 0
        if cursor > 0 {
            let found = text.range(of: "\n", options: .backwards, range: NSRange(location: 0, length: cursor))
            if found.location != NSNotFound { start = found.location + 1 }
        }
        let after = text.range(of: "\n", options: [], range: NSRange(location: cursor, length: text.length - cursor))
        let end = after.location == NSNotFound ? text.length : after.location
        return (start, end, text.substring(with: NSRange(location: start, length: end - start)))
    }

    /// listContinuationForEnter: continue the list, or exit it on an empty item.
    static func listContinuation(_ string: String, cursor: Int) -> T3ComposerListEdit? {
        let text = string as NSString
        guard cursor >= 0, cursor <= text.length else { return nil }
        let current = line(text, at: cursor)
        guard let parsed = parseMarker(current.text) else { return nil }
        let markerEnd = current.start + parsed.end
        guard cursor >= markerEnd, chip(containing: cursor, in: chips(string)) == nil else { return nil }
        if text.substring(with: NSRange(location: markerEnd, length: current.end - markerEnd)).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return T3ComposerListEdit(start: current.start, end: max(cursor, markerEnd), replacement: "")
        }
        return T3ComposerListEdit(start: cursor, end: cursor, replacement: "\n" + nextMarker(parsed.marker))
    }

    /// listIndentForTab: a collapsed caret on a list item indents it by two spaces.
    static func listIndent(_ string: String, start: Int, end: Int) -> T3ComposerListEdit? {
        let text = string as NSString
        guard start == end, start >= 0, start <= text.length else { return nil }
        let current = line(text, at: start)
        guard parseMarker(current.text) != nil, chip(containing: start, in: chips(string)) == nil else { return nil }
        return T3ComposerListEdit(start: current.start, end: current.start, replacement: "  ")
    }

    /// A `- [ ]` or `- [x]` marker's box range on the line starting at `lineStart`.
    static func taskBoxes(_ string: String) -> [(box: NSRange, checked: Bool)] {
        let text = string as NSString
        var out: [(NSRange, Bool)] = []
        var location = 0
        while location <= text.length {
            let current = line(text, at: location)
            let ns = current.text as NSString
            var indent = 0
            while indent < ns.length, ns.character(at: indent) == 32 || ns.character(at: indent) == 9 { indent += 1 }
            let rest = ns.substring(from: indent)
            if taskMarker.firstMatch(in: rest, range: NSRange(location: 0, length: (rest as NSString).length)) != nil {
                let mark = (rest as NSString).character(at: 3)
                out.append((NSRange(location: current.start + indent + 2, length: 3), mark == 120 || mark == 88))
            }
            if current.end >= text.length { break }
            location = current.end + 1
        }
        return out
    }

    // MARK: Prompt history (composerPromptHistory.ts)

    struct HistoryEntry: Equatable { let id: String; let prompt: String }
    struct HistoryPosition: Equatable { let entryId: String; let recalled: String }

    /// stepComposerPromptHistory: nil lets the key move the caret instead.
    static func historyStep(backward: Bool, entries: [HistoryEntry], position: HistoryPosition?, current: String) -> (position: HistoryPosition?, prompt: String)? {
        var active = -1
        if let position, position.recalled == current {
            active = entries.firstIndex { $0.id == position.entryId } ?? (entries.lastIndex { $0.prompt == position.recalled } ?? -1)
        }
        if backward {
            if active < 0 && !current.isEmpty { return nil }
            let index = active < 0 ? entries.count - 1 : active - 1
            guard index >= 0, index < entries.count else { return nil }
            return (HistoryPosition(entryId: entries[index].id, recalled: entries[index].prompt), entries[index].prompt)
        }
        guard active >= 0 else { return nil }
        let index = active + 1
        guard index < entries.count else { return (nil, "") }
        return (HistoryPosition(entryId: entries[index].id, recalled: entries[index].prompt), entries[index].prompt)
    }
}
#endif
