// CSS `font-weight` on a platform face (LLP 1019 §13 D11): UIKit's and
// AppKit's named weights at CSS's hundreds, the `wght` axis between them.
import Foundation
import CoreText
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

extension TextEngine {
    /// The named weights at CSS 100, 200 … 900.
    package static let namedWeights: [PlatformFont.Weight] = [.ultraLight, .thin, .light, .regular, .medium, .semibold, .bold, .heavy, .black]

    /// The named weight a CSS weight starts from: below 200 ultra light,
    /// from 900 black.
    package static func namedWeight(_ weight: Int) -> PlatformFont.Weight { namedWeights[min(max(weight / 100, 1), 9) - 1] }

    /// CSS `font-weight` on a platform face, `named` making the face at a
    /// hundred's named weight. At each hundred it is the named instance
    /// itself, as a UIKit (AppKit) app draws it: `600` is San Francisco's
    /// semibold at `wght` 590, where Chrome and WebKit set 600 (Charlie,
    /// 2026-10-24: "more concerned with trying to look like UIKit by
    /// default"). Between two hundreds each axis runs linearly from one named
    /// instance's value to the next (SF Mono's `YAXS` moves with its
    /// `wght`), so a transition is continuous and comes to rest on a named
    /// instance; below 100 `wght` runs from the axis minimum at 1, above 900
    /// to its maximum at 1000. A face without the axis steps.
    package static func cssWeight(_ weight: Int, size: CGFloat, named: (Int) -> PlatformFont) -> PlatformFont {
        let index = min(max(weight / 100, 1), 9) - 1, stop = (index + 1) * 100
        let low = named(stop)
        guard weight != stop, let from = wght(low) else { return low }
        let start = variation(low)
        var axes = start
        if weight < stop || index == namedWeights.count - 1 {
            let end = weight < stop ? from.min : from.max
            let t = weight < stop ? CGFloat(stop - weight) / CGFloat(stop - 1) : CGFloat(weight - stop) / 100
            axes[wghtTag] = from.value + (end - from.value) * t
        } else {
            let next = named(stop + 100)
            guard wght(next) != nil else { return low }
            let t = CGFloat(weight - stop) / 100
            for (tag, to) in variation(next) { if let a = axes[tag] { axes[tag] = a + (to - a) * t } }
        }
        guard axes != start else { return low }
        let d = CTFontDescriptorCreateWithAttributes([kCTFontVariationAttribute: axes.mapValues { $0 as NSNumber }] as CFDictionary)
        return CTFontCreateCopyWithAttributes(low as CTFont, size, nil, d) as PlatformFont
    }

    /// The CSS weight a platform face draws, the inverse of `cssWeight`
    /// through San Francisco's named instances: within 3 of one is its
    /// hundred (the other designs' differ by that much: SF Rounded's medium
    /// is 508), a value between two of them in proportion.
    /// Nil for a face without a `wght` axis.
    package static func cssWeight(of font: PlatformFont) -> Int? {
        guard let value = wght(font)?.value else { return nil }
        let stops = systemStops
        if let i = stops.firstIndex(where: { abs($0 - value) <= 3 }) { return (i + 1) * 100 }
        guard let i = stops.firstIndex(where: { $0 > value }) else { return 900 }
        guard i > 0 else { return 1 + Int((max(value - 1, 0) / (stops[0] - 1) * 99).rounded()) }
        return i * 100 + Int(((value - stops[i - 1]) / (stops[i] - stops[i - 1]) * 100).rounded())
    }

    private static let wghtTag = 0x77676874 as NSNumber // 'wght'

    /// San Francisco's `wght` at each named weight.
    private static let systemStops: [CGFloat] = namedWeights.map { wght(PlatformFont.systemFont(ofSize: 17, weight: $0))?.value ?? 400 }

    /// Every axis of a face at its value, its default where the instance sets none.
    private static func variation(_ font: PlatformFont) -> [NSNumber: CGFloat] {
        var axes: [NSNumber: CGFloat] = [:]
        for axis in CTFontCopyVariationAxes(font as CTFont) as? [[CFString: Any]] ?? [] {
            if let tag = axis[kCTFontVariationAxisIdentifierKey] as? NSNumber,
               let initial = (axis[kCTFontVariationAxisDefaultValueKey] as? NSNumber)?.doubleValue { axes[tag] = CGFloat(initial) }
        }
        for (tag, value) in CTFontCopyVariation(font as CTFont) as? [NSNumber: NSNumber] ?? [:] { axes[tag] = CGFloat(value.doubleValue) }
        return axes
    }

    /// A face's `wght` (its axis default where the instance sets none) and
    /// the axis's range; nil without the axis. Kept per font object, which is
    /// immutable: Core Text builds every axis's localized name to answer
    /// `CTFontCopyVariationAxes`, and measuring controls asked it for the
    /// same face on every commit (44% of the owner thread's work while a list
    /// filled rows on the Mac).
    private static func wght(_ font: PlatformFont) -> (value: CGFloat, min: CGFloat, max: CGFloat)? {
        wghtLock.lock()
        let kept = wghtKept.object(forKey: font)
        wghtLock.unlock()
        if let kept { return kept.wght }
        let wght = readWght(font)
        wghtLock.lock()
        wghtKept.setObject(KeptWght(wght), forKey: font)
        wghtLock.unlock()
        return wght
    }
    private final class KeptWght {
        let wght: (value: CGFloat, min: CGFloat, max: CGFloat)?
        init(_ wght: (value: CGFloat, min: CGFloat, max: CGFloat)?) { self.wght = wght }
    }
    /// Gone with its font; the main and owner threads both measure.
    private static let wghtKept = NSMapTable<PlatformFont, KeptWght>.weakToStrongObjects()
    private static let wghtLock = NSLock()
    private static func readWght(_ font: PlatformFont) -> (value: CGFloat, min: CGFloat, max: CGFloat)? {
        let tag = wghtTag.uint32Value
        guard let axis = (CTFontCopyVariationAxes(font as CTFont) as? [[CFString: Any]])?
            .first(where: { ($0[kCTFontVariationAxisIdentifierKey] as? NSNumber)?.uint32Value == tag }),
              let low = (axis[kCTFontVariationAxisMinimumValueKey] as? NSNumber)?.doubleValue,
              let high = (axis[kCTFontVariationAxisMaximumValueKey] as? NSNumber)?.doubleValue,
              let initial = (axis[kCTFontVariationAxisDefaultValueKey] as? NSNumber)?.doubleValue else { return nil }
        let set = (CTFontCopyVariation(font as CTFont) as? [NSNumber: NSNumber])?[wghtTag]?.doubleValue
        return (CGFloat(set ?? initial), CGFloat(low), CGFloat(high))
    }
}
