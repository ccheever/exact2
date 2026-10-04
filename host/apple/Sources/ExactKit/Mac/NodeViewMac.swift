// One presenter identity per kernel node (LLP 1008 §5); inline text stays
// unmounted as its paragraph's run data (LLP 1033). Mounted NSViews draw
// backgrounds, borders, and text are drawn; frames come from the kernel's
// layout; transforms and opacity from presentation values. Everything a
// node reaches beyond itself — the text engine, the canvases, the web
// views, the session's clock — it reaches through its presenter's session
// (LLP 1031 D1), never a global.
#if os(macOS)
import AppKit
import IOSurface
/// A material paints, but never supplies a new hit target or focus owner.
private final class MaterialContent: NSView {
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = raisedHit(super.hitTest(point), point)
        return hit === self ? nil : hit
    }
}

@available(macOS 26.0, *)
private final class GlassBackground: NSGlassEffectView {
    override func hitTest(_ point: NSPoint) -> NSView? {
        contentView?.hitTest(convert(point, from: superview))
    }
}

private final class BlurBackground: NSVisualEffectView {
    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = raisedHit(super.hitTest(point), point)
        return hit === self ? nil : hit
    }
}

final class NodeView: NSView, NSTextViewDelegate, NSTextFieldDelegate {
    override func selectAll(_ sender: Any?) { presenter?.selection.selectAll() }

    let id: UInt32
    // Twice the kernel rank; transients never overwrite the authored answer.
    var rank: Int64 = 0
    var paintLifted = false
    var paintGhost = false
    var paintGhosts = 0
    weak var paintParent: PaintView?
    weak var ghostParent: NodeView?
    let firstDraw: () -> Void
    let kind: String
    var inlineText: [InlineText] = []
    var inlinePressed: UInt32?
    /// An SVG element a click in this `svg` went down on (LLP 1055.000 D17).
    var svgPressed: UInt32?
    var cachedTextSpec: Spec?
    var textLayoutValid = false
    /// The paragraph's text as a worker-painted surface (TextRasterMac.swift).
    var textRaster: IOSurface?
    var textRasterScale: CGFloat = 2
    var textRasterFrame: CGRect = .zero
    var textRasterOverflowLayer: CALayer?
    var textRasterKey: TextRasterKey?
    var textRasterReady = false
    var textRasterFailed = false
    var textRasterPending = false
    var textRasterUsesStrips = false
    var flowShapes: [TextFlowShape] = []
    var cachedTextLayout: (width: CGFloat, paragraph: Paragraph)?
    var liveText: String?
    var props: [String: String] = [:] { didSet { presenter?.propsChanged(self) } }
    var style: NodeStyle = [:]
    /// What the host's own writers hid (a covered route, a tab a native control
    /// stands in for, a placement); a `display: none` box is hidden besides, as
    /// CSS removes it with its subtree, or its texts paint at its 0×0 frame's
    /// origin (recipes F19). Reading says whether it is hidden, for either reason.
    private var hostHidden = false
    /// Whether the host hid this view, whatever CSS's `display` says: what a
    /// projection or a placement saves and restores, since `isHidden` also
    /// reads `display: none` (review B1: restoring that wrote CSS's bit into
    /// the host's and kept the view hidden once it was displayed).
    var hiddenByHost: Bool { hostHidden }
    override var isHidden: Bool {
        get { super.isHidden }
        set {
            // Writing back "hidden" that only CSS's `display: none` made is not
            // the host's word (a save of `isHidden` restored; review B1): a
            // projection that means it hides again on its next pass.
            let css = style["display"]?.string == "none"
            if !(newValue && css && !hostHidden && super.isHidden) { hostHidden = newValue }
            super.isHidden = hostHidden || css
        }
    }
    var clipPath: CGPath?, clipRule = CGPathFillRule.winding
    var handlers: Set<String> = [] { didSet { video?.update(); if handlers.contains("hover") != oldValue.contains("hover") || handlers.contains("pointermove") != oldValue.contains("pointermove") { syncHoverTracking() }; if handlers.contains("drop") != oldValue.contains("drop") { syncDropTypes() } } } // the media events the player reports; a hover handler's tracking area; a drop handler's dragged types
    var translate = CGPoint.zero, layoutOffset = CGPoint.zero, layoutScale = CGPoint(x: 1, y: 1) // layout*: the box layout moved it from (LLP 1063)
    var surface: SurfaceLayer? { didSet { layerPaintCache = nil } } // its surface at a layout transition's size (`Surface.swift`)
    /// How far its frame stands from layout's: a lifted Arrange row's
    /// translation plus `stickyOffset` (`applyTransform`).
    var arrangeShift = CGPoint.zero
    /// How far its scroller's scroll moves a sticky box (LLP 1083, `Sticky.swift`).
    var stickyOffset = CGPoint.zero
    var scale: CGFloat = 1
    var rotate: CGFloat = 0
    weak var presenter: Presenter?
    var textArea: NSTextView?
    var textAreaScroll: NSScrollView?
    var field: NSTextField?
    /// A `value` that arrived mid-composition, applied when it ends.
    var pendingValue: String?
    var scroll: ChainingScrollView?
    /// `box-shadow` (`BoxShadow.swift`): outer, and inset (LLP 1077 D4).
    var shadowCaster: ShadowCaster?
    var insetCaster: InsetShadowCaster?
    var clipBox: NSView?
    /// The box's border, gradient and image pixels as sublayers (`BoxLayerMac.swift`).
    var boxBorder: CALayer?
    var boxFill: CALayer?
    /// `drawsPaint`, kept: AppKit asks `wantsUpdateLayer` of every view as it
    /// builds the layer tree each display cycle, and the decision reads
    /// colours and radii. Cleared by whatever it reads (style, size, clip,
    /// raster, symbol, surface).
    var layerPaintCache: Bool?
    /// What `applyProps` last wrote to AppKit's accessibility (see there).
    struct AccessibilityWrite: Equatable { var enabled = true; var identifier: String?; var label: String? }
    var lastAccessibilityWrite = AccessibilityWrite()
    var boxGradient: CAGradientLayer?
    var imageLayer: CALayer?
    /// While it flies as a shared element (LLP 1013.000 D4): where its image is drawn.
    var flightLook: FlightLook?
    var materialView: NSView?
    /// `glassGroup`'s view and a grouped glass's isolation (`GlassGroup.swift`).
    var glassGroupView: NSView?
    var glassIsolation: NSView?
    var materialContent: NSView?
    private var materialKind: String?
    /// Natural extent from the kernel, before the CSS client-size minimum.
    var content = CGSize.zero
    /// The platform view returned by the dlopened iframe arm (@ref LLP 1020 D3).
    var video: VideoView?
    var web: NSView?
    /// A canvas node's Metal layer (LLP 1009).
    var metal: MetalView?
    var canvasInput: CanvasInput?
    /// A canvas's children live here (LLP 1014): laid out by the kernel in
    /// the canvas's box, over the Metal layer; when the surface samples them
    /// they are painted into its children texture and this view composites
    /// at alpha 0. `needsCapture`: painted again at the next capture;
    /// `paintedThisTurn`: a draw on this turn is the capture's own.
    var overlay: FlippedView?
    var needsCapture = false
    var paintedThisTurn = false
    /// Where a canvas's surface put this direct child (LLP 1014 D5): a 3×3
    /// homography, row major, from this node's own points to the canvas's,
    /// then its depth (larger nearer); `nil` is the kernel's frame.
    /// Hit-testing inverts it, nearest child first; accessibility reports the
    /// mapped box.
    var placement: [Double]?
    private var hiddenBeforePlacement = false
    var placementHidden = false {
        didSet {
            if placementHidden && !oldValue { hiddenBeforePlacement = hostHidden }
            if placementHidden { isHidden = true }
            else if oldValue { isHidden = hiddenBeforePlacement }
            setAccessibilityHidden(placementHidden || inert)
        }
    }
    /// An image node's picture, once loaded (decoded off the main thread),
    /// the source it came from, and which load is current: a completion
    /// from an older load, or for a view that was destroyed, is dropped.
    var symbolView: NSImageView? { didSet { layerPaintCache = nil } }
    var symbolFound = false
    var symbolKey: String?
    var symbolRefusal: String?
    var symbolClip: NSView?
    var image: NSImage?
    var raster: NativeRasterLease? { didSet { layerPaintCache = nil } }
    var imageSource: String?
    var loadGeneration = 0
    var pressed = false { didSet { if pressed != oldValue { pressChanged() } } }
    var press = PressFeedback() // LLP 1061: the feedback `pressed` drives
    // @ref LLP 1038 D6 — projection does not overwrite authored inert.
    var routeInert = false
    var inert: Bool {
        if presenter?.dialogs.blocks(self) == true { return true }
        var ancestor: NSView? = self
        while let view = ancestor {
            if let node = view as? NodeView, node.routeInert || node.props["inert"] == "true" { return true }
            if let node = view as? NodeView, presenter?.dialogs.owns(node) == true { break }
            ancestor = presenter?.menus.parent(of: view) ?? view.superview
        }
        return false
    }
    /// `aria-hidden` on this node or an ancestor: off the accessibility tree.
    var accessibilityHiddenByProp: Bool {
        sequence(first: self as NSView, next: { self.presenter?.menus.parent(of: $0) ?? $0.superview }).contains { ($0 as? NodeView)?.props["accessibilityElementsHidden"] == "true" }
    }
    var disabled: Bool { props["disabled"] == "true" }
    /// The pointer's tracking, for a `hover` handler (LLP 1005 §3).
    var tracking: NSTrackingArea?
    /// Images loaded since launch (smoke reporting).
    /// The session's text engine (LLP 1031 D12: the catalog is the session's).
    var text: TextEngine? { presenter?.session?.text }
    var canvases: Canvases? { presenter?.session?.canvases }

