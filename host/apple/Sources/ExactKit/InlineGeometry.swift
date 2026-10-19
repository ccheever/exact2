// @ref LLP 1051.000 D1 — inline frames are the union of the presented fragments.
import Foundation
import CoreText
import CExact
#if os(macOS)
import AppKit
#else
import UIKit
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
    func rangeFrame(_ id: UInt32, start: UInt32, end: UInt32) -> CGRect? {
        guard start <= end, let node = views[id], node.window != nil, !node.isHidden,
              let field = node.textArea, end <= node.fieldText.utf16.count else { return nil }
        var ancestor: PlatformView? = node
        while let view = ancestor {
            if view.isHidden { return nil }
            ancestor = view.superview
        }
        #if os(macOS)
        guard let manager = field.textLayoutManager, let content = manager.textContentManager,
              let first = content.location(content.documentRange.location, offsetBy: Int(start)),
              let last = content.location(content.documentRange.location, offsetBy: Int(end)),
              let range = NSTextRange(location: first, end: last) else { return nil }
        manager.ensureLayout(for: range)
        var result = CGRect.null
        manager.enumerateTextSegments(in: range, type: .standard, options: [.rangeNotRequired]) { _, rect, _, _ in
            result = result.union(rect.offsetBy(dx: field.textContainerOrigin.x, dy: field.textContainerOrigin.y))
            return true
        }
        #else
        guard let first = field.position(from: field.beginningOfDocument, offset: Int(start)),
              let last = field.position(from: field.beginningOfDocument, offset: Int(end)),
              let range = field.textRange(from: first, to: last) else { return nil }
        let result = start == end ? field.caretRect(for: first)
            : field.selectionRects(for: range).reduce(CGRect.null) { $0.union($1.rect) }
        #endif
        guard !result.isNull, !result.isInfinite else { return nil }
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
        guard let paragraph = paragraphLayout(), let text else { return [] }
        let spec = paragraphSpec(), box = paragraphBox()
        // Source ranges precede whitespace collapse and inserted hyphens.
        let start = spec.source.collapsed(run.range.location), end = spec.source.collapsed(NSMaxRange(run.range))
        let empty = run.range.length == 0
        let fit = props["fitScale"].flatMap(Float.init) ?? 1
        let font = text.font(run.run(dark: drawsDark).scaled(fit)) as CTFont
        let (above, below) = CSSLineBox.content(font)
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
            var rect = CGRect(x: x + min(x0, x1), y: box.minY + paragraph.baselines[index].rounded() - above,
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
