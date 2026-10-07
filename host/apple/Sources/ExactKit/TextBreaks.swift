// CSS line breaking on Apple (LLP 1008 §3, CSS Text 3 §5): where a line may
// end, and min-content's unbreakable pieces, from the shared walker's
// opportunities (UAX #14 as Chrome tailors it, LLP 1043 §4 C) rather than
// CoreText's or CFStringTokenizer's, which break after a `/` before a letter.
import Foundation
import CoreText
import CExact

/// One paragraph layout's opportunities and where it has got to in them.
struct LineBreakPlan {
    var boundaries: [Int] = []
    var contentEnds: [Int] = []
    var hardEnds: [Int] = []
    var boundaryIndex = 0, hardIndex = 0

    mutating func prepare(_ text: NSString, length: Int) {
        (contentEnds, hardEnds) = TextEngine.breakEnds(text, boundaries: boundaries, length: length)
    }
    mutating func advance(to start: Int) {
        while boundaryIndex < boundaries.count && boundaries[boundaryIndex] <= start { boundaryIndex += 1 }
        while hardIndex < hardEnds.count && hardEnds[hardIndex] <= start { hardIndex += 1 }
    }
    /// The line's length from `start` in `room`. No opportunities (an
    /// infinite offer): CoreText's own break. A zero-width offer keeps
    /// CoreText's degenerate breaking, as the region worker does, a `normal`
    /// word kept whole.
    func suggest(_ typesetter: CTTypesetter, start: Int, room: Double, offerWidth: CGFloat, breakWord: Bool, length: Int, text: NSString) -> Int {
        guard !boundaries.isEmpty else { return CTTypesetterSuggestLineBreak(typesetter, start, room) }
        if offerWidth <= 0 {
            var count = CTTypesetterSuggestLineBreak(typesetter, start, room)
            if !breakWord {
                var i = boundaryIndex
                while i < boundaries.count && boundaries[i] < start + count { i += 1 }
                if i < boundaries.count { count = boundaries[i] - start }
            }
            return count
        }
        // A forced break ends the line wherever the room is.
        let hard = (hardIndex < hardEnds.count ? hardEnds[hardIndex] : length) - start
        let fitted = min(CTTypesetterSuggestClusterBreak(typesetter, start, room), hard)
        return TextEngine.cssBreak(start: start, fitted: fitted, hard: hard, boundaries: boundaries, contentEnds: contentEnds,
                                   from: boundaryIndex, breakWord: breakWord, text: text)
    }
}

extension TextEngine {
    /// Each opportunity's content end, and the ends of the forced breaks
    /// (CR LF one), in one forward pass. A boundary's own forced break is no
    /// content, and the spaces before it hang (CSS Text 3 §4.1.3): U+0020,
    /// tabs and the other space separators; no-break spaces do not.
    static func breakEnds(_ text: NSString, boundaries: [Int], length: Int) -> ([Int], [Int]) {
        func hangs(_ ch: unichar) -> Bool { hangingSpace(ch) }
        // runStart[e]: where the spaces and tabs ending at e begin.
        var runStart = [Int](repeating: 0, count: length + 1)
        var hard: [Int] = []
        var i = 0
        while i < length {
            let ch = text.character(at: i)
            runStart[i + 1] = hangs(ch) ? runStart[i] : i + 1
            if ch == 13, i + 1 < length, text.character(at: i + 1) == 10 {
                runStart[i + 2] = i + 2; hard.append(i + 2); i += 2; continue
            }
            if forcedBreak(ch) { hard.append(i + 1) }
            i += 1
        }
        let content = boundaries.map { boundary -> Int in
            var end = min(boundary, length)
            if end > 0, forcedBreak(text.character(at: end - 1)) {
                end -= end >= 2 && text.character(at: end - 1) == 10 && text.character(at: end - 2) == 13 ? 2 : 1
            }
            return runStart[end]
        }
        return (content, hard)
    }
    /// A space that hangs at a line's end: U+0020, a tab, or another Unicode
    /// space separator (Zs) that is not a no-break space (U+00A0, U+2007, U+202F).
    static func hangingSpace(_ ch: unichar) -> Bool {
        ch == 0x20 || ch == 0x09 || ch == 0x1680 || (0x2000...0x200A).contains(ch) && ch != 0x2007 || ch == 0x205F || ch == 0x3000
    }
    static func forcedBreak(_ ch: unichar) -> Bool {
        ch == 10 || ch == 13 || ch == 0x0B || ch == 0x0C || ch == 0x85 || ch == 0x2028 || ch == 0x2029
    }

