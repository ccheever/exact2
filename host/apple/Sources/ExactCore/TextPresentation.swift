import Foundation
import CExact

/// Optional presentation services. A headless engine installs none.
package protocol TextPresentation: AnyObject {
    func reset()
    func checkpoint() -> () -> Void
    func controlFont(_ engine: TextEngine, size: CGFloat, weight: Int, family: Int, italic: Bool) -> PlatformFont?
    func measure(_ engine: TextEngine, _ request: ExactMeasureRequest) -> ExactMetrics?
    func markdown(_ engine: TextEngine, source: Run) -> [Run]?
    func attributes(_ run: Run, color: [Double], attributes: inout [NSAttributedString.Key: Any])
}
