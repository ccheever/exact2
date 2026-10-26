// Link destinations kept out of layout, by the web editor's rules.
// @ref LLP 1045 D1, D5 — which runs are hidden is
// `exact_markdown_editor::link_destinations` (the styler's `l`). Where a
// caret sits beside one, what a deletion or replacement across one changes,
// and what a copy takes follow that crate's `Editor::select`, `remove`,
// `replace_visible` and `copy_range`. This file reimplements them over the
// styler's marks, since Apple hides only destinations and the crate's
// projection hides all syntax. The styler marks each destination
// `.markupHidden` and its `[` `.markupOpener`; TextKit 2 asks this editor
// for each paragraph it lays out and draws a copy whose hidden code units
// are word joiners (U+2060: no advance, no glyph, no line break). The
// storage keeps the source, so offsets, native undo and copy stay source
// offsets, and `layoutManager` is never touched (it would switch the view
// to TextKit 1).
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import ExactKit

extension MarkupEditor: NSTextContentStorageDelegate {
    func textContentStorage(_ textContentStorage: NSTextContentStorage, textParagraphWith range: NSRange) -> NSTextParagraph? {
        guard let storage = textContentStorage.textStorage, storage.hidesSource(in: range) else { return nil }
        let shown = NSMutableAttributedString(attributedString: storage.attributedSubstring(from: range))
        let all = NSRange(location: 0, length: shown.length)
        // Hidden runs, and each atom's first code unit (LLP 1045.000 §4): a
        // U+FFFC carrying a new attachment whose picture is the pill.
        var runs: [(NSRange, MarkupAtom?)] = []
        shown.enumerateAttribute(.markupHidden, in: all) { value, run, _ in
            if value != nil { runs.append((run, nil)) }
        }
        shown.enumerateAttribute(.markupAtom, in: all) { value, run, _ in
            if let atom = value as? MarkupAtom { runs.append((run, atom)) }
        }
        for (run, atom) in runs.sorted(by: { $0.0.location > $1.0.location }) {
            var attributes = shown.attributes(at: run.location, effectiveRange: nil)
            attributes[.markupAtom] = nil
            let piece = NSMutableAttributedString(string: String(repeating: "\u{2060}", count: run.length), attributes: attributes)
            // An atom whose source began in an earlier paragraph has none here.
            if let atom, storage.atomRange(at: range.location + run.location)?.location == range.location + run.location {
                piece.replaceCharacters(in: NSRange(location: 0, length: 1), with: "\u{FFFC}")
                piece.addAttribute(.attachment, value: AtomAttachment(atom), range: NSRange(location: 0, length: 1))
            }
            shown.replaceCharacters(in: run, with: piece)
        }
        return NSTextParagraph(attributedString: shown)
    }
}

extension MarkupEditor {
    /// The hidden run with `start <= p <= end`.
    static func hidden(at p: Int, in s: NSAttributedString) -> NSRange? {
        let all = NSRange(location: 0, length: s.length)
        for q in [p, p - 1] where q >= 0 && q < s.length {
            var run = NSRange()
            if s.attribute(.markupHidden, at: q, longestEffectiveRange: &run, in: all) != nil { return run }
        }
        return nil
    }

    /// The grapheme boundary before `p`, or after it.
    static func step(_ p: Int, backward: Bool, in s: NSAttributedString) -> Int {
        let text = s.string as NSString
        if backward { return p > 0 ? text.rangeOfComposedCharacterSequence(at: p - 1).location : 0 }
        return p < text.length ? NSMaxRange(text.rangeOfComposedCharacterSequence(at: p)) : text.length
    }

