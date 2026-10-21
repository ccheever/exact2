// A system symbol's natural size (`exact_set_symbol_measure`), which the kernel
// asks for before laying out. The UIKit and AppKit symbol views take their
// point size and weight from `configuration` too. In the core so a headless
// session measures a symbol as a presented one does.
import CExact
import Foundation
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

package enum SymbolMeasure {
    #if canImport(UIKit)
    package typealias Configuration = UIImage.SymbolConfiguration
    #else
    package typealias Configuration = NSImage.SymbolConfiguration
    #endif

    /// A symbol's point size and weight, as its view and its measure take
    /// them: a CSS weight is the nearest of the nine symbol weights.
    package static func configuration(points: CGFloat, weight: CGFloat) -> Configuration {
        let index = min(8, max(0, Int((weight / 100).rounded()) - 1))
        #if canImport(UIKit)
        let weights: [UIImage.SymbolWeight] = [.ultraLight, .thin, .light, .regular, .medium, .semibold, .bold, .heavy, .black]
        #else
        let weights: [NSFont.Weight] = [.ultraLight, .thin, .light, .regular, .medium, .semibold, .bold, .heavy, .black]
        #endif
        return Configuration(pointSize: points > 0 ? points : 1, weight: weights[index])
    }

    /// The box a symbol takes: its glyph's, an em square when the OS has no
    /// glyph by that name (LLP 1035.004.000 D2), none at a point size of 0.
    package static func size(_ name: String, points: CGFloat, weight: CGFloat) -> CGSize? {
        guard points > 0 else { return nil }
        let config = configuration(points: points, weight: weight)
        #if canImport(UIKit)
        let image = name.isEmpty ? nil : UIImage(systemName: name, withConfiguration: config)
        #else
        let image = name.isEmpty ? nil : NSImage(systemSymbolName: name, accessibilityDescription: nil)?.withSymbolConfiguration(config)
        #endif
        return image?.size ?? CGSize(width: points, height: points)
    }

    /// The kernel's symbol measurer, on the runtime's thread.
    package static let measure: ExactSymbolFn = { _, name, length, points, weight, out in
        guard let out else { return 0 }
        let text = name.map { String(decoding: UnsafeBufferPointer(start: $0, count: length), as: UTF8.self) } ?? ""
        // A symbol at 0 points takes no room: a known size, not one to wait for
        let size = size(text, points: CGFloat(points), weight: CGFloat(weight)) ?? .zero
        out[0] = Float(size.width); out[1] = Float(size.height)
        return 1
    }
}
