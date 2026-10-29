// Each line's base direction in a Markdown editor (LLP 1045 D11), as the web
// editor gives it: the app's `paragraphDirs` words by line number when they
// change, then each word kept with its line through edits. Undo and redo give
// back the words of the source they return to (`MarkupEditor` keeps them with
// the native undo step). A line ends at CRLF, CR or LF, where TextKit ends a
// paragraph and the web editor a line. `auto` is decided for both hosts by
// the editor crate (`exact_markup_auto`); a line without a word, or an `auto`
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
    /// A line's word: none (or one other than the three), `ltr`, `rtl` or `auto`.
    typealias Word = UInt8
    static let ltr: Word = 1, rtl: Word = 2, auto: Word = 3

    /// One source's words as an undo step keeps them: the line count, the
    /// most common word, and the lines that hold another.
    struct Words {
        let count: Int
        let common: Word
        let others: [(line: UInt32, word: Word)]

        var expanded: [Word] {
            var words = [Word](repeating: common, count: count)
            for other in others { words[Int(other.line)] = other.word }
            return words
        }
    }

    /// What an update did to the words.
    enum Change {
        /// Nothing an undo step keeps: no words, a moved selection, the
        /// field's direction or atoms, or the words an undo or redo gave back.
        case none
        /// An edit changed the source of a field with words: `before` is
        /// the words the source had.
        case edited(before: Words)
        /// The app placed new words, or took them away.
        case placed
    }

    /// The words last placed, `paragraphDirs` as the app wrote it.
    private(set) var said = ""
    /// Each line's word; empty while the field has none.
    private var words: [Word] = []
    /// The source the lines were found in, and each line with its break.
    private var source = ""
    private var lines: [NSRange] = []
    /// What the directions were resolved for besides the source and words.
    private var resolvedFor: (rtl: Bool, kinds: String, faces: String)?
    private var directions: [NSWritingDirection] = []
    /// The paragraphs' base, and the runs of lines whose direction differs.
    private(set) var base: NSWritingDirection = .leftToRight
    private(set) var runs: [(range: NSRange, direction: NSWritingDirection)] = []

    /// Bring the lines up to `text` and `said`. `back` is the words an undo
    /// or redo gives back with `text`; `rtl` is the field's own direction,
    /// which a line without a word takes; `auto` gives each line's direction
    /// under `auto` (`MarkupCommands.auto`), asked only when a line says it.
    mutating func update(text: String, said: String, rtl: Bool, atoms: MarkdownAtoms, back: Words?, auto: () -> [UInt8]) -> Change {
        let field: NSWritingDirection = rtl ? .rightToLeft : .leftToRight
        let list = said.split { $0 == " " || $0 == "\t" || $0 == "\n" || $0 == "\r" || $0 == "\r\n" || $0 == "\u{0C}" }
        guard !list.isEmpty else {
            // Every line takes the field's direction: nothing to carry or keep.
            let placed = !words.isEmpty
            if placed || said != self.said || resolvedFor?.rtl != rtl {
                self.said = said
                words = []; source = ""; lines = []; directions = []; runs = []
                base = field
                resolvedFor = (rtl, atoms.kinds, atoms.faces)
            }
            return placed ? .placed : .none
        }
        var change = Change.none, dirty = false
        // Literally, code unit for code unit: canonically equivalent text
        // (NFC for NFD) can differ in length, so it is another source.
        if lines.isEmpty || !text.utf8.elementsEqual(source.utf8) {
            dirty = true
            let old = source as NSString, oldLines = lines, new = text as NSString
            lines = Self.lines(new)
            if let back, back.count == lines.count {
                words = back.expanded
            } else if !words.isEmpty {
                if back == nil { change = .edited(before: memory) }
                words = Self.carry(words, from: oldLines, in: old, to: lines, in: new)
            }
            source = text
        }
        if said != self.said || words.isEmpty {
            self.said = said
            words = lines.indices.map { Self.word(list[min($0, list.count - 1)]) }
            change = .placed
            dirty = true
        }
        if !dirty, let r = resolvedFor, r.rtl == rtl, r.kinds == atoms.kinds, r.faces == atoms.faces { return change }
        resolvedFor = (rtl, atoms.kinds, atoms.faces)
        resolve(field, auto: words.contains(Self.auto) ? auto() : [])
        return change
    }

    /// The words drawn now, as an undo step keeps them.
    var memory: Words {
        var tally = [Int](repeating: 0, count: 4)
        for word in words { tally[Int(min(word, 3))] += 1 }
        let common = Word(tally.indices.max { tally[$0] < tally[$1] } ?? 0)
        return Words(count: words.count, common: common,
                     others: words.indices.compactMap { words[$0] == common ? nil : (UInt32($0), words[$0]) })
    }

    /// The direction of the line holding `caret`, which typing takes.
    func direction(at caret: Int) -> NSWritingDirection {
        guard !directions.isEmpty else { return base }
        var lo = 0, hi = lines.count - 1
        while lo < hi { let mid = (lo + hi + 1) / 2; if lines[mid].location <= caret { lo = mid } else { hi = mid - 1 } }
        return directions[lo]
    }

    private mutating func resolve(_ field: NSWritingDirection, auto: [UInt8]) {
        directions = words.indices.map { i in
            switch words[i] {
            case Self.ltr: .leftToRight
            case Self.rtl: .rightToLeft
            case Self.auto where i < auto.count && auto[i] == 1: .leftToRight
            case Self.auto where i < auto.count && auto[i] == 2: .rightToLeft
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

    static func word(_ word: Substring) -> Word {
        switch word {
        case "ltr": ltr
        case "rtl": rtl
        case "auto": auto
        default: 0
        }
    }

    private static let breaks = CharacterSet(charactersIn: "\r\n")

    /// Each line of `text` with its break (CRLF, CR or LF); a final break
    /// leaves an empty last line, as the web editor draws one.
    static func lines(_ text: NSString) -> [NSRange] {
        var out: [NSRange] = [], start = 0
        let length = text.length
        while true {
            let found = text.rangeOfCharacter(from: breaks, options: .literal, range: NSRange(location: start, length: length - start))
            guard found.location != NSNotFound else { out.append(NSRange(location: start, length: length - start)); return out }
            var end = NSMaxRange(found)
            if text.character(at: found.location) == 13, end < length, text.character(at: end) == 10 { end += 1 }
            out.append(NSRange(location: start, length: end - start))
            start = end
        }
    }

    /// The words moved with their lines from `old` to `new`: the lines
    /// before and after an edit keep theirs, the edited lines take theirs in
    /// order, and a line the edit adds takes the word of the line above it
    /// (at the top, below it), as `carry` in `markup-editor.js`.
    static func carry(_ words: [Word], from oldLines: [NSRange], in old: NSString, to newLines: [NSRange], in new: NSString) -> [Word] {
        func body(_ line: NSRange, _ s: NSString) -> NSRange {
            var length = line.length
            if length > 0, s.character(at: line.location + length - 1) == 10 { length -= 1 }
            if length > 0, s.character(at: line.location + length - 1) == 13 { length -= 1 }
            return NSRange(location: line.location, length: length)
        }
        func same(_ a: NSRange, _ b: NSRange) -> Bool {
            let a = body(a, old), b = body(b, new)
            return a.length == b.length && old.compare(new.substring(with: b), options: .literal, range: a) == .orderedSame
        }
        var head = 0, tail = 0
        while head < oldLines.count, head < newLines.count, same(oldLines[head], newLines[head]) { head += 1 }
        while tail < oldLines.count - head, tail < newLines.count - head,
              same(oldLines[oldLines.count - 1 - tail], newLines[newLines.count - 1 - tail]) { tail += 1 }
        let kept = oldLines.count - head - tail
        var next = Array(words.prefix(head))
        for k in 0..<(newLines.count - head - tail) {
            if k < kept { next.append(words[head + k]) }
            else if head + k > 0 { next.append(next[head + k - 1]) }
            else { next.append(head + kept < words.count ? words[head + kept] : 0) }
        }
        return next + words.suffix(tail)
    }
}
