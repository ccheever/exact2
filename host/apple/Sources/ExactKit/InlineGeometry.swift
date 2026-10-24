// @ref LLP 1051.000 D1 — inline frames are the union of the presented fragments.
import Foundation
import CoreText
import CExact
import ExactCore
#if os(macOS)
import AppKit
private typealias PlatformFont = NSFont
#else
import UIKit
private typealias PlatformFont = UIFont
#endif

extension Runtime {
    func setInlineGeometry(_ read: @escaping (UInt32) -> CGRect?, range: @escaping (UInt32, UInt32, UInt32) -> CGRect?) {
        inlineFrame = read
        rangeFrame = range
        let ctx = UInt(bitPattern: Unmanaged.passUnretained(self).toOpaque())
        on {
            exact_set_inline_geometry(rt, Self.readInlineFrame, Self.readRangeFrame, UnsafeMutableRawPointer(bitPattern: ctx))
        }
    }

    private static let readRangeFrame: ExactRangeFrameFn = { ctx, id, start, end in
        guard let ctx else { return ExactInlineFrame() }
        let runtime = Unmanaged<Runtime>.fromOpaque(ctx).takeUnretainedValue()
        return Owner.shared.callMain {
            guard let rect = runtime.rangeFrame?(id, start, end) else { return ExactInlineFrame() }
            return ExactInlineFrame(x: Double(rect.minX), y: Double(rect.minY), width: Double(rect.width), height: Double(rect.height), available: 1)
        }
    }

    private static let readInlineFrame: ExactInlineFrameFn = { ctx, id in
        guard let ctx else { return ExactInlineFrame() }
        let runtime = Unmanaged<Runtime>.fromOpaque(ctx).takeUnretainedValue()
        return Owner.shared.callMain {
            guard let rect = runtime.inlineFrame?(id) else { return ExactInlineFrame() }
            return ExactInlineFrame(x: Double(rect.minX), y: Double(rect.minY), width: Double(rect.width), height: Double(rect.height), available: 1)
        }
    }
}

