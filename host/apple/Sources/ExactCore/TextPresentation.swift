import Foundation
import CExact

/// Optional presentation services. A headless engine installs none.
package protocol TextPresentation: AnyObject {
    func reset()
    func checkpoint() -> () -> Void
    func controlFont(_ engine: TextEngine, size: CGFloat, weight: Int, family: Int, italic: Bool) -> PlatformFont?
    func measure(_ engine: TextEngine, _ request: ExactMeasureRequest) -> ExactMetrics?
    /// Markdown source as display runs, its inline atoms by the node's
    /// `atoms` and `atomFaces` JSON (LLP 1045.000; empty for none).
    func markdown(_ engine: TextEngine, source: Run, atoms: String, faces: String) -> [Run]?
    func attributes(_ run: Run, color: [Double], attributes: inout [NSAttributedString.Key: Any])
    /// The paragraph's whole attributed string, once every run's are set:
    /// what spans runs (an inline box's extent) is read here.
    func finish(_ string: NSMutableAttributedString)
}

package extension NSAttributedString.Key {
    /// A truncated line's "…" (`TextEngine.ellipsis`, `clampedLine`).
    static let exactTruncation = NSAttributedString.Key("ExactTruncation")
    /// The block container's token style, shared by layout and raster workers.
    static let exactClampStyle = NSAttributedString.Key("ExactClampStyle")
}

extension TextPresentation {
    package func finish(_ string: NSMutableAttributedString) {}
}