    /// A node with focus, blur, or key handlers takes the focus (an input's
    /// field does by itself): the web's rule that only a focusable element
    /// hears these. A pressable is in the tab order the way a `<button>` is.
    /// A paragraph takes the focus too, for selection, but plain text is
    /// never a Tab stop on the web. An explicit `tabindex` makes any box
    /// focusable, and a Tab stop only when ≥ 0 (LLP 1088 D7.3).
    override var acceptsFirstResponder: Bool {
        if disabled || inert || isHiddenOrHasHiddenAncestor { return false }
        if field != nil || textArea != nil { return false }
        return props["semanticTag"] == "dialog" || isParagraph || explicitTabIndex != nil || tabbable
    }
    /// A native button's command is its own too (a confirmation's close row, LLP 1069.011.000 D9).
    var pressable: Bool { handlers.contains("press") || (isButton && (props["commandfor"] != nil || props["popovertarget"] != nil)) }
    var tabbable: Bool {
        if let index = explicitTabIndex { return index >= 0 }
        return kind == "button" || isNativeButton || canvases?.wantsInput(id) == true || pressable || !handlers.isDisjoint(with: Self.focusEvents)
            || reorderKeys // a grouped grip takes the keys (LLP 1094 D9)
    }
    /// Sequential focus follows the web: a button is in the loop even when
    /// macOS "Keyboard navigation" is off (that setting would otherwise
    /// skip every non-field).
    override var canBecomeKeyView: Bool { acceptsFirstResponder && tabbable }
    override func becomeFirstResponder() -> Bool {
        guard !disabled else { return false }
        let ok = super.becomeFirstResponder()
        if ok { presenter?.collections.pinsChanged() }
        if ok, handlers.contains("focus") { presenter?.focus(id) }
        return ok
    }
    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { presenter?.selection.clear() }
        if ok { presenter?.collections.pinsChanged() }
        if ok && !isSurfaceControl { inputCanvas?.canvasInput?.blur() }
        if ok, handlers.contains("blur") { presenter?.blur(id) }
        return ok
    }
    override func drawFocusRingMask() {
        guard field == nil, pressable else { return }
        roundedPath(in: bounds).fill()
    }
    /// A key down's default action at a focused node; its `key` handlers
    /// heard it before AppKit delivered it (`Presenter.keyDown`, KeyEvents.swift).
    /// Space and Enter on a pressable fire `press`, as they do on a `<button>`.
    override func keyDown(with event: NSEvent) {
        guard !inert else { return }
        if inputCanvas?.canvasInput?.key(event, down: true, source: self) == true { return }
        guard !disabled else { return }
        if presenter?.menus.key(event) == true || presenter?.dialogs.key(event) == true { return }
        if isParagraph, window?.firstResponder === self, event.modifierFlags.contains(.command) {
            switch event.charactersIgnoringModifiers?.lowercased() {
            case "a": presenter?.selection.selectAll(); return
            case "c": presenter?.selection.copy(); return
            default: break
            }
        }
        let name = NodeView.keyName(event)
        // Sequential focus from every stop, as on the web: AppKit moves on from
        // a non-text view only with macOS "Keyboard navigation" on, while
        // Exact's loop holds buttons and native editors either way.
        if name == "Tab", event.modifierFlags.intersection([.command, .control, .option]).isEmpty, let window {
            presenter?.flushKeyViewLoop()
            if event.modifierFlags.contains(.shift) { window.selectPreviousKeyView(self) } else { window.selectNextKeyView(self) }
            return
        }
        if reorderKey(name) { return }
        if pressable, name == "Enter" || name == " " {
            let canvas = inputCanvas, ownerWindow = window
            presenter?.press(id)
            finishPress(canvas: canvas, window: ownerWindow, pointer: false)
            return
        }
        super.keyDown(with: event)
    }
    override func keyUp(with event: NSEvent) {
        guard !inert else { return }
        if inputCanvas?.canvasInput?.key(event, down: false, source: self) != true { super.keyUp(with: event) }
    }
    override func flagsChanged(with event: NSEvent) {
        guard !inert else { return }
        if canvasInput?.flags(event) != true { super.flagsChanged(with: event) }
    }
    /// ⌘A while this node's field is being edited. The Edit menu is the
    /// usual path; this catches it when that item is disabled (a secure
    /// field) or when the event arrives at the window rather than the app.
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if isParagraph, window?.firstResponder === self, event.modifierFlags.contains(.command) {
            switch event.charactersIgnoringModifiers?.lowercased() {
            case "a": presenter?.selection.selectAll(); return true
            case "c": presenter?.selection.copy(); return true
            default: break
            }
        }
        if let editor = textArea, NodeView.isCommandA(event), window?.firstResponder === editor {
            editor.selectAll(nil)
            return true
        }
        if let f = field, NodeView.isCommandA(event),
           window?.firstResponder === f || window?.firstResponder === f.currentEditor() {
            NodeView.selectAll(in: f)
            return true
        }
        return super.performKeyEquivalent(with: event)
    }
    /// Command-A with no other chord, ignoring Caps Lock / function noise.
    static func isCommandA(_ event: NSEvent) -> Bool {
        guard event.type == .keyDown else { return false }
        let mods = event.modifierFlags.intersection(.deviceIndependentFlagsMask).subtracting([.capsLock, .numericPad, .function])
        return mods == .command && event.charactersIgnoringModifiers?.lowercased() == "a"
    }
    /// Select the field's whole value. A secure editor can ignore `selectAll:`.
    static func selectAll(in f: NSTextField) {
        if let editor = f.currentEditor() {
            editor.selectAll(nil)
            if editor.selectedRange.length == 0 {
                let n = (editor.string as NSString).length
                if n > 0 { editor.selectedRange = NSRange(location: 0, length: n) }
            }
        } else {
            f.selectText(nil)
        }
    }
    /// The tracking area a `hover` handler needs (LLP 1005 §3), on the node
    /// or an inline run, kept while one wants it. `.inVisibleRect`: AppKit
    /// keeps its rect, so nothing here overrides `updateTrackingAreas` —
    /// AppKit called that on every node view, thousands of them, each time
    /// the scroll view moved, and posted a notification for each (25 ms/s
    /// of a fling's main thread on bones, 2026-09-30, against SwiftUI's 9).
    override func resetCursorRects() {
        super.resetCursorRects()
        if let cursor = CSSCursor.value(style["cursor"]?.string ?? "auto") { addCursorRect(bounds, cursor: cursor) }
    }
    func syncHoverTracking() {
        let wants = handlers.contains("hover") || handlers.contains("pointermove") || inlineText.contains(where: { $0.handlers.contains("hover") })
        if wants, tracking == nil {
            let t = NSTrackingArea(rect: .zero, options: [.mouseEnteredAndExited, .mouseMoved, .activeAlways, .inVisibleRect], owner: self, userInfo: nil)
            addTrackingArea(t)
            tracking = t
        } else if !wants, let t = tracking {
            removeTrackingArea(t)
            tracking = nil
        }
    }
    override func mouseEntered(with event: NSEvent) { mouseMoved(with: event) }
    override func mouseMoved(with event: NSEvent) {
        guard !inert else { return }
        pointerHovered(event)
        if canvasInput?.pointer(event, phase: "move") == true { return }
        let run = inlineTarget(at: local(event.locationInWindow), handler: "hover")
        presenter?.hoverInline(run?.id)
        if run == nil, handlers.contains("hover") { presenter?.hover(self, true) }
    }
    override func mouseExited(with event: NSEvent) {
        presenter?.hoverInline(nil)
        if handlers.contains("hover") { presenter?.hover(self, false) }
    }
    /// A control is a leaf, as UIKit makes one: VoiceOver reads its name.
    override func accessibilityAttributeNames() -> [NSAccessibility.Attribute] {
        super.accessibilityAttributeNames() + (["AXLanguage"] + Self.ariaAttributes.filter { ariaAttribute($0) != nil }).map { .init(rawValue: $0) }
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if attribute.rawValue == "AXLanguage" {
            let language = presenter?.documentLanguage ?? ""
            return language.isEmpty ? nil : language
        }
        return ariaAttribute(attribute.rawValue) ?? super.accessibilityAttributeValue(attribute)
    }
    /// `aria-hidden` takes the node and its subtree off the tree, as the
    /// web's does (onboarding F16: a checkbox's visible label stayed exposed).
    override func isAccessibilityElement() -> Bool {
        props["accessibilityElementsHidden"] != "true" && super.isAccessibilityElement()
    }
    override func accessibilityChildren() -> [Any]? {
        if props["accessibilityElementsHidden"] == "true" { return [] }
        return actsAsButton || props["accessibilityRole"] == "img" ? nil : textAccessibilityChildren() ?? super.accessibilityChildren()
    }
    /// What VoiceOver reaches, as the web's accessibility tree and iOS's
    /// traits have it: a pressable is a button — a link, checkbox, radio or
    /// switch when its role says so — a labelled image (or `role="img"`,
    /// an svg's) an image, and a `group` or `radiogroup` a group of its
    /// children. Headings are paragraphs (`updateTextAccessibility`); names
    /// come from `syncAccessibility`.
    func updateRoleAccessibility() {
        let role = props["accessibilityRole"]
        if actsAsButton {
            setAccessibilityElement(true)
            if let checked = checkedRole { setAccessibilityChecked(checked.role, checked.checked) }
            else { setAccessibilityToggle(pressedState, else: role == "link" ? .link : .button) }
            setAccessibilitySelected(props["accessibilitySelected"] == "true")
            if let expanded = props["accessibilityExpanded"] { setAccessibilityExpanded(expanded == "true") }
        } else if kind == "image" || role == "img" {
            let labelled = authoredLabel != nil
            setAccessibilityElement(labelled)
            setAccessibilityRole(labelled ? .image : nil)
        } else if role == "group" || role == "radiogroup" {
            setAccessibilityElement(true)
            setAccessibilityRole(role == "radiogroup" ? .radioGroup : .group)
        }
    }
    /// Where an image source resolves, as a page resolves `src`: an `http(s)`
    /// URL as is; a relative path under the asset root (`EXACT_ASSETS`, else
    /// the current directory) and never outside it; anything else (`file:`,
    /// `..` escaping the root) does not load.
    static func resolveSource(_ source: String, app: ExactApp?) -> URL? {
        if let u = URL(string: source), let scheme = u.scheme {
            return scheme == "http" || scheme == "https" ? u : nil
        }
        return app?.resolveAsset(source)
    }

    /// One bounded, session-owned pipeline. Replacement keeps the old raster
    /// and original intrinsic geometry until a matching new backing is accepted.
    func loadImage(_ source: String) {
        let previousSource = imageSource, previousGeneration = loadGeneration
        imageSource = source
        loadGeneration += 1
        if source.hasPrefix("symbol:") { presenter?.session?.rasters.cancel(id); raster = nil; updateSymbol(); return }
        clearSymbol(); image = nil
        if previousSource?.hasPrefix("symbol:") == true { presenter?.intrinsic(id, nil) }
        guard let session = presenter?.session else { return }
        if !session.rasters.load(self, source: source, resolver: session.app.resolver) {
            imageSource = previousSource; loadGeneration = previousGeneration
        }
    }

    /// The lease is this load's: shown, its natural size answered for the
    /// loader to report (`RasterLoader.reconcile`, one report per turn).
    func acceptRaster(_ lease: NativeRasterLease, generation: Int) -> CGSize? {
        guard loadGeneration == generation, let presenter, presenter.views[id] === self else { return nil }
        raster = lease; AnimatedRasters.shared.attach(self)
        self.needsDisplay = true
        if let c = canvasAbove { c.needsCapture = true; canvases?.scheduleCapture() }
        return lease.image.naturalSize
    }

    /// The view is gone: no load in flight may report for it.
    func forget() {
        presenter?.forgetParagraph(self)
        cancelSurfaceControls()
        invalidateText()
        cachedTextLayout = nil
        dropTextRaster()
        loadGeneration += 1
        presenter?.session?.rasters.cancel(id)
        raster = nil
        imageSource = nil
        clearSymbol()
        image = nil
        video?.invalidate(); video = nil
        destroyEmbedded()
        web = nil
        presenter = nil
    }

    init(id: UInt32, kind: String, presenter: Presenter) {
        self.id = id
        firstDraw = presenter.session?.drawReceipt() ?? {}
        self.kind = kind
        self.presenter = presenter
        super.init(frame: .zero)
        wantsLayer = kind != "text"
        // A frame change during live resize repaints at the new width
        // instead of stretching stale pixels.
        layerContentsRedrawPolicy = .duringViewResize
        if kind == "canvas" {
            let m = MetalView(frame: .zero)
            addSubview(m)
            metal = m
            let o = FlippedView(frame: .zero)
            o.autoresizingMask = [.width, .height]
            addSubview(o)
            overlay = o
        }
        if kind == "textarea" { makeTextArea() }
        if kind == "input" {
            let f = makeField(secure: false)
            addSubview(f)
            field = f
        }
        presenter.leaves.embed(self) // a video's player, an iframe's web view, a module's box (LLP 1068 §5.1)
    }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }

    /// Where children go: the scroll document view, or this view; a glass
    /// group innermost (`GlassGroup.swift`).
    var container: NSView { glassGroupContent ?? baseContainer }

    // @ref LLP 1001 §1 — two semantic materials, not sampled blur constants.
    // AppKit owns accessibility/appearance adaptation, including Reduce
    // Transparency and Increase Contrast; do not freeze the effective appearance.
    var appliedMaterial: String {
        guard let materialView, materialView.superview === self || (glassIsolation != nil && materialView.superview?.superview === glassIsolation) else {
            if (layer?.backgroundFilters?.count ?? 0) > 0 { return "backgroundFilters(CIGaussianBlur)" }
            return props["backgroundMaterial"] == nil ? "none" : "unsupported"
        }
        if #available(macOS 26.0, *), let glass = materialView as? NSGlassEffectView { return "NSGlassEffectView(.\(glass.style == .clear ? "clear" : "regular"))" }
        return "NSVisualEffectView(.\(materialKind.map { Materials.resolve($0) { _ in } } ?? "popover"))"
    }

    func updateMaterial() {
        let requested = props["backgroundMaterial"]
        defer { applyBackdrop(); syncGlassSlot(); syncGlassGroup() }
        let kind = requested
        if materialKind != kind {
            releaseGlassIsolation()
            let children = container.subviews.compactMap { $0 as? NodeView }
            let old = materialView
            materialView = nil
            materialContent = nil
            materialKind = kind
            if let kind {
                let content = MaterialContent(frame: bounds)
                content.autoresizingMask = [.width, .height]
                let effect: NSView
                let apple = Materials.resolve(kind) { [weak self] in self?.presenter?.session?.log($0) }
                if apple == "glass" || apple == "glassClear", #available(macOS 26.0, *) {
                    let glass = GlassBackground(frame: bounds)
                    glass.style = apple == "glass" ? .regular : .clear
                    glass.contentView = content
                    effect = glass
                } else {
                    // The table's material; AppKit's stand-in where it lacks
                    // the named one (ultra-thin draws popover), LLP 1053.000 D4.
                    let blur = BlurBackground(frame: bounds)
                    blur.material = Materials.material(apple) ?? .popover
                    blur.blendingMode = .withinWindow
                    blur.state = .followsWindowActiveState
                    blur.addSubview(content)
                    effect = blur
                }
                effect.autoresizingMask = [.width, .height]
                effect.setAccessibilityElement(false)
                content.setAccessibilityElement(false)
                addSubview(effect, positioned: .below, relativeTo: subviews.first)
                materialView = effect
                materialContent = content
            }
            // Reconcile before removing the old container, keeping live child
            // identities, their frames and their native editors intact.
            for child in children where child.superview !== container { container.addSubview(child) }
            old?.removeFromSuperview()
        }
        applyMaterialRadius()
    }

    /// The material's corners: the box's radius as CSS reduces it to fit the
    /// border box (a 100 pt radius on a 28 pt pill is 14), at the current size.
    func applyMaterialRadius() {
        guard let materialView else { return }
        materialView.wantsLayer = true
        guard let materialLayer = materialView.layer else { return }
        let radius = BorderPaint.clip(materialLayer, in: bounds, radii: cornerSizes(in: bounds))
        if #available(macOS 26.0, *), let glass = materialView as? NSGlassEffectView {
            glass.cornerRadius = radius
        } else {
            materialView.wantsLayer = true
            materialView.layer?.cornerRadius = radius
            materialView.layer?.masksToBounds = true
        }
    }

    /// The canvas this node is painted through, if any: the nearest canvas
    /// above whose overlay holds it.
    var canvasAbove: NodeView? {
        var v: NSView = self
        while let s = v.superview {
            if let c = s as? NodeView, c.overlay === v { return c }
            v = s
        }
        return nil
    }

    /// Something under a canvas repainted outside a batch (LLP 1014 D4 b, c):
    /// the canvas captures again on this run-loop turn — unless the draw is
    /// the capture's own, or follows a batch that already captured.
    func repaintThrough() {
        guard !Capture.capturing, let c = canvasAbove, !c.paintedThisTurn else { return }
        c.needsCapture = true
        canvases?.scheduleCapture()
    }

    @objc func clipScrolled() {
        // A collection's knob keeps the offset the reader saw (CollectionMac.swift).
        if let sv = scroll, let drag = KnobDrag.of(sv), !drag.admits(sv.contentView, correcting: presenter?.collections.correcting == true) { return }
        if let o = scroll?.contentView.bounds.origin { presenter?.onScrolled?(id, Double(o.x), Double(o.y)) }
        presenter?.stickies.scrolled(id)
        presenter?.collectionScrolled(id)
        presenter?.transformGeometry.changed()
        presenter?.videoVisibility?.changed()
        // The list window and the text bands follow the scroll; they are not
        // part of it (`Presenter.scrolled`).
        presenter?.scrolled()
        repaintThrough(); queueScrollEvent()
    }
    private var scrollEventQueued = false
    private var lastScrollEvent = CGPoint.zero
    private func queueScrollEvent() {
        guard handlers.contains("scroll"), !scrollEventQueued else { return }
        scrollEventQueued = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.scrollEventQueued = false
            guard let sv = self.scroll, case let point = sv.contentView.bounds.origin, point != self.lastScrollEvent,
                  self.presenter?.views[self.id] === self, self.hasScrollLayoutBox else { return }
            self.lastScrollEvent = point
            // CSS's extents (`ScrollEvent`): the port, and the port plus
            // the range the clip view clamps to, so at the end
            // `scrollHeight - scrollTop - clientHeight` is 0 (chat F4).
            let port = sv.contentView.bounds.size, document = sv.documentView?.frame.size ?? port
            let range = CGSize(width: max(0, document.width - port.width), height: max(0, document.height - port.height))
            self.presenter?.scroll(self.id, [point.x, point.y, port.width + range.width, port.height + range.height,
                                             port.width, port.height].map(Double.init))
        }
    }

    /// The direct child of a canvas this node is under, when that child is
    /// placed by the surface: the node whose `placement` maps this subtree.
    var placedAncestor: NodeView? {
        var v: NSView? = self
        while let n = v {
            if let node = n as? NodeView, (node.placement != nil || node.placementHidden) { return node }
            if let s = n.superview as? FlippedView, s.superview is NodeView, (s.superview as? NodeView)?.overlay === s { return nil }
            v = n.superview
        }
        return nil
    }

    /// A homography applied to a point (row major, projective).
    static func map(_ h: [Double], _ p: NSPoint) -> NSPoint {
        let w = h[6] * p.x + h[7] * p.y + h[8]
        guard abs(w) > 1e-9 else { return NSPoint(x: CGFloat.infinity, y: CGFloat.infinity) }
        return NSPoint(x: (h[0] * p.x + h[1] * p.y + h[2]) / w, y: (h[3] * p.x + h[4] * p.y + h[5]) / w)
    }

    /// The inverse of a 3×3 (row major), or nil when singular.
    static func invert(_ h: [Double]) -> [Double]? {
        let (a, b, c, d, e, f, g, hh, i) = (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8])
        let det = a * (e * i - f * hh) - b * (d * i - f * g) + c * (d * hh - e * g)
        guard abs(det) > 1e-12 else { return nil }
        let inv = [e * i - f * hh, c * hh - b * i, b * f - c * e,
                   f * g - d * i, a * i - c * g, c * d - a * f,
                   d * hh - e * g, b * g - a * hh, a * e - b * d]
        return inv.map { $0 / det }
    }

    /// A window point in this node's own coordinates: AppKit's conversion,
    /// through each transformed box on the way (LLP 1077 D8), and through the
    /// surface's placement when this node is under a placed child (LLP 1014
    /// D5) — the canvas reached first the same way, then the child's points.
    func local(_ windowPoint: NSPoint) -> NSPoint {
        guard let placed = placedAncestor, let h = placed.placement, let inv = NodeView.invert(h),
              let overlay = placed.superview, let canvas = overlay.superview as? NodeView else {
            return descend(windowPoint)
        }
        let inChild = NodeView.map(inv, canvas.local(windowPoint))
        return descend(inChild, from: placed)
    }

    /// The placement changed: accessibility sees the new box.
    func placementChanged() {
        NSAccessibility.post(element: self, notification: .layoutChanged)
    }

    /// Hit-testing through the surface's placements (LLP 1014 D5): a canvas
    /// whose children are placed maps the point through each child's
    /// inverse, topmost first — straight from the canvas to the child,
    /// skipping the box AppKit would test. A placed child is only where the
    /// surface put it, never at its kernel frame: the rest of the overlay
    /// (children the surface left in place) is tested in AppKit's order
    /// without them, and then the canvas itself is the hit.
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard !inert, !isHiddenOrHasHiddenAncestor, placedAncestor?.placementHidden != true, let point = spaceHit(point) else { return nil }
        if let clipPath, !clipPath.contains(convert(point, from: superview), using: clipRule) { return nil }
        if isSurfaceControl, bounds.contains(convert(point, from: superview)) { return self }
        func ordinary() -> NSView? {
            let found = raisedHit(super.hitTest(point), point) ?? overflowHit(point)
            let hit = found != nil && found === overlay ? self : found
            // CSS `pointer-events: none`: the box is never the target, nor
            // are its own platform views — a native module's (paint F9), a
            // field's — so the click goes to what is under it, as on iOS. A
            // descendant node that sets `auto` again still takes it.
            if let hit, style["pointer_events"]?.string == "none" {
                var owner: NSView? = hit
                while let v = owner, !(v is NodeView) { owner = v.superview }
                if owner === self { return nil }
            }
            return hit
        }
        guard let overlay, let sup = superview else { return ordinary() }
        let placed = overlay.subviews.compactMap { $0 as? NodeView }.filter { $0.placement != nil || $0.placementHidden }
        guard !placed.isEmpty else { return ordinary() }
        let inCanvas = convert(point, from: sup)
        guard !isHidden, bounds.contains(inCanvas) else { return nil }
        // Ordinary HUD paints above the captured children, so it hits first.
        let inOverlay = overlay.convert(inCanvas, from: self)
        for child in NodeView.hitOrder(overlay.subviews) where (child as? NodeView)?.placement == nil && (child as? NodeView)?.placementHidden != true {
            if let hit = child.hitTest(inOverlay) { return hit }
        }
        // Nearest first: what is seen on top is what a tap reaches.
        for child in placed.reversed().sorted(by: { ($0.placement?[9] ?? 0) > ($1.placement?[9] ?? 0) }) {
            guard let h = child.placement, let inv = NodeView.invert(h) else { continue }
            let p = NodeView.map(inv, inCanvas)
            guard child.bounds.contains(p) else { continue }
            // Into the child's superview's space, where AppKit expects it.
            let inOverlay = NSPoint(x: child.frame.minX + p.x, y: child.frame.minY + p.y)
            if let hit = child.hitTest(inOverlay) { return hit }
        }
        // Missed by every child: the canvas itself, unless it lets the
        // pointer through (`pointer-events: none`, as `ordinary` says).
        return style["pointer_events"]?.string == "none" ? nil : self
    }

    /// CSS visible overflow is hit where it paints, as on iOS: AppKit
    /// refuses a point outside a view's frame before it asks the subviews,
    /// so a positioned popup beyond its parent's box took no clicks (ledger
    /// F13, shop F18). Outside this box and unclipped, the children are
    /// asked here, topmost first. `point` is in the superview's space.
    private func overflowHit(_ point: NSPoint) -> NSView? {
        guard !isHidden, scroll == nil, clipBox == nil, !clipsToBounds, !subviews.isEmpty else { return nil }
        let local = convert(point, from: superview)
        let outsideX = local.x < bounds.minX || local.x > bounds.maxX
        let outsideY = local.y < bounds.minY || local.y > bounds.maxY
        guard outsideX || outsideY else { return nil }
        if outsideX && (style["overflow_x"]?.string ?? "visible") != "visible" { return nil }
        if outsideY && (style["overflow_y"]?.string ?? "visible") != "visible" { return nil }
        let order = subviews.enumerated().sorted {
            let (a, b) = ($0.element.layer?.zPosition ?? 0, $1.element.layer?.zPosition ?? 0)
            return a != b ? a > b : $0.offset > $1.offset
        }
        for case let child as NodeView in order.map(\.element) {
            if let hit = child.hitTest(local) { return hit }
        }
        return nil
    }

    /// The box on screen, through the placement of the placed child this
    /// node is (or is under), for assistive technology — the same box the
    /// agent's `layout` reports.
    override func accessibilityFrame() -> NSRect {
        if placedAncestor?.placementHidden == true { return .zero }
        guard placedAncestor?.placement != nil, let win = window else { return super.accessibilityFrame() }
        return win.convertToScreen(drawnRect(bounds))
    }

    /// Whether this view draws in the dark appearance. The **owning view's**
    /// appearance, not `NSAppearance.currentDrawing()`: `currentDrawing()`
    /// names whatever is drawing at that instant, and colours are not all
    /// applied inside a draw — a field's `textColor` is assigned in
    /// `applyStyle`. @ref LLP 1034 D2
    var drawsDark: Bool {
        effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
    }

    /// The four channels a colour row carries, resolved. A fixed colour is
    /// the four; a `light-dark()` pair is two fours and this picks one
    /// (LLP 1034 D1). Anything else is not a colour.
    func channels(_ key: String, dark: Bool? = nil) -> [Double]? {
        style[key]?.channels(dark: dark ?? drawsDark)
    }

    /// Whether any colour on this node is a pair — what says an appearance
    /// change is something to this view rather than nothing.
    var hasSchemeColor: Bool {
        style.values.contains { $0.isSchemeColor || $0.isSchemeGradient }
    }

    func color(_ key: String, _ fallback: NSColor) -> NSColor {
        guard let c = channels(key) else { return fallback }
        return NSColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
    }

    /// The appearance changed under this view. A repaint is not enough: the
    /// text engine caches a paragraph spec and a laid-out paragraph, and a
    /// `Run` carries a concrete colour, so ink from the previous appearance
    /// would survive a redisplay. Inline text nodes are not in the native
    /// hierarchy, so the paragraph that owns them is invalidated too, and
    /// the colours assigned outside a draw are re-applied. @ref LLP 1034 D2
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        if let regions = presenter?.session?.regions, regions.owns(self) { regions.geometryChanged() }
        // An `svg`'s paints are resolved into its scene's layers.
        presenter?.svg.reappear(id, dark: drawsDark, clock: presenter?.session?.clock)
        guard hasSchemeColor || inlineText.contains(where: { $0.hasSchemeColor }) else { return }
        paragraphOwner.invalidateText()
        paragraphOwner.needsDisplay = true
        applyStyle(style)
        needsDisplay = true
    }
    func number(_ key: String, _ fallback: CGFloat = 0) -> CGFloat {
        if let n = style[key]?.number { return CGFloat(n) }
        return fallback
    }

    /// The field for an input: `NSSecureTextField` for `type="password"`
    /// (the web's masking), a plain one otherwise; the same delegate,
    /// borderless, the node paints its own box.
    func makeField(secure: Bool) -> NSTextField {
        let f = secure ? SecureField(frame: .zero) : Field(frame: .zero)
        f.isBordered = false
        f.isBezeled = false
        f.drawsBackground = false
        f.backgroundColor = .clear
        (f.cell as? NSTextFieldCell)?.drawsBackground = false
        f.focusRingType = .none
        f.isEditable = true
        f.isSelectable = true
        f.delegate = self
        f.formatter = TextInputFormatter(self)
        f.cell?.isScrollable = true
        f.cell?.wraps = false
        f.cell?.usesSingleLineMode = true
        return f
    }

    /// The input's content box: padding and border sit on the node, the
    /// field is the text inside — CSS's rule, so a placeholder lines up
    /// with a native one.
    func contentBox() -> NSRect {
        let uniform = number("border_width")
        return bounds.insetBy(
            left: number("border_width_left", uniform) + number("padding_left"),
            top: number("border_width_top", uniform) + number("padding_top"),
            right: number("border_width_right", uniform) + number("padding_right"),
            bottom: number("border_width_bottom", uniform) + number("padding_bottom"))
    }

    func applyPlaceholder(_ f: NSTextField) {
        let text = props["placeholder"] ?? ""
        let font = f.font ?? NSFont.systemFont(ofSize: 17)
        if text.isEmpty {
            f.placeholderAttributedString = nil
            f.placeholderString = nil
            return
        }
        // Not `placeholderTextColor`: that tracks the window's appearance, so
        // a white field in a dark app (the night) paints a light placeholder
        // and it vanishes. Mute this field's text color — the web's
        // `input::placeholder` (`#3c3c434c` on black type).
        let ink = (f.textColor ?? NSColor(srgbRed: 0, green: 0, blue: 0, alpha: 1)).withAlphaComponent(0.30)
        f.placeholderAttributedString = NSAttributedString(string: text, attributes: [
            .font: font,
            .foregroundColor: ink,
        ])
    }

    // display:none removes the CSS box, but retains its stored scroll position.
    // UIKit/AppKit collapse the native extent; keep that transient reset out of
    // scroll events and restore only when the box returns.
    private var beforeLayoutScroll: CGPoint?
    private var hiddenScroll: CGPoint?
    private var hasScrollLayoutBox: Bool {
        var ancestor: NSView? = self
        while let current = ancestor {
            if let node = current as? NodeView, node.style["display"]?.string == "none" { return false }
            ancestor = current.superview
        }
        return true
    }

    var followedScroll: (top: CGFloat, end: Bool)?
    func captureScrollPosition() {
        beforeLayoutScroll = scroll?.contentView.bounds.origin
        followedScroll = nil
        guard props["scrollFollowEnd"] == "true", let sv = scroll, let doc = sv.documentView else { return }
        let maximum = max(0, doc.bounds.height - sv.contentView.bounds.height)
        followedScroll = (sv.contentView.bounds.minY, sv.contentView.bounds.minY >= maximum - 1)
    }
    func restoreScrollPosition() {
        defer { followedScroll = nil }
        guard props["scrollFollowEnd"] == "true", let sv = scroll, let doc = sv.documentView else { return }
        let maximum = max(0, doc.bounds.height - sv.contentView.bounds.height)
        let prior = followedScroll ?? (top: maximum, end: true)
        let y = prior.end ? maximum : min(maximum, max(0, prior.top))
        if sv.contentView.bounds.minY != y {
            sv.contentView.scroll(to: NSPoint(x: sv.contentView.bounds.minX, y: y))
            sv.reflectScrolledClipView(sv.contentView)
        }
    }
    func applyPendingScroll() {
        defer { pendingScrollTop = nil; pendingScrollLeft = nil }
        guard let sv = scroll, let doc = sv.documentView else { return }
        guard hasScrollLayoutBox else {
            if hiddenScroll == nil { hiddenScroll = beforeLayoutScroll ?? .zero }
            return
        }
        if let saved = hiddenScroll {
            hiddenScroll = nil
            let target = NSPoint(
                x: min(max(saved.x, 0), max(0, doc.bounds.width - sv.contentView.bounds.width)),
                y: min(max(saved.y, 0), max(0, doc.bounds.height - sv.contentView.bounds.height)))
            if sv.contentView.bounds.origin != target {
                sv.contentView.scroll(to: target)
                sv.reflectScrolledClipView(sv.contentView)
            }
        }
        guard pendingScrollTop != nil || pendingScrollLeft != nil else { return }
        let y = pendingScrollTop.map { CGFloat($0) == sv.contentView.bounds.minY ? sv.contentView.bounds.minY : min(max(CGFloat($0), 0), max(0, doc.bounds.height - sv.contentView.bounds.height)) } ?? sv.contentView.bounds.minY
        let x = pendingScrollLeft.map { CGFloat($0) == sv.contentView.bounds.minX ? sv.contentView.bounds.minX : min(max(CGFloat($0), 0), max(0, doc.bounds.width - sv.contentView.bounds.width)) } ?? sv.contentView.bounds.minX
        let target = NSPoint(x: x, y: y)
        guard sv.contentView.bounds.origin != target else { return }
        // `scroll-behavior: smooth` (CSS) animates a prop write, never a
        // reader's own scroll. Under the agent's frozen clock it lands at once,
        // so `layout` reads the target.
        if style["scroll_behavior"]?.string == "smooth" && !ExactEnv.agentFreezes {
            NSAnimationContext.runAnimationGroup({ _ in sv.contentView.animator().setBoundsOrigin(target) },
                                                 completionHandler: { sv.reflectScrolledClipView(sv.contentView) })
        } else {
            sv.contentView.scroll(to: target)
            sv.reflectScrolledClipView(sv.contentView)
        }
    }
    var pendingScrollLeft: Double?
    var pendingScrollTop: Double?
    func applyProps(set: [String: String], clear: [String]) {
        if clear.contains("action") { cancelSurfaceControls() }
        if clear.contains("scrollLeft") { pendingScrollLeft = nil }
        if let raw = set["scrollLeft"], let left = Double(raw), left.isFinite { pendingScrollLeft = left }
        if clear.contains("scrollTop") { pendingScrollTop = nil }
        if let raw = set["scrollTop"], let top = Double(raw), top.isFinite { pendingScrollTop = top }
        if pendingScrollTop != nil || pendingScrollLeft != nil { presenter?.pendingScrolls.insert(id) }
        var next = props
        for k in clear { next.removeValue(forKey: k) }
        for (k, v) in set { next[k] = v }
        props = next
        if set["symbolEffectValue"] != nil { updateSymbol() }
        applyTextArea()
        if let f = field {
            // `type` changed between password and text: a secure field is a
            // different class on AppKit, so the field is remade in place.
            let secure = props["type"] == "password"
            if (f is NSSecureTextField) != secure {
                let n = makeField(secure: secure)
                n.frame = f.frame
                n.stringValue = f.stringValue
                n.font = f.font
                n.textColor = f.textColor
                f.removeFromSuperview()
                addSubview(n)
                field = n
            }
        }
        if let f = field {
            if let v = props["value"] {
                // While the field is being edited its field editor holds the
                // caret; write there so the caret survives, as in a text view.
                if let editor = f.currentEditor() as? NSTextView { writeValue(v, into: editor) } else if f.stringValue != v { f.stringValue = v }
            }
            applyPlaceholder(f)
            f.isEnabled = !disabled
            f.isEditable = !disabled && props["editable"] != "false"
            (f.currentEditor() as? NSTextView)?.isAutomaticSpellingCorrectionEnabled = allowsInputCorrection
            (f.currentEditor() as? NSTextView)?.isContinuousSpellCheckingEnabled = allowsInputSpellChecking
        }
        // Each AppKit accessibility write posts a notification, changed or
        // not: write only what differs from the last write (a new view's
        // is AppKit's own: enabled, no identifier, no label).
        let ax = AccessibilityWrite(enabled: !disabled, identifier: props["testId"], label: props["accessibilityLabel"])
        if ax != lastAccessibilityWrite {
            let last = lastAccessibilityWrite
            if ax.enabled != last.enabled { setAccessibilityEnabled(ax.enabled) }
            if ax.identifier != last.identifier { setAccessibilityIdentifier(ax.identifier) }
            if ax.label != last.label { setAccessibilityLabel(ax.label) }
            lastAccessibilityWrite = ax
        }
        updateTextAccessibility()
        updateRoleAccessibility()
        if kind == "image", let src = props["imageSource"], src != imageSource { loadImage(src) }
        if kind == "image", props["imageSource"] == nil, imageSource != nil {
            loadGeneration += 1
            presenter?.session?.rasters.cancel(id)
            raster = nil
            imageSource = nil
            clearSymbol()
            image = nil
            presenter?.intrinsic(id, nil)
        }
        updateEmbedded()
        video?.update()
        updateMaterial()
        needsDisplay = true
    }

    // Empty container layers carry geometry and children, with no bitmap.
    private(set) var hasBoxPaint = false
    override var wantsUpdateLayer: Bool {
        if let readerParagraph, readerParagraph.hasPixels {
            return !hasBoxPaint && !Capture.capturing && canvasAbove == nil
        }
        if kind == "text" { return rastersText }
        // A box the layer can say keeps no backing store (`BoxLayerMac.swift`).
        return layerBoxEligible && !Capture.capturing && !drawsPaint
    }
    override func updateLayer() {
        // AppKit has rewritten the layer's geometry by now: the authored
        // transform goes back on (as after `layout()`).
        applyTransform()
        if let readerParagraph {
            readerParagraph.update(self)
            layer?.contents = nil
        } else if kind == "text" {
            // AppKit asks for its overdraw as well as for what is on screen.
            // Only what is on screen without pixels is painted here, rather
            // than shown blank; the rest is a worker's.
            if let presenter {
                presenter.textRasters.ensure(self, urgent: presenter.textIsVisible(self))
            }
            if textRaster != nil { presentTextRaster() } else { layer?.contents = nil }
        } else {
            layer?.contents = nil
            applyLayerPaint()
        }
        repaintThrough()
        if presenter?.views[id] === self { firstDraw() }
    }

    func applyStyle(_ s: NodeStyle) {
        defer { video?.update() }
        layerPaintCache = nil
        let origin = style["transform_origin"]
        let old = style
        style = s
        if old["display"] != s["display"] { isHidden = hostHidden }
        if old["cursor"] != s["cursor"] { window?.invalidateCursorRects(for: self) }
        if s["transform_origin"] != origin { applyTransform() }
        applySpace(changedFrom: old)
        let uniformBorder = number("border_width")
        hasBoxPaint = s["background_color"] != nil || s["background_image"] != nil
            || number("border_width_top", uniformBorder) > 0
            || number("border_width_right", uniformBorder) > 0
            || number("border_width_bottom", uniformBorder) > 0
            || number("border_width_left", uniformBorder) > 0
        layerContentsRedrawPolicy = kind != "text" && wantsUpdateLayer ? .onSetNeedsDisplay : .duringViewResize
        updateSymbol()
        (clipPath, clipRule) = (ClipPath.path(s["clip_path"]), ClipPath.rule(s["clip_path"]))
        // Inline text is unmounted run data. Its containing paragraph owns
        // the backing store; create this node's layer only when it mounts.
        if kind != "text" || superview != nil { wantsLayer = true }
        applyBoxMask()
        applyFilter()
        // Scrolling and clipping come from the effective overflow the host
        // wrote in (never from the node's kind): `scroll` on an axis makes a
        // scroll container that scrolls that axis; `hidden` clips.
        // A paragraph paints its own text and has no child views for a
        // scroll view to hold; one there (CSS computes `overflow-x: hidden`'s
        // other axis to `auto`, an ellipsis's usual pair) only took the
        // clicks a button around the label should hear (files diary F15).
        // Its overflow clips instead, as the box it paints in.
        let paragraph = kind == "text"
        let ox = paragraph ? "visible" : s["overflow_x"]?.string ?? "visible", oy = paragraph ? "visible" : s["overflow_y"]?.string ?? "visible"
        if ((ox == "scroll" || ox == "auto") || (oy == "scroll" || oy == "auto")) && scroll == nil {
            let sv = ChainingScrollView(frame: bounds)
            sv.collectionWillScroll = { [weak self] in
                guard let self else { return }
                // The reader's own scroll ends a smooth correction's animation.
                if presenter?.collections.animating.contains(id) == true, let clip = scroll?.contentView {
                    NSAnimationContext.runAnimationGroup({ c in c.duration = 0; clip.animator().setBoundsOrigin(clip.bounds.origin) })
                }
                presenter?.collections.animationEnded(id, dragging: true)
                presenter?.collections.userIntent(id, travel: true)
            }
            sv.drawsBackground = false
            sv.scrollerStyle = .overlay
            sv.hasVerticalScroller = true
            sv.hasHorizontalScroller = true
            sv.autohidesScrollers = true
            sv.automaticallyAdjustsContentInsets = false
            sv.contentInsets = NSEdgeInsetsZero
            sv.documentView = FlippedView(frame: .zero)
            // A scroll under a canvas repaints it (LLP 1014 D4 c).
            sv.contentView.postsBoundsChangedNotifications = true
            NotificationCenter.default.addObserver(self, selector: #selector(clipScrolled), name: NSView.boundsDidChangeNotification, object: sv.contentView)
            sv.autoresizingMask = [.width, .height]
            GlassGroups.moving(in: self) {
                for child in container.subviews where child is NodeView { child.removeFromSuperview(); sv.documentView?.addSubview(child) }
                addSubview(sv)
            }
            scroll = sv
            presenter?.scrollers.insert(id)
        }
        if ox != "scroll" && ox != "auto" && oy != "scroll" && oy != "auto", let sv = scroll {
            // Neither axis scrolls any more: the children come back out.
            GlassGroups.moving(in: self) {
                for child in sv.documentView?.subviews ?? [] where child is NodeView { child.removeFromSuperview(); (overlay ?? materialContent ?? self).addSubview(child) }
                sv.removeFromSuperview()
            }
            scroll = nil
            presenter?.scrollers.remove(id)
        }
        scroll?.scrollsX = (ox == "scroll" || ox == "auto")
        scroll?.scrollsY = (oy == "scroll" || oy == "auto")
        // `overflow: hidden` clips the children, to the box's rounded corners
        // as the web and UIKit do (LLP 1054 P2). One radius rides the layer;
        // differing radii clip to the bounds, as UIKit's layer path does.
        let clips = ox == "hidden" || oy == "hidden" || (paragraph && [s["overflow_x"]?.string, s["overflow_y"]?.string].contains { ($0 ?? "visible") != "visible" })
        // CSS's line-clamp implies `overflow: hidden`: a clamped paragraph's
        // one over-wide word must not paint over its neighbour (LLP 1054 P3).
        let clamped = kind == "text" && number("line_clamp") > 0
        // On a layer-backed view this is the layer's `masksToBounds`, unless
        // the node casts a shadow that clipping would clip (`BoxShadow.swift`).
        // A paragraph paints its own text, which a box would not clip.
        syncClipBox(clips && kind != "text" && shadowColor != nil && scroll == nil && overlay == nil && materialContent == nil)
        let clipped = (clips || clamped) && clipBox == nil
        if clipsToBounds != clipped { clipsToBounds = clipped }
        applyClipRadius()
        applyShadow()
        syncGlassGroup()
        // `overscroll-behavior` (CSS): `auto` chains, `contain` keeps the
        // gesture and bounces, `none` keeps it and does not.
        let bx = s["overscroll_behavior_x"]?.string ?? "auto"
        let by = s["overscroll_behavior_y"]?.string ?? "auto"
        scroll?.containX = bx != "auto"
        scroll?.containY = by != "auto"
        scroll?.bouncesX = bx == "contain"
        scroll?.bouncesY = by == "contain"
        // Set with the style and not per event: elasticity is what AppKit
        // reads to decide whether a gesture may stretch past the end, and
        // writing it while one is in flight disturbs the machine it enables.
        let ex: NSScrollView.Elasticity = bx == "contain" ? .allowed : bx == "none" ? .none : .automatic
        let ey: NSScrollView.Elasticity = by == "contain" ? .allowed : by == "none" ? .none : .automatic
        if let sv = scroll, sv.horizontalScrollElasticity != ex { sv.horizontalScrollElasticity = ex }
        if let sv = scroll, sv.verticalScrollElasticity != ey { sv.verticalScrollElasticity = ey }
        let scrollbarWidth = s["scrollbar_width"]?.string ?? "auto"
        scroll?.hasHorizontalScroller = (ox == "scroll" || ox == "auto") && scrollbarWidth != "none"
        scroll?.hasVerticalScroller = (oy == "scroll" || oy == "auto") && scrollbarWidth != "none"
        scroll?.horizontalScroller?.controlSize = scrollbarWidth == "thin" ? .small : .regular
        scroll?.verticalScroller?.controlSize = scrollbarWidth == "thin" ? .small : .regular
        styleTextArea()
        if let f = field, let t = text {
            (f.currentEditor() as? NSTextView)?.insertionPointColor = caretColor
            f.font = t.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"]?.string) == "italic", numeric: Int(number("font_variant_numeric")))
            f.textColor = color("text_color", .black)
            applyPlaceholder(f)
            f.frame = contentBox()
        }
        updateMaterial()
        needsDisplay = true
    }

    /// CSS `filter` (LLP 1055.000 D14): the box shows through a filtered
    /// picture (`BoxFilter`), drawn again after each batch.
    private(set) var boxFilter: BoxFilter?
    var hasBoxFilter: Bool { boxFilter != nil }
    func applyFilter() {
        // A node with no filter makes no BoxFilter to learn so (three layers
        // per styled node otherwise; iOS's 50e9abf6e).
        guard boxFilter != nil || style["filter"] != nil else { return }
        let f = boxFilter ?? BoxFilter()
        if f.set(style["filter"]) {
            boxFilter = f
            layer?.mask = f.hide
            presenter?.boxFilters.add(self) { [weak self] in self?.renderFilter() }
            renderFilter()
        } else if let f = boxFilter {
            f.remove()
            boxFilter = nil
            presenter?.boxFilters.remove(self)
            applyBoxMask()
        }
    }

    func renderFilter() {
        guard let f = boxFilter, let layer else { return }
        guard superview != nil else { f.remove(); return }
        // The picture renders the layer as it stands: a box that is layer
        // properties (`BoxLayerMac.swift`) gets them now, not at the next
        // display, so a new filtered box is not pictured empty.
        if layerBoxEligible, !Capture.capturing { applyLayerPaint() }
        setPaintPosition(paintZPosition)
        f.render(layer, clip: resolvedClipMask(), scale: window?.backingScaleFactor ?? 2)
    }

    override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        if superview == nil { boxFilter?.remove() } else if boxFilter != nil { renderFilter() }
        paintOrderMoved()
    }

    /// `overflow: hidden` clips to the rounded corners: one radius rides the
    /// clipping layer, reduced as CSS reduces it to fit the box (Core
    /// Animation draws nothing for a radius past half the shorter side);
    /// differing radii clip to the bounds, as UIKit's layer path does. The
    /// box's own layer paint decides the radius with it (`applyBoxLayer`).
    func applyClipRadius() { applyBoxLayer() }

    /// The reduction depends on the size, which the kernel's layout sets
    /// after the style.
    override func setFrameSize(_ newSize: NSSize) {
        let changed = newSize != frame.size
        super.setFrameSize(newSize)
        guard changed else { return }
        layerPaintCache = nil
        if hasBoxPaint || clipsToBounds || clipBox != nil { applyClipRadius() }
        if materialView != nil { applyMaterialRadius() }
        if number("backdrop_blur") > 0 { applyBackdrop() }
        // Border, gradient and image sublayers follow the new size.
        if layerBoxEligible && (hasBoxPaint || kind == "image") { needsDisplay = true }
    }

    func prepareToMount() {
        guard kind == "text" else { return }
        wantsLayer = true
        applyShadow()
        if let hide = boxFilter?.hide { layer?.mask = hide } else { applyBoxMask() }
        setPaintPosition(paintZPosition)
        applyTransform()
    }

    func fitScroll() {
        guard let document = scroll?.documentView else { return }
        let size = CGSize(width: max(content.width, bounds.width), height: max(content.height, bounds.height))
        if document.frame.size != size { document.setFrameSize(size) }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        presenter?.transformGeometry.changed()
        presenter?.videoVisibility?.changed()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        if textRaster != nil { textRasterKey = nil; textRasterPending = false; needsDisplay = true }
        // A border's device pixels follow the scale (`BoxLayerMac.swift`).
        if layerBoxEligible && hasBoxPaint { needsDisplay = true }
    }

    override func layout() {
        if let s = presenter?.session, s.firstLayoutMs == nil { s.firstLayoutMs = ExactEnv.wall() }
        super.layout()
        // AppKit rewrites a layer-backed view's layer geometry, transform
        // included, as it lays it out: the authored one goes back on.
        applyTransform()
        if style["perspective"] != nil { applyPerspective() }
        if kind == "image", flightLook == nil { presenter?.session?.rasters.resized(self) }
        presenter?.collections.changed(id)
        presenter?.transformGeometry.changed()
        presenter?.videoVisibility?.changed()
        if field != nil { field?.frame = contentBox() }
        video?.layout()
        if kind == "native" { presenter?.session?.natives.laidOut(self) }
        layoutTextArea()
        layoutSymbol()
    }

    /// The reduced radii; the layer fast path additionally requires circles.
    func cornerSizes(in rect: NSRect, inset: CGFloat = 0) -> [CGSize] {
        BorderPaint.reduced(BorderPaint.radii(style, in: rect, inset: inset), in: rect)
    }
    func cornerRadii(in rect: NSRect, inset: CGFloat = 0) -> [CGFloat] {
        cornerSizes(in: rect, inset: inset).map { $0.width }
    }
    func roundedPath(in rect: NSRect, inset: CGFloat = 0) -> NSBezierPath {
        NSBezierPath(cgPath: BorderPaint.roundedRect(rect, cornerSizes(in: rect, inset: inset), shape: CornerShape(style["corner_shape"])))
    }

    override func draw(_ rect: NSRect) {
        // The display path a drawn box takes instead of `updateLayer()`:
        // AppKit has rewritten the layer's transform here too.
        applyTransform()
        // Selection, capture, and decorated text return to direct painting.
        if textRasterUsesStrips {
            textRasterOverflowLayer?.removeFromSuperlayer()
            textRasterOverflowLayer = nil
        } else if textRasterOverflowLayer != nil {
            dropTextRaster()
            // cacheDisplay does not invalidate the live backing layer. Restore
            // its pixels on the pump after the offscreen capture has finished.
            if Capture.capturing { presenter?.requestTextPublication() }
        }
        repaintThrough()
        if Capture.capturing, kind == "canvas", let rep = canvases?.readback(view: self) {
            // A canvas nested under a canvas painted through its surface: its
            // picture into the ancestor's capture (LLP 1014); its own Metal
            // layer is not seen there.
            let picture = NSImage(size: bounds.size)
            picture.addRepresentation(rep)
            picture.draw(in: bounds, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
        }
        // The first pixel is on its way: the GPU module may load now (LLP
        // 1009 D4), on the next turn. A batch's own attempt runs before the
        // display pass and finds no first draw yet; an app with no later
        // batch — no image, no timer, no motion — would never load it
        // (found by the readback fixture, LLP 1014).
        if presenter?.views[id] === self { firstDraw() }
        if let ctx = NSGraphicsContext.current?.cgContext { drawCapturedShadow(ctx) }
        // What the layer shows (`BoxLayerMac.swift`) is not painted again,
        // nor into a capture where the capture shows the layer's paint as
        // the window does (`captureShowsLayerPaint`).
        let layerPaint = layerBoxEligible && (!Capture.capturing || captureShowsLayerPaint)
        if layerPaint, !Capture.capturing { applyLayerPaint() }
        let paintsBox = hasBoxPaint && (!layerPaint || boxNeedsDraw)
        let rounded = cornerRadii(in: bounds).contains { $0 > 0 }
        // The box's outline only where something is painted through it.
        lazy var path = roundedPath(in: bounds)
        // A layout transition's size shows the surface on its own layer.
        if paintsBox { drawBackground(path, rounded: rounded) }
        if let ctx = NSGraphicsContext.current?.cgContext { drawCapturedInsetShadow(ctx) }
        // The host sends each side's colour (`style.rs`), never a uniform
        // one: each side in its colour, joined as the web joins them.
        let uniform = number("border_width")
        if paintsBox, let ctx = NSGraphicsContext.current?.cgContext, surface == nil {
            let widths = ["top", "right", "bottom", "left"].map { number("border_width_" + $0, uniform) }
            let top = color("border_color_top", .clear)
            let colors = ["top", "right", "bottom", "left"].map { color("border_color_" + $0, top).cgColor }
            let radii = BorderPaint.radii(style, in: bounds)
            BorderPaint.paint(ctx, box: bounds, widths: widths, colors: colors, radii: radii, shape: CornerShape(style["corner_shape"]))
        }
        if kind == "image", symbolView == nil, flightLook == nil || imageLayer == nil, !(layerBoxEligible && !Capture.capturing && imageLayer != nil), let bitmap = raster?.image {
            // CSS object-fit over the content box (the frame inside border
            // and padding), clipped by the border box's radius: `fill`
            // stretches, `contain`/`cover` keep the ratio, `none` is the
            // natural size, `scale-down` the smaller of none and contain;
            // an unknown value is the initial `fill`.
            let fit = style["object_fit"]?.string ?? "fill"
            let content = bounds.insetBy(
                left: number("border_width_left", uniform) + number("padding_left"),
                top: number("border_width_top", uniform) + number("padding_top"),
                right: number("border_width_right", uniform) + number("padding_right"),
                bottom: number("border_width_bottom", uniform) + number("padding_bottom"))
            guard let ctx = NSGraphicsContext.current?.cgContext else { return }
            let rect = RasterGeometry.rect(natural: bitmap.naturalSize, content: content, fit: fit)
            ctx.saveGState()
            path.addClip()
            NSBezierPath(rect: content).addClip()
            ctx.translateBy(x: rect.minX, y: rect.maxY)
            ctx.scaleBy(x: 1, y: -1)
            RasterGeometry.draw(ctx, AnimatedRasters.shared.frame(for: self) ?? bitmap.image, in: CGRect(origin: .zero, size: rect.size), tint: channels("tint_color").map { TextEngine.color($0).cgColor })
            ctx.restoreGState()
        }
        let textDirty = Capture.capturing || canvasAbove != nil || textIsSmall ? rect : rect.intersection(presenter?.textVisibleRect(self) ?? visibleRect)
        if isParagraph, !textDirty.isEmpty, presenter?.session?.regions.owns(self) != true {
            // The same paragraph the kernel measured at this width, painted.
            let spec = paragraphSpec()
            if let readerParagraph, let ctx = NSGraphicsContext.current?.cgContext {
                if !readerParagraph.present(in: self) {
                    readerParagraph.draw(self, in: ctx, dirty: textDirty)
                }
            } else if let ctx = NSGraphicsContext.current?.cgContext, let paragraph = paragraphLayout() {
                presenter?.selection.draw(self, paragraph: paragraph, spec: spec, dirty: textDirty)
                paintBackgroundThroughText(ctx, paragraph: paragraph, spec: spec, in: contentBox())
                TextEngine.draw(paragraph, spec: spec, in: contentBox(), context: ctx, dirty: textDirty)
            }
        }
        if Capture.capturing, let picture = Capture.web[id] {
            // Remote WebKit layers supply their own picture for this capture
            // turn, at the node's normal hierarchy position (@ref LLP 1020 D4).
            picture.draw(in: bounds, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
        }
    }

    /// A click counts even when it is the one that activates the window —
    /// the web's rule (a click on an unfocused page still clicks). AppKit's
    /// default swallows it, which made a `tap` sent before the window became
    /// key vanish (found driving the app by hand over stdin).
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    // Press: down and up inside the bounds. A pressed node that does not
    // take the focus ends the editing, as a click on a button blurs a page's
    // input; a click nothing consumes reaches the viewport, which does the
    // same (a click on the page's ground).
    override func accessibilityPerformPress() -> Bool {
        if isSurfaceControl { return control("down") && control("up") }
        guard !disabled, !inert, pressable else { return false }
        presenter?.press(id); return true
    }
    override func mouseDown(with event: NSEvent) {
        guard !inert else { return }
        pointerPressed(event)
        if isSurfaceControl { _ = control("down", point: local(event.locationInWindow), timestamp: event.timestamp); return }
        if canvasInput?.pointer(event, phase: "down") == true { return }
        presenter?.leaves.pressed(self) // a held leaf's box was clicked: made now (LLP 1068 §5.1)
        presenter?.collections.pointerDown(id, event: event)
        presenter?.mouseChain.down(self, event: event)
        guard !disabled else { pressed = false; return }
        presenter?.interacting = id
        if let target = presenter?.svg.target(id, at: local(event.locationInWindow)) { svgPressed = target; return }
        if isParagraph, let run = inlineTarget(at: local(event.locationInWindow), handler: "press") {
            inlinePressed = run.id
            window?.makeFirstResponder(self)
            presenter?.selection.begin(self, event: event)
            return
        }
        if selectsText {
            window?.makeFirstResponder(self)
            presenter?.selection.begin(self, event: event)
            return
        }
        var focusNode: NSView? = self
        var retainFocus = false
        while let view = focusNode {
            if (view as? NodeView)?.props["retainFocus"] == "true" { retainFocus = true; break }
            focusNode = view.superview
        }
        // NSView forwards a click up the chain: a node under it the web would focus (not a paragraph selecting) took it first and keeps it (LLP 1088 D7.3).
        let inner = (window?.firstResponder as? NodeView).map { $0 !== self && ($0.tabbable || $0.explicitTabIndex != nil) && (window?.contentView?.hitTest(event.locationInWindow)?.isDescendant(of: $0) ?? false) } ?? false
        if acceptsFirstResponder, !retainFocus, !inner { window?.makeFirstResponder(self) }
        if pressable {
            if !acceptsFirstResponder, !retainFocus { window?.makeFirstResponder(nil) }
            pressed = true
        } else { super.mouseDown(with: event) }
    }
    /// A paragraph's drag selects its text unless a press takes it: its
    /// own handler (a `<span onClick>`, spreadsheet F12) or an ancestor's.
    var selectsText: Bool { isParagraph && !pressable && !hasPressableAncestor }
    var hasPressableAncestor: Bool {
        var next = superview
        while let view = next {
            if let node = view as? NodeView, (node.pressable || node.isSurfaceControl) { return true }
            next = view.superview
        }
        return false
    }
    override func mouseDragged(with event: NSEvent) {
        pointerDragged(event)
        guard !inert else { return }
        if isSurfaceControl || ownsSurfaceControl { _ = control("move", point: local(event.locationInWindow), timestamp: event.timestamp); return }
        if canvasInput?.pointer(event, phase: "move") == true { return }
        inlinePressed = nil
        // A gesture that engages ends the press (the chain clears `pressed`).
        if presenter?.mouseChain.drag(event) == true { return }
        pressFollows(inside: pressInside(event.locationInWindow))
        if selectsText { presenter?.selection.drag(event) }
        else { super.mouseDragged(with: event) }
    }
    override func rightMouseUp(with event: NSEvent) {
        pointerReleased(event)
        if canvasInput?.pointer(event, phase: "up") != true { super.rightMouseUp(with: event) }
    }
    override func mouseUp(with event: NSEvent) {
        pointerReleased(event)
        guard !inert else { return }
        if isSurfaceControl || ownsSurfaceControl { _ = control("up", point: local(event.locationInWindow), timestamp: event.timestamp); finishPointerPress(); return }
        if canvasInput?.pointer(event, phase: "up") == true { return }
        defer { presenter?.interacting = 0 }
        if presenter?.mouseChain.up(event) == true { return }
        presenter?.collections.releaseInteractionLater()
        let double = dblclickTarget(event)
        defer { dispatchDblclick(double) }
        if let target = svgPressed {
            svgPressed = nil
            // The element has no view of its own: the `svg`'s view stands for it.
            if !inert, presenter?.svg.target(id, at: local(event.locationInWindow)) == target {
                presenter?.pressHeld = KeyCodes.held(event.modifierFlags); presenter?.onPress?(target); presenter?.pressHeld = ""
            }
            return
        }
        if let run = inlinePressed {
            inlinePressed = nil
            if inlineTarget(at: local(event.locationInWindow), handler: "press")?.id == run { _ = activateInline(run) }
            return
        }
        if selectsText { presenter?.selection.end(self, event: event); return }
        guard !disabled else { pressed = false; return }
        guard pressed else { return super.mouseUp(with: event) }
        pressed = false
        if pressInside(event.locationInWindow) {
            let canvas = inputCanvas, ownerWindow = window
            presenter?.press(id, held: KeyCodes.held(event.modifierFlags))
            finishPress(canvas: canvas, window: ownerWindow, pointer: true)
        }
    }
    override func rightMouseDown(with event: NSEvent) {
        // DOM's order on a Mac: the secondary button's `pointerdown`, then
        // the `contextmenu` at its point, on the button's down (studio diary R22).
        pointerPressed(event)
        if canvasInput?.pointer(event, phase: "down") == true { return }
        guard !disabled, handlers.contains("contextmenu") else { return super.rightMouseDown(with: event) }
        presenter?.mouseEvent(id, 10, pointerSample(event).line)
    }
    override func rightMouseDragged(with event: NSEvent) {
        if canvasInput?.pointer(event, phase: "move") != true { super.rightMouseDragged(with: event) }
    }
    override func otherMouseDown(with event: NSEvent) {
        if canvasInput?.pointer(event, phase: "down") != true { super.otherMouseDown(with: event) }
    }
    override func otherMouseDragged(with event: NSEvent) {
        if canvasInput?.pointer(event, phase: "move") != true { super.otherMouseDragged(with: event) }
    }
    override func otherMouseUp(with event: NSEvent) {
        if canvasInput?.pointer(event, phase: "up") != true { super.otherMouseUp(with: event) }
    }
    override func scrollWheel(with event: NSEvent) {
        if canvasInput?.wheel(event) != true, presenter?.mouseTransformDrag.scroll(self, event: event) != true, !wheel(event) { super.scrollWheel(with: event) }
    }
    override func magnify(with event: NSEvent) {
        if presenter?.mouseTransformDrag.magnify(self, event: event) != true, !wheel(event) { super.magnify(with: event) }
    }
}
#endif