extension Presenter {
    /// `rangeFrame` (LLP 1114.007): a textarea's UTF-16 range as TextKit
    /// last laid it out, a plain one's or a Markdown one's source alike, in
    /// viewport points; collapsed, the caret's rectangle. Nil past the
    /// field's text, while text is marked, or when the field is hidden. A
    /// Markdown reader answers its atoms' pills (LLP 1045.000 D7).
    func rangeFrame(_ id: UInt32, start: UInt32, end: UInt32) -> CGRect? {
        if let node = views[id], node.textArea == nil, node.props["markup"] == "markdown" {
            var ancestor: PlatformView? = node
            while let view = ancestor {
                if view.isHidden { return nil }
                ancestor = view.superview
            }
            guard start <= end, node.window != nil, !node.cssVisibilityHidden, let rect = node.readerAtomFrame(Int(start), Int(end)) else { return nil }
            return Agent.box(rect, of: node, viewport: viewport)
        }
        guard start <= end, let node = views[id], node.window != nil, !node.isHidden, !node.fieldComposing,
              let field = node.textArea, end <= node.fieldText.utf16.count else { return nil }
        var ancestor: PlatformView? = field
        while let view = ancestor {
            if view.isHidden { return nil }
            ancestor = view.superview
        }
        // Each TextKit 2 segment's text box: its glyphs across and its font's
        // content area down (ascent over descent at the segment's baseline),
        // not the line fragment, which a paragraph's line height makes
        // taller (the grammar's, as Chrome and Linux answer; theme2's 22:27).
        guard let manager = field.textLayoutManager, let content = manager.textContentManager,
              let first = content.location(content.documentRange.location, offsetBy: Int(start)),
              let last = content.location(content.documentRange.location, offsetBy: Int(end)),
              let range = NSTextRange(location: first, end: last) else { return nil }
        manager.ensureLayout(for: range)
        #if os(macOS)
        let origin = field.textContainerOrigin
        guard let storage = field.textStorage else { return nil }
        #else
        let origin = CGPoint(x: field.textContainerInset.left, y: field.textContainerInset.top)
        let storage = field.textStorage
        #endif
        let length = storage.length
        #if os(tvOS)
        // tvOS has no `systemFontSize`; its body style is the system's text size there.
        let systemSize = PlatformFont.preferredFont(forTextStyle: .body).pointSize
        #else
        let systemSize = PlatformFont.systemFontSize
        #endif
        let fallback = field.font ?? field.typingAttributes[.font] as? PlatformFont ?? PlatformFont.systemFont(ofSize: systemSize)
        // A face's content area around `baseline` (its `baselineOffset`
        // raising it), as this host's CSS line box rounds it.
        func area(_ face: PlatformFont, offset: CGFloat, baseline: CGFloat) -> (CGFloat, CGFloat) {
            let (ascent, descent) = CSSLineBox.content(face as CTFont)
            return (baseline - offset - ascent, baseline - offset + descent)
        }
        func face(at index: Int) -> (PlatformFont, CGFloat) {
            guard index >= 0, index < length else { return (fallback, 0) }
            let font = storage.attribute(.font, at: index, effectiveRange: nil) as? PlatformFont ?? fallback
            let offset = (storage.attribute(.baselineOffset, at: index, effectiveRange: nil) as? NSNumber).map { CGFloat($0.doubleValue) } ?? 0
            return (font, offset)
        }
        // The text box over `chars` at `baseline` (y down): every font run's
        // content area in it, or a caret's (the character after it, else
        // before, else the field's font).
        func extent(_ chars: NSRange, caret: Int, baseline: CGFloat) -> (CGFloat, CGFloat) {
            guard chars.length > 0 else {
                let (font, offset) = face(at: caret < length ? caret : caret - 1)
                return area(font, offset: offset, baseline: baseline)
            }
            var (top, bottom) = (CGFloat.infinity, -CGFloat.infinity)
            var at = chars.location
            while at < NSMaxRange(chars) {
                var run = NSRange()
                _ = storage.attributes(at: at, longestEffectiveRange: &run, in: chars)
                let (font, offset) = face(at: at)
                let (t, b) = area(font, offset: offset, baseline: baseline)
                (top, bottom) = (min(top, t), max(bottom, b))
                at = max(NSMaxRange(run), at + 1)
            }
            return (top, bottom)
        }
        let offset = { (location: NSTextLocation) in content.offset(from: content.documentRange.location, to: location) }
        var result = CGRect.null
        // An empty range's `.selection` segment is the insertion point's;
        // TextKit gives each its baseline from the segment's top.
        manager.enumerateTextSegments(in: range, type: start == end ? .selection : .standard, options: []) { segment, rect, baseline, _ in
            let from = segment.map { offset($0.location) } ?? Int(start), to = segment.map { offset($0.endLocation) } ?? from
            let (top, bottom) = extent(NSRange(location: from, length: max(0, to - from)), caret: from, baseline: rect.minY + baseline)
            result = result.union(CGRect(x: rect.minX + origin.x, y: top + origin.y, width: rect.width, height: bottom - top))
            return true
        }
        // No segment (an empty field TextKit has laid nothing out for): the
        // caret's place, and its text box at the line's baseline.
        if result.isNull, start == end, let fragment = manager.textLayoutFragment(for: first) {
            let line = fragment.textLineFragments.last
            let baseline = fragment.layoutFragmentFrame.minY + (line.map { $0.typographicBounds.minY + $0.glyphOrigin.y } ?? 0)
            let (top, bottom) = extent(NSRange(location: Int(start), length: 0), caret: Int(start), baseline: baseline)
            let x = fragment.layoutFragmentFrame.minX + (line?.locationForCharacter(at: 0).x ?? 0)
            result = CGRect(x: x + origin.x, y: top + origin.y, width: 0, height: bottom - top)
        }
        guard !result.isNull, !result.isInfinite else { return nil }
        if start == end { result.size.width = 0 }
        return Agent.box(result, of: field, viewport: viewport)
    }

