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
    let lineClamp: Int
    let overflowWrap: Int
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
            let natural = f.ascender - f.descender + f.leading
            let half = ((run.lineHeight ?? natural) - natural) / 2
            return RegionFont(owner: RegionFontOwner(f as CTFont), above: f.ascender + half,
                              below: -f.descender + f.leading + half)
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
        align = spec.align; lineClamp = spec.lineClamp; overflowWrap = spec.overflowWrap
        text = spec.runs.map(\.text).joined()
        utf16Count = offset
        sourceUTF8Bytes = spec.runs.reduce(0) { $0 + $1.text.utf8.count }
        joinedSourceUTF8Bytes = text.utf8.count
        capturedOnMainThread = Thread.isMainThread
        captureSeconds = ProcessInfo.processInfo.systemUptime - start
    }

    static func capture(_ spec: Spec, engine: TextEngine) -> RegionTextSource {
        RegionTextSource(spec, engine: engine)
    }

    func link(at index: Int) -> String? {
        guard let run = runs.first(where: { NSLocationInRange(index, $0.range) }), !run.href.isEmpty else { return nil }
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
        return value
    }
}
