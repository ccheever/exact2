// @ref LLP 1044.000 §6 S1 — textual descendants are paragraph values.
import Foundation
import CoreText

package struct InlineText {
    package let id: UInt32
    package let parent: UInt32
    package let props: [String: String]
    package let lightRun: Run
    package let darkColor: [Double]?
    package let darkBackground: [Double]?
    package let colorRef: BatchValue?
    package let backgroundRef: BatchValue?
    /// Its `color` is one its author fixed (`BatchValue.fixesInk`).
    package let fixedInk: Bool
    package let paint: RunPaintRows
    package let hasSchemeColor: Bool

    package let handlers: Set<String>
    package let paints: Bool
    /// Its computed `visibility: hidden`: its text is no target
    /// (`inlineTarget`), though it hears a visible run's click inside it.
    package var hidden: Bool { lightRun.hidden }
    package var range: NSRange = NSRange(location: 0, length: 0)

    package init(id: UInt32, parent: UInt32, props: [String: String], style: InlineStyle,
         handlers: Set<String>, paints: Bool) {
        self.id = id; self.parent = parent; self.props = props
        var value = style.run
        value.text = props["text"] ?? ""
        value.href = props["href"] ?? ""
        lightRun = value
        darkColor = style.darkColor
        darkBackground = style.darkBackground
        colorRef = style.colorRef; backgroundRef = style.backgroundRef
        fixedInk = style.fixedInk
        paint = style.paint
        hasSchemeColor = style.paired || style.paint.paired
        self.handlers = handlers; self.paints = paints
    }

    package var text: String { lightRun.text }
    /// A run with an `href`: the web's `<a>` in its paragraph.
    package var isLink: Bool { !(props["href"] ?? "").isEmpty }

}

// Resolved while reading a paragraph; no dictionary of unused layout rows.
package struct InlineStyle {
    package init() {}
    package var run = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
    package var darkColor: [Double]?
    package var darkBackground: [Double]?
    /// A platform colour by name, resolved when the run is read (LLP 1095 D5).
    package var colorRef: BatchValue?
    package var backgroundRef: BatchValue?
    /// Whether `color` is one its author fixed, not a platform colour by name.
    package var fixedInk = false
    /// `text-shadow` and `-webkit-text-stroke`, computed for the run (LLP 1077).
    package var paint = RunPaintRows()
    package var paired = false
}