    /// Where a person's selection lands. A caret in a run, or at its start,
    /// sits after it (the crate's `canonical`); a range's ends inside one
    /// go to its outer edges. Both edges are drawn at one place, so an end
    /// that steps from one edge onto the run steps on over the next visible
    /// grapheme, as the web editor's caret does around `display: none`.
    func place(_ new: NSRange, from old: NSRange?, in s: NSAttributedString) -> NSRange {
        if let old, old == new { return new }
        // An atom first (LLP 1045.000 D5): a caret inside one goes to its
        // end, where those offsets draw; a range's ends go to its outer edges.
        let new = Self.outward(new, in: s)
        if new.length == 0, let atom = s.atomRange(at: new.location), new.location > atom.location {
            return NSRange(location: NSMaxRange(atom), length: 0)
        }
        if new.length == 0 {
            let p = new.location, o = old.flatMap { $0.length == 0 ? $0.location : nil }
            guard let run = Self.hidden(at: p, in: s) else { return new }
            let a = run.location, b = NSMaxRange(run)
            let back = a > 0 ? Self.step(a, backward: true, in: s) : b
            let at: Int
            if p == b { at = o == a ? Self.step(b, backward: false, in: s) : b }
            else if p == a { at = o == b ? back : b }
            // Inside: one step from an edge (UIKit's arrow) crosses on.
            else if o == b, p == Self.step(b, backward: true, in: s) { at = back }
            else if o == a, p == Self.step(a, backward: false, in: s) { at = Self.step(b, backward: false, in: s) }
            else { at = b }
            return NSRange(location: at, length: 0)
        }
        var start = new.location, end = NSMaxRange(new)
        if let run = Self.hidden(at: start, in: s), start < NSMaxRange(run) {
            if old?.location == NSMaxRange(run), run.location > 0 { start = Self.step(run.location, backward: true, in: s) }
            else { start = run.location }
        }
        if let run = Self.hidden(at: end, in: s), end > run.location {
            if old.map(NSMaxRange) == run.location { end = Self.step(NSMaxRange(run), backward: false, in: s) }
            else { end = NSMaxRange(run) }
        }
        return NSRange(location: start, length: max(start, end) - start)
    }

    func crossing(_ caret: Int, backward: Bool, in s: NSAttributedString) -> Int? {
        guard let run = Self.hidden(at: caret, in: s) else { return nil }
        let a = run.location, b = NSMaxRange(run)
        if backward, caret > a { return a }
        if !backward, caret < b { return b }
        return nil
    }

    /// `range` with each end inside an atom moved to that atom's outer
    /// edge, so an edit or a copy takes an atom whole or not at all.
    static func outward(_ range: NSRange, in s: NSAttributedString) -> NSRange {
        guard range.length > 0 else { return range }
        var start = range.location, end = NSMaxRange(range)
        if let atom = s.atomRange(at: start), start > atom.location { start = atom.location }
        if let atom = s.atomRange(at: end), end > atom.location { end = NSMaxRange(atom) }
        return NSRange(location: start, length: end - start)
    }

    func rewrite(_ range: NSRange, with text: String, caret: Int?, in s: NSAttributedString) -> MarkupRewrite? {
        // A native edit that would take part of an atom takes all of it:
        // Backspace at its end, Delete at its start, a word or line deletion
        // across it (LLP 1045.000 D5). Text typed inside one lands after it.
        if range.length == 0, let atom = s.atomRange(at: range.location), range.location > atom.location {
            let at = NSMaxRange(atom)
            return MarkupRewrite(edits: [(NSRange(location: at, length: 0), text)], caret: at + text.utf16.count)
        }
        let wide = Self.outward(range, in: s)
        if let hidden = rewriteHidden(wide, with: text, caret: caret, in: s) { return hidden }
        guard wide != range else { return nil }
        return MarkupRewrite(edits: [(wide, text)], caret: wide.location + text.utf16.count)
    }

