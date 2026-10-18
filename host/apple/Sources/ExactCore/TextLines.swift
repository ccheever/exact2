import Foundation
import CoreText
import CExact

extension TextEngine {
    /// @ref LLP 1093 D6 — each line box's bottom, in content coordinates, of
    /// the paragraph `request` answers: the one the presenter paints.
    package func lineBottoms(_ request: ExactMeasureRequest) -> [CGFloat] {
        guard request.width >= 0, request.exclusion_count == 0, request.markup == 0 else { return [] }
        return paragraph(requestSpec(request), width: CGFloat(request.width)).lineBottoms
    }

    /// The kernel's line-box hook (`exact_set_lines`); `ctx` is the engine.
    package static let linesText: ExactLinesFn = { ctx, request, out, cap in
        guard let ctx, let request = request?.pointee else { return 0 }
        let bottoms = Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue().lineBottoms(request)
        if let out { for (i, b) in bottoms.prefix(cap).enumerated() { out[i] = Float(b) } }
        return bottoms.count
    }
}
