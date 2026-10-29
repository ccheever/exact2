// Each line's base direction in a Markdown editor (LLP 1045 D11), as the web
// editor gives it: the app's `paragraphDirs` words by line number when they
// change, then each word kept with its line through edits; `auto` and lines
// without a word resolved by the same rules as `markup-editor.js`.
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
    /// What the directions were resolved for besides the source and words.
    private var resolvedFor: (rtl: Bool, kinds: String, faces: String)?
    private var directions: [NSWritingDirection] = [.leftToRight]
    /// The paragraphs' base, and the runs of lines whose direction differs.
    private(set) var base: NSWritingDirection = .leftToRight
    private(set) var runs: [(range: NSRange, direction: NSWritingDirection)] = []

    /// Bring the lines up to `text` and `said`. `rtl` is the field's own
    /// direction, which a line without a word takes; `syntax` gives the
    /// ranges `auto` passes over, asked only when a line says `auto`.
    mutating func update(text: String, said: String, rtl: Bool, atoms: MarkdownAtoms, syntax: () -> [NSRange]) {
        var changed = false
        if text != source {
            let old = source as NSString, oldLines = lines, new = text as NSString
            lines = Self.lines(new)
            words = Self.carry(words, from: oldLines, in: old, to: lines, in: new)
            source = text
            changed = true
        }
        if said != self.said {
            self.said = said
            let list = said.split { $0 == " " || $0 == "\t" || $0 == "\n" || $0 == "\r" || $0 == "\r\n" || $0 == "\u{0C}" }
            words = lines.indices.map { list.isEmpty ? nil : String(list[min($0, list.count - 1)]) }
            changed = true
        }
        if !changed, let r = resolvedFor, r.rtl == rtl, r.kinds == atoms.kinds, r.faces == atoms.faces { return }
        resolvedFor = (rtl, atoms.kinds, atoms.faces)
        resolve(text as NSString, rtl: rtl, skip: words.contains("auto") ? syntax().sorted { $0.location < $1.location } : [])
    }

    /// The direction of the line holding `caret`, which typing takes.
    func direction(at caret: Int) -> NSWritingDirection {
        var lo = 0, hi = lines.count - 1
        while lo < hi { let mid = (lo + hi + 1) / 2; if lines[mid].location <= caret { lo = mid } else { hi = mid - 1 } }
        return directions[lo]
    }

    private mutating func resolve(_ text: NSString, rtl: Bool, skip: [NSRange]) {
        let field: NSWritingDirection = rtl ? .rightToLeft : .leftToRight
        directions = zip(lines, words).map { line, word in
            switch word {
            case "ltr"?: .leftToRight
            case "rtl"?: .rightToLeft
            case "auto"?: Self.firstStrong(text, line, skipping: skip) ?? field
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

    /// `auto`'s direction for `line`: its first strong letter outside
    /// `skip` (sorted: syntax and atoms), as `firstStrong` in
    /// `markup-editor.js`; nil for none.
    static func firstStrong(_ text: NSString, _ line: NSRange, skipping skip: [NSRange]) -> NSWritingDirection? {
        var i = line.location, k = 0
        let end = NSMaxRange(line)
        while i < end {
            while k < skip.count, NSMaxRange(skip[k]) <= i { k += 1 }
            if k < skip.count, skip[k].location <= i { i = NSMaxRange(skip[k]); continue }
            let unit = text.character(at: i)
            var value = UInt32(unit), width = 1
            if UTF16.isLeadSurrogate(unit), i + 1 < end, UTF16.isTrailSurrogate(text.character(at: i + 1)) {
                value = 0x10000 + ((value - 0xD800) << 10) + (UInt32(text.character(at: i + 1)) - 0xDC00)
                width = 2
            }
            if let scalar = Unicode.Scalar(value), let direction = strong(scalar) { return direction }
            i += width
        }
        return nil
    }

    /// A strong letter's direction (a letter, or an explicit mark), and the
    /// right-to-left scripts' as UAX #9's R and AL; nil for any other.
    static func strong(_ s: Unicode.Scalar) -> NSWritingDirection? {
        switch s.value {
        case 0x200E: return .leftToRight
        case 0x200F, 0x061C: return .rightToLeft
        default: break
        }
        switch s.properties.generalCategory {
        case .uppercaseLetter, .lowercaseLetter, .titlecaseLetter, .modifierLetter, .otherLetter: break
        default: return nil
        }
        switch s.value {
        case 0x0590...0x08FF, 0xFB1D...0xFDFF, 0xFE70...0xFEFF, 0x10800...0x10FFF, 0x1E800...0x1EFFF: return .rightToLeft
        default: return .leftToRight
        }
    }
}
