// UI-resolved immutable font/source capture. CoreText layout stays on the worker.
import Foundation
import CoreText
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// CoreText permits concurrent use of immutable font objects. Layout objects
/// (typesetter/line/run) stay entirely inside RegionTextLayout on its worker.
/// https://developer.apple.com/documentation/coretext
final class RegionFontOwner: @unchecked Sendable {
    // The SDK omits Sendable on CTFont. This exception covers only the retained
    // immutable font object Apple explicitly permits concurrent use of, never a
    // layout object, attributed string, engine, cache, or resolver closure.
    let value: CTFont
    init(_ value: CTFont) { self.value = value }
}

struct RegionFont: Sendable {
    let owner: RegionFontOwner
    var value: CTFont { owner.value }
    let above: CGFloat
    let below: CGFloat
}

struct RegionTextRun: Sendable {
    let text: String
    let font: RegionFont
    let lineHeight: CGFloat?
    let letterSpacing: CGFloat
    let color: CGColor
    let underline: Bool
    let strike: Bool
    let href: String
    let range: NSRange
}

final class RegionTextSource: Sendable {
    let runs: [RegionTextRun]
    let strut: RegionFont?
    let strutExplicit: Bool
    let align: Int
    let direction: Int
    let lineClamp: Int
    let overflowWrap: Int
    /// A preserved tab's stop interval (CSS `tab-size: 8`), as the engine's
    /// paragraphs have it; nil when no tab is preserved.
    let tabInterval: CGFloat?
    let text: String
    let utf16Count: Int
    let sourceUTF8Bytes: Int
    /// Logical joined UTF8 length, not distinct allocated storage: joining one
    /// run may share its String. CF attributed storage is worker-local extra.
    let joinedSourceUTF8Bytes: Int
    let captureSeconds: Double
    let capturedOnMainThread: Bool

    private init(_ spec: Spec, engine: TextEngine) {
        precondition(Thread.isMainThread, "font and appearance resolution belongs to UI")
        let start = ProcessInfo.processInfo.systemUptime
        func font(_ run: Run) -> RegionFont {
            let f = engine.font(size: run.size, weight: run.weight, family: run.family, italic: run.italic)
            let (above, below) = CSSLineBox.extents(f as CTFont, height: run.lineHeight)
            return RegionFont(owner: RegionFontOwner(f as CTFont), above: above, below: below)
        }
        var offset = 0
        runs = spec.runs.map { run in
            let count = run.text.utf16.count
            defer { offset += count }
            return RegionTextRun(text: run.text, font: font(run), lineHeight: run.lineHeight,
                                 letterSpacing: run.letterSpacing,
                                 color: TextEngine.color(run.color ?? spec.color).cgColor,
                                 underline: run.decoration.contains("underline") || (run.decoration.isEmpty && !run.href.isEmpty),
                                 strike: run.decoration.contains("line-through"), href: run.href,
                                 range: NSRange(location: offset, length: count))
        }
        let base = spec.strut ?? spec.runs.first
        strut = base.map(font); strutExplicit = base?.lineHeight != nil
        align = spec.align; direction = spec.direction; lineClamp = spec.lineClamp; overflowWrap = spec.overflowWrap
        text = spec.runs.map(\.text).joined()
        tabInterval = spec.preserves && text.contains("\t") ? base.map { TextEngine.tabInterval(font($0).value, letterSpacing: $0.letterSpacing) } : nil
        utf16Count = offset
        sourceUTF8Bytes = spec.runs.reduce(0) { $0 + $1.text.utf8.count }
        joinedSourceUTF8Bytes = text.utf8.count
        capturedOnMainThread = Thread.isMainThread
        captureSeconds = ProcessInfo.processInfo.systemUptime - start
    }

    static func capture(_ spec: Spec, engine: TextEngine) -> RegionTextSource {
        RegionTextSource(spec, engine: engine)
    }

    /// Authored ranges are ordered and disjoint, including empty runs. CoreText
    /// may coalesce several of them into one glyph run; visit only its overlaps.
    func runs(overlapping range: NSRange) -> ArraySlice<RegionTextRun> {
        var lo = 0, hi = runs.count
        while lo < hi {
            let mid = lo + (hi - lo) / 2
            if NSMaxRange(runs[mid].range) <= range.location { lo = mid + 1 }
            else { hi = mid }
        }
        let start = lo
        while lo < runs.count && runs[lo].range.location < NSMaxRange(range) { lo += 1 }
        return runs[start..<lo]
    }

    func link(at index: Int) -> String? {
        guard let run = runs(overlapping: NSRange(location: index, length: 1)).first,
              !run.href.isEmpty else { return nil }
        return run.href
    }

    /// Locally mutable Foundation builder; only immutable font/color/scalars
    /// from capture are used. No AppKit font manager, resolver or TextEngine.
    func attributed() -> NSAttributedString {
        let value = NSMutableAttributedString()
        for run in runs {
            var attributes: [NSAttributedString.Key: Any] = [
                NSAttributedString.Key(kCTFontAttributeName as String): run.font.value,
                NSAttributedString.Key(kCTForegroundColorAttributeName as String): run.color,
            ]
            if run.letterSpacing != 0 { attributes[.kern] = run.letterSpacing }
            if run.underline { attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue }
            if run.strike { attributes[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
            value.append(NSAttributedString(string: run.text, attributes: attributes))
        }
        TextEngine.setBaseDirection(value, direction: direction)
        if let tabInterval { TextEngine.setTabStops(value, interval: tabInterval) }
        return value
    }
}
