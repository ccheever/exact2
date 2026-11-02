import Foundation

/// The runtime and text lifetime shared by a presented session and a headless one.
package final class SessionCore {
    package let runtime: Runtime
    package var text: TextEngine
    package init(runtime: Runtime, text: TextEngine) {
        self.runtime = runtime; self.text = text
    }
    /// A reload candidate borrows the callbacks until the transaction decides.
    package func measure(with engine: TextEngine) {
        runtime.setMeasure(TextEngine.measureText, ctx: engine.measuring.opaque)
        runtime.setFonts(TextEngine.installFonts, ctx: engine.measuring.opaque)
    }
}
