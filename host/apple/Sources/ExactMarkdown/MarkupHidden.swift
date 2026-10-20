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
        var runs: [NSRange] = []
        shown.enumerateAttribute(.markupHidden, in: NSRange(location: 0, length: shown.length)) { value, run, _ in
            if value != nil { runs.append(run) }
        }
        for run in runs.reversed() {
            shown.replaceCharacters(in: run, with: String(repeating: "\u{2060}", count: run.length))
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

    func rewrite(_ range: NSRange, with text: String, in s: NSAttributedString) -> MarkupRewrite? {
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
            // Only hidden source: a deletion removes nothing; text lands
            // where a caret there sits, after the run.
            guard !text.isEmpty else { return MarkupRewrite(edits: [], caret: nil) }
            let place = Self.hidden(at: range.location, in: s).map { NSMaxRange($0) } ?? range.location
            return MarkupRewrite(edits: [(NSRange(location: place, length: 0), text)], caret: place + text.utf16.count)
        }
        let edits = parts.enumerated().map { (i, part) in (range: part, text: i == 0 ? text : "") }
        return MarkupRewrite(edits: edits, caret: first.location + text.utf16.count)
    }

    func copyRange(_ range: NSRange, in s: NSAttributedString) -> NSRange {
        guard range.length > 0 else { return range }
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
