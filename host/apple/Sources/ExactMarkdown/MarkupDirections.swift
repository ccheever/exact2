// Each line's base direction in a Markdown editor (LLP 1045 D11), as the web
// editor gives it: the app's `paragraphDirs` words by line number when they
// change, then each word kept with its line through edits, and a source shown
// again (undo, redo) given back its words. `auto` is decided for both by the
// editor crate (`exact_markup_auto`); a line without a word, or an `auto`
// line without a strong letter, takes the field's direction.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import ExactKit

/// The lines' directions as drawn, and what they were drawn for. The most
/// common direction is every paragraph's base, set once with the base
/// style; only runs of lines that differ are set line by line.
struct MarkupDirections {
    /// The words last placed, `paragraphDirs` as the app wrote it.
    private(set) var said = ""
    /// Each line's word as drawn.
    private var words: [String?] = [nil]
    /// The source drawn, and its lines, each with its line feed.
    private var source = ""
    private var lines = [NSRange(location: 0, length: 0)]
    /// The words drawn for the last 64 sources shown, by the source's hash and
    /// length, most recent last: undo and redo show one again.
    private var seen: [(key: Int, length: Int, words: [String?])] = []
    /// What the directions were resolved for besides the source and words.
    private var resolvedFor: (rtl: Bool, kinds: String, faces: String)?
    private var directions: [NSWritingDirection] = [.leftToRight]
    /// The paragraphs' base, and the runs of lines whose direction differs.
    private(set) var base: NSWritingDirection = .leftToRight
    private(set) var runs: [(range: NSRange, direction: NSWritingDirection)] = []

    /// Bring the lines up to `text` and `said`. `rtl` is the field's own
    /// direction, which a line without a word takes; `auto` gives each
    /// line's direction under `auto` (`MarkupCommands.auto`), asked only
    /// when a line says `auto`.
    mutating func update(text: String, said: String, rtl: Bool, atoms: MarkdownAtoms, auto: () -> [UInt8]) {
        var changed = false
        if text != source {
            let old = source as NSString, oldLines = lines, new = text as NSString
            lines = Self.lines(new)
            if let known = seen.last(where: { $0.key == text.hashValue && $0.length == new.length }), known.words.count == lines.count {
                words = known.words
            } else {
                words = Self.carry(words, from: oldLines, in: old, to: lines, in: new)
            }
            source = text
            changed = true
        }
        if said != self.said {
            self.said = said
            let list = said.split { $0 == " " || $0 == "\t" || $0 == "\n" || $0 == "\r" || $0 == "\r\n" || $0 == "\u{0C}" }
            words = lines.indices.map { list.isEmpty ? nil : String(list[min($0, list.count - 1)]) }
            changed = true
        }
        if changed {
            seen.removeAll { $0.key == text.hashValue }
            seen.append((text.hashValue, (text as NSString).length, words))
            if seen.count > 64 { seen.removeFirst() }
        }
        if !changed, let r = resolvedFor, r.rtl == rtl, r.kinds == atoms.kinds, r.faces == atoms.faces { return }
        resolvedFor = (rtl, atoms.kinds, atoms.faces)
        resolve(rtl: rtl, auto: words.contains("auto") ? auto() : [])
    }

    /// The direction of the line holding `caret`, which typing takes.
    func direction(at caret: Int) -> NSWritingDirection {
        var lo = 0, hi = lines.count - 1
        while lo < hi { let mid = (lo + hi + 1) / 2; if lines[mid].location <= caret { lo = mid } else { hi = mid - 1 } }
        return directions[lo]
    }

    private mutating func resolve(rtl: Bool, auto: [UInt8]) {
        let field: NSWritingDirection = rtl ? .rightToLeft : .leftToRight
        directions = words.enumerated().map { i, word in
            switch word {
            case "ltr"?: .leftToRight
            case "rtl"?: .rightToLeft
            case "auto"? where i < auto.count && auto[i] == 1: .leftToRight
            case "auto"? where i < auto.count && auto[i] == 2: .rightToLeft
            default: field
            }
        }
        base = directions.filter { $0 == .rightToLeft }.count * 2 > directions.count ? .rightToLeft : .leftToRight
        runs = []
        for (line, direction) in zip(lines, directions) where direction != base && line.length > 0 {
            if let last = runs.last, NSMaxRange(last.range) == line.location, last.direction == direction {
                runs[runs.count - 1].range.length += line.length
            } else {
                runs.append((line, direction))
            }
        }
    }

    /// Each line of `text` with its line feed; a final line feed leaves an
    /// empty last line, as the web editor draws one.
    static func lines(_ text: NSString) -> [NSRange] {
        var out: [NSRange] = [], start = 0
        while true {
            let feed = text.range(of: "\n", options: .literal, range: NSRange(location: start, length: text.length - start))
            guard feed.location != NSNotFound else { out.append(NSRange(location: start, length: text.length - start)); return out }
            out.append(NSRange(location: start, length: NSMaxRange(feed) - start))
            start = NSMaxRange(feed)
        }
    }

    /// The words moved with their lines from `old` to `new`: the lines
    /// before and after an edit keep theirs, the edited lines take theirs in
    /// order, and a line the edit adds takes the word of the line above it
    /// (at the top, below it), as `carry` in `markup-editor.js`.
    static func carry(_ words: [String?], from oldLines: [NSRange], in old: NSString, to newLines: [NSRange], in new: NSString) -> [String?] {
        func text(_ line: NSRange, _ s: NSString) -> String {
            let feed = line.length > 0 && s.character(at: NSMaxRange(line) - 1) == 10
            return s.substring(with: NSRange(location: line.location, length: line.length - (feed ? 1 : 0)))
        }
        var head = 0, tail = 0
        while head < oldLines.count, head < newLines.count, text(oldLines[head], old) == text(newLines[head], new) { head += 1 }
        while tail < oldLines.count - head, tail < newLines.count - head,
              text(oldLines[oldLines.count - 1 - tail], old) == text(newLines[newLines.count - 1 - tail], new) { tail += 1 }
        let kept = oldLines.count - head - tail
        var next = Array(words.prefix(head))
        for k in 0..<(newLines.count - head - tail) {
            if k < kept { next.append(words[head + k]) }
            else if head + k > 0 { next.append(next[head + k - 1]) }
            else { next.append(head + kept < words.count ? words[head + kept] : nil) }
        }
        return next + words.suffix(tail)
    }
}