    func inlineFrame(_ id: UInt32) -> CGRect? {
        guard let run = inlineText(id), let owner = textHost(id), owner.window != nil else { return nil }
        let rect = owner.inlineRects(run).map { Agent.box($0, of: owner, viewport: viewport) }
            .reduce(CGRect.null) { $0.union($1) }
        return rect.isNull ? nil : rect
    }
}

extension Agent {
    func inlineLayout() -> [[String: Any]] {
        presenter.inlineOwners.keys.sorted().compactMap { id in
            guard let r = presenter.inlineFrame(id) else { return nil }
            return ["id": Int(id), "x": Self.r2(r.minX), "y": Self.r2(r.minY), "w": Self.r2(r.width), "h": Self.r2(r.height)]
        }
    }
}

extension NodeView {
    func inlineBounds(_ run: InlineText) -> CGRect? {
        let rect = inlineRects(run).reduce(CGRect.null) { $0.union($1) }
        return rect.isNull ? nil : rect
    }

    func inlineRects(_ run: InlineText) -> [CGRect] {
        guard let text else { return [] }
        let spec = paragraphSpec()
        // Source ranges precede whitespace collapse and inserted hyphens.
        let start = spec.source.collapsed(run.range.location), end = spec.source.collapsed(NSMaxRange(run.range))
        let fit = props["fitScale"].flatMap(Float.init) ?? 1
        return shapedRects(start, end, empty: run.range.length == 0, font: text.font(run.run(dark: drawsDark).scaled(fit)) as CTFont)
    }

    /// The fragments of the shaped text `start..<end` as the paragraph lays it
    /// out, each its line's slice of `font`'s content area.
    func shapedRects(_ start: Int, _ end: Int, empty: Bool, font: CTFont) -> [CGRect] {
        guard let paragraph = paragraphLayout() else { return [] }
        let spec = paragraphSpec(), box = paragraphBox()
        let (above, below) = CSSLineBox.content(font)
        #if os(iOS) || os(tvOS)
        let scale = traitCollection.displayScale
        #else
        let scale: CGFloat = 1
        #endif
        var placedEmpty = false
        return paragraph.lines.indices.compactMap { index in
            let line = paragraph.lines[index], range = CTLineGetStringRange(line)
            let lo = max(start, range.location), hi = min(end, range.location + range.length)
            if empty {
                guard !placedEmpty, start >= range.location, start <= range.location + range.length else { return nil }
                placedEmpty = true
            } else if hi <= lo { return nil }
            let x = box.minX + paragraph.origin(index, align: spec.align, width: box.width)
            let x0 = CTLineGetOffsetForStringIndex(line, lo, nil), x1 = CTLineGetOffsetForStringIndex(line, hi, nil)
            var rect = CGRect(x: x + min(x0, x1), y: box.minY + TextEngine.paintedBaseline(paragraph.baselines[index], scale: scale) - above,
                              width: abs(x1 - x0), height: above + below)
            // An empty inline box keeps its insertion point and font height.
            // A nonempty run covering only hanging wrap space has no fragment.
            if !empty && spec.whiteSpace != 1 && spec.whiteSpace != 4 {
                let trailing = CGFloat(CTLineGetTrailingWhitespaceWidth(line))
                let width = CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)) - trailing
                let left = spec.direction == 1 ? x + trailing : x
                let right = left + width
                let start = max(rect.minX, left), end = min(rect.maxX, right)
                guard end > start else { return nil }
                rect.origin.x = start; rect.size.width = end - start
            }
            if let fragments = columnRecord?.fragments, !fragments.isEmpty {
                guard let fragment = fragments.first(where: { $0.lines.contains(index) }) else { return nil }
                rect = rect.offsetBy(dx: fragment.dx, dy: fragment.dy)
            }
            return rect
        }
    }
}
