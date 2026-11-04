import Foundation
import CExact

/// Optional presentation services. A headless engine installs none.
package protocol TextPresentation: AnyObject {
    func reset()
    func checkpoint() -> () -> Void
    func controlFont(_ engine: TextEngine, size: CGFloat, weight: Int, family: Int, italic: Bool) -> PlatformFont?
    func measure(_ engine: TextEngine, _ request: ExactMeasureRequest) -> ExactMetrics?
    /// Markdown source as display runs, its inline atoms by the node's
    /// `atoms` and `atomFaces` JSON (LLP 1045.000; empty for none), in a
    /// paragraph `rtl` says runs right to left.
    func markdown(_ engine: TextEngine, source: Run, atoms: String, faces: String, rtl: Bool) -> [Run]?
    /// A Markdown text area's value as its editor lays it out: its syntax
    /// at full advance, a link's destination as nothing, each atom its pill
    /// (no wider than `width`, when the offer has one); nil where the
    /// reader's runs stand in.
    func editorMarkdown(_ engine: TextEngine, source: Run, atoms: String, faces: String, rtl: Bool, width: CGFloat?) -> [Run]?
    /// A run's paint attributes; `scale` is the engine's `lineScale`, which
    /// an inline background's content area rounds to.
    func attributes(_ run: Run, color: [Double], scale: CGFloat, attributes: inout [NSAttributedString.Key: Any])
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
    package func editorMarkdown(_ engine: TextEngine, source: Run, atoms: String, faces: String, rtl: Bool, width: CGFloat?) -> [Run]? { nil }
}