    private func rewriteHidden(_ range: NSRange, with text: String, caret: Int?, in s: NSAttributedString) -> MarkupRewrite? {
        // Select All owns the whole source, its destinations too.
        if range.location == 0, range.length == s.length { return nil }
        if range.length == 0 {
            guard let run = Self.hidden(at: range.location, in: s), range.location > run.location, range.location < NSMaxRange(run) else { return nil }
            let at = NSMaxRange(run)
            return MarkupRewrite(edits: [(NSRange(location: at, length: 0), text)], caret: at + text.utf16.count)
        }
        // A destination stays, and so does its link's `[`: without it the
        // destination would read as text (the web hides both).
        var parts: [NSRange] = []
        var at = range.location, keeps = false
        s.enumerateAttributes(in: range) { attributes, run, _ in
            guard attributes[.markupHidden] != nil || attributes[.markupOpener] != nil else { return }
            keeps = true
            if run.location > at { parts.append(NSRange(location: at, length: run.location - at)) }
            at = NSMaxRange(run)
        }
        guard keeps else { return nil }
        if NSMaxRange(range) > at { parts.append(NSRange(location: at, length: NSMaxRange(range) - at)) }
        guard let first = parts.first else {
            // Only kept source: a deletion from a caret beside a link's `[`
            // takes the grapheme past it, as the crate's `remove` does; any
            // other removes nothing. Text lands where a caret there sits,
            // after the run.
            guard !text.isEmpty else { return Self.past(range, from: caret, in: s) ?? MarkupRewrite(edits: [], caret: nil) }
            let place = Self.hidden(at: range.location, in: s).map { NSMaxRange($0) } ?? range.location
            return MarkupRewrite(edits: [(NSRange(location: place, length: 0), text)], caret: place + text.utf16.count)
        }
        let edits = parts.enumerated().map { (i, part) in (range: part, text: i == 0 ? text : "") }
        return MarkupRewrite(edits: edits, caret: first.location + text.utf16.count)
    }

    /// A deletion of a link's `[` from the caret beside it, as one of the
    /// grapheme on its far side, past any destination there too.
    static func past(_ range: NSRange, from caret: Int?, in s: NSAttributedString) -> MarkupRewrite? {
        var opener = NSRange()
        guard let caret, range.length > 0, range.location < s.length,
              s.attribute(.markupOpener, at: range.location, longestEffectiveRange: &opener, in: NSRange(location: 0, length: s.length)) != nil,
              NSLocationInRange(NSMaxRange(range) - 1, opener) else { return nil }
        if caret == NSMaxRange(range) {
            var p = opener.location
            while let run = hidden(at: p, in: s), NSMaxRange(run) == p, run.location < p { p = run.location }
            guard p > 0 else { return MarkupRewrite(edits: [], caret: nil) }
            let from = step(p, backward: true, in: s)
            return MarkupRewrite(edits: [(NSRange(location: from, length: p - from), "")], caret: from)
        }
        guard caret == range.location else { return nil }
        var q = NSMaxRange(opener)
        while let run = hidden(at: q, in: s), run.location == q { q = NSMaxRange(run) }
        guard q < s.length else { return MarkupRewrite(edits: [], caret: nil) }
        return MarkupRewrite(edits: [(NSRange(location: q, length: step(q, backward: false, in: s) - q), "")], caret: caret)
    }

    func copyRange(_ range: NSRange, in s: NSAttributedString) -> NSRange {
        guard range.length > 0 else { return range }
        let range = Self.outward(range, in: s)
        var a = range.location, b = NSMaxRange(range)
        if let run = Self.hidden(at: a, in: s), NSMaxRange(run) == a { a = run.location }
        if let run = Self.hidden(at: b, in: s), run.location == b {
            b = NSMaxRange(run)
            // A label copied with its destination takes its `[` too.
            if let opener = Self.opener(of: run, in: s), NSMaxRange(opener) == a { a = opener.location }
        }
        return NSRange(location: a, length: b - a)
    }

    /// The `[` of the link whose destination is `run`.
    static func opener(of run: NSRange, in s: NSAttributedString) -> NSRange? {
        guard let n = s.attribute(.markupHidden, at: run.location, effectiveRange: nil) as? Int else { return nil }
        var found: NSRange?
        s.enumerateAttribute(.markupOpener, in: NSRange(location: 0, length: run.location), options: .reverse) { value, range, stop in
            if value as? Int == n { found = range; stop.pointee = true }
        }
        return found
    }
}