    /// CSS's choice of where a line ends, `fitted` the most clusters that fit
    /// from `start` (no further than the next forced break, `hard`): the last
    /// opportunity whose content fits; else the first opportunity, letting
    /// the word overflow (`normal`), or inside it at `fitted` (`break-word`).
    static func cssBreak(start: Int, fitted: Int, hard: Int, boundaries: [Int], contentEnds: [Int],
                         from: Int, breakWord: Bool, text: NSString) -> Int {
        var i = from
        while i < boundaries.count && boundaries[i] <= start { i += 1 }
        let first = i
        while i < boundaries.count && boundaries[i] - start <= hard && contentEnds[i] <= start + fitted { i += 1 }
        if i > first { return boundaries[i - 1] - start }
        if !breakWord, first < boundaries.count { return min(boundaries[first] - start, hard) }
        if fitted > 0 { return fitted }
        return start < text.length ? text.rangeOfComposedCharacterSequence(at: start).length : 0
    }

    /// A break at a soft hyphen shows one, which must fit too, as in Chrome
    /// (CoreText counts the invisible SHY as nothing). When it does not, an
    /// earlier opportunity, which shows no hyphen, takes the original room;
    /// only the first opportunity falls back to `retry` with the hyphen's
    /// advance taken from the room (a break inside it, or its overflow).
    static func fitSoftHyphen(start: Int, count: Int, room: Double, boundaries: [Int], source: NSAttributedString,
                              typesetter: CTTypesetter, length: Int, retry: (Double) -> Int) -> Int {
        var count = count
        let text = source.string as NSString
        while count > 0, start + count < length, room.isFinite, text.character(at: start + count - 1) == 0xAD {
            let plain = CTTypesetterCreateLine(typesetter, CFRangeMake(start, count))
            let ink = CTLineGetTypographicBounds(inkedSoftHyphen(plain, source: source, range: CFRangeMake(start, count)), nil, nil, nil)
            if ink <= room { break }
            var lo = 0, hi = boundaries.count
            while lo < hi { let mid = (lo + hi) / 2; if boundaries[mid] < start + count { lo = mid + 1 } else { hi = mid } }
            if lo > 0, boundaries[lo - 1] > start { count = boundaries[lo - 1] - start; continue }
            return retry(room - (ink - CTLineGetTypographicBounds(plain, nil, nil, nil)))
        }
        return count
    }

    /// Min-content's pieces of `value`: the text between UAX #14 line-break
    /// opportunities, their hanging spaces dropped (`normal` and `break-word`
    /// alike: CSS counts no break-word break toward min-content).
    func unbreakablePieces(_ value: String) -> [String] {
        Self.pieces(value as NSString, boundaries: lineBoundaries(value as NSString, length: (value as NSString).length))
    }
    /// The same from a worker.
    static func unbreakablePieces(_ value: String) -> [String] {
        let text = value as NSString
        return pieces(text, boundaries: lineBreaks(text, length: text.length))
    }
    /// Where a line may end, as UTF16 offsets, the last being `length`: the
    /// shared walker's opportunities (`exact_text_line_breaks`), Chrome's, with
    /// CFStringTokenizer's dictionary words inside Thai, Lao, Khmer and Myanmar
    /// runs (TextFlowSource.complexWords). Any thread.
    static func lineBreaks(_ text: NSString, length: Int) -> [Int] {
        guard length > 0 else { return [0] }
        let string = text as String
        let words = TextFlowSource.complexWords(string)
        var ends = [UInt32](repeating: 0, count: length + 1)
        let count = Array(string.utf8).withUnsafeBufferPointer { bytes in
            words.withUnsafeBufferPointer { words in
                ends.withUnsafeMutableBufferPointer { out in
                    exact_text_line_breaks(bytes.baseAddress, bytes.count, words.baseAddress, words.count, out.baseAddress, out.count)
                }
            }
        }
        var boundaries = ends.prefix(min(count, ends.count)).map { Int($0) }.filter { $0 > 0 && $0 <= length }
        if boundaries.last != length { boundaries.append(length) }
        return boundaries
    }
    static func pieces(_ text: NSString, boundaries: [Int]) -> [String] {
        guard text.length > 0 else { return [] }
        func trims(_ ch: unichar) -> Bool { hangingSpace(ch) || forcedBreak(ch) }
        var pieces: [String] = []
        var start = 0
        for end in boundaries where end > start {
            var trimmed = end
            while trimmed > start, trims(text.character(at: trimmed - 1)) { trimmed -= 1 }
            var lead = start
            while lead < trimmed, trims(text.character(at: lead)) { lead += 1 }
            if trimmed > lead {
                var piece = text.substring(with: NSRange(location: lead, length: trimmed - lead))
                // A break right after a soft hyphen shows one (inkedSoftHyphen
                // reads the line's last character): min-content counts it.
                if trimmed == end, text.character(at: end - 1) == 0xAD { piece = String(piece.dropLast()) + "-" }
                pieces.append(piece)
            }
            start = end
        }
        return pieces
    }
}
