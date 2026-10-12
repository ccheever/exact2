// What a press and a disabled state show on a pressable that is no native
// control, on the Mac (LLP 1115.000 D6): what AppKit shows on a borderless
// `NSButton`. A card, a custom button and a `link` take the system's pressed
// effect while the mouse is down inside them, and half their opacity while
// disabled. A default yields only to the author's version of the same state:
// authored press feedback replaces the pressed look, and a `hover` handler,
// which only observes, replaces nothing. A native control keeps its own
// feedback, and a row of a `list` its cell's (D16). There is no hover look.
#if os(macOS)
import AppKit

extension NodeView {
    /// AppKit's pressed effect as a wash over the node's own ink. A
    /// borderless `NSButton` shows its image and tinted title through
    /// `NSColor.withSystemEffect(.pressed)` while the mouse is down, measured
    /// on macOS 27.0: in a light appearance each channel c becomes
    /// 0.92c − 0.08 (white 1.00 to 0.84, mid grey 0.50 to 0.38), in a dark
    /// one c + 0.18 (black to 0.18, mid grey to 0.68). It darkens in light and
    /// lightens in dark; it is no fade. Black at this alpha over the ink
    /// (source-atop) is the same at white and within 0.06 of it below; white
    /// at this alpha is the same at black and falls short on light tones
    /// (0.59 for mid grey).
    static let pressedWash: (light: CGFloat, dark: CGFloat) = (0.16, 0.18)
    /// A disabled borderless `NSButton` draws its image at half its alpha
    /// (0.50 measured) and its title near it (0.58).
    static let disabledOpacity: CGFloat = 0.5
    private static let washName = "exact.pressWash"

    /// A row of a `list`: the list's child, or a `section`'s in it. Its
    /// press and its disabled look are a cell's (LLP 1115.000 D16, which is
    /// of a vertical list's rows). An item of a list that scrolls sideways
    /// is a carousel's card, not a row, as the web host has it.
    var isListRow: Bool {
        func nodeAbove(_ view: NSView) -> NodeView? {
            var above = view.superview
            while let next = above, !(next is NodeView) { above = next.superview }
            return above as? NodeView
        }
        guard let parent = nodeAbove(self) else { return false }
        let list = parent.kind == "list" ? parent : parent.props["semanticTag"] == "section" ? nodeAbove(parent) : nil
        guard let list, list.kind == "list" else { return false }
        return presenter?.collections.entries[list.id]?.snapshot.horizontal != true
    }

    /// A pressable that is the author's own box, not AppKit's control and
    /// not a list's row: what the platform's press and disabled looks are for.
    private var customPressable: Bool {
        (pressable || kind == "button") && !isNativeButton && !isSurfaceControl && kind != "control" && !isListRow
    }

    /// Whether a press shows the platform's look: a node this host presses,
    /// with no `-exact-press-scale` written (any value, 1 included, is the
    /// author's), no `pointerdown` or `pointerup` of its own (the author
    /// paints its pressed state from those; a `hover` handler replaces
    /// nothing), and not a scrim covering the window. The same test iOS's
    /// dim makes (`platformPressDims`), less a list's rows.
    var platformPressTints: Bool {
        guard pressable, customPressable, style["press_scale"] == nil,
              !handlers.contains("pointerdown"), !handlers.contains("pointerup") else { return false }
        if let content = window?.contentView {
            let box = convert(bounds, to: nil)
            if box.width >= content.bounds.width, box.height >= content.bounds.height * 0.9 { return false }
        }
        return true
    }

    /// The pressed look on (`down`) or off, at once both ways, as an
    /// `NSButton` highlights: a layer over the node's views, masked by the
    /// node's own picture as it stands, so it tints the ink and nothing
    /// around it (a card's whole box, a text button's glyphs). The picture
    /// is the capture's copy of the node's layers, their contents shared: it
    /// is drawn in the geometry the real ones are, and costs no bitmap.
    func tintPress(_ down: Bool) {
        guard let layer else { return }
        let showing = layer.sublayers?.first { $0.name == Self.washName }
        guard down else { showing?.removeFromSuperlayer(); return }
        guard showing == nil, bounds.width > 0, bounds.height > 0 else { return }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let dark = effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        let wash = CALayer(), ink = Capture.paintOrderLayer(of: self, root: layer)
        wash.name = Self.washName
        wash.frame = layer.bounds
        wash.backgroundColor = dark ? CGColor(gray: 1, alpha: Self.pressedWash.dark) : CGColor(gray: 0, alpha: Self.pressedWash.light)
        // The copy stands where the node does in its parent: here it is the wash's own box, whole and unshadowed.
        ink.transform = CATransform3DIdentity
        ink.frame = wash.bounds
        ink.opacity = 1; ink.isHidden = false; ink.shadowOpacity = 0; ink.mask = nil
        wash.mask = ink
        wash.zPosition = 1 // over the node's own views, whose ranks are thousandths
        layer.addSublayer(wash)
    }
    /// Whether the pressed look is showing (the agent's `layout`, the tests).
    var pressTinted: Bool { layer?.sublayers?.contains { $0.name == Self.washName } == true }

    /// A disabled custom pressable shows as a disabled control does, at half
    /// its opacity, and takes no press (`mouseDown`). Written when the state
    /// changes; the presented opacity is multiplied in as it arrives.
    func syncDisabledLook() {
        let dims = disabled && customPressable
        guard dims != disabledShown else { return }
        disabledShown = dims
        alphaValue = presentedAlpha * (dims ? Self.disabledOpacity : 1)
    }
}
#endif
