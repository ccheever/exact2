// A real stationary viewport beside an NSClipView. The accepted scroll extent,
// source/hits and pixels publish together; live candidate NodeViews never paint.
#if os(macOS)
import AppKit

private final class RegionInkView: NSView {
    weak var surface: RegionSurfaceMac?
    override var isFlipped: Bool { true }
    override var isOpaque: Bool { true }
    override var acceptsFirstResponder: Bool { true }
    override var wantsUpdateLayer: Bool { true }
    override func updateLayer() { surface?.updateInk() }
    override func scrollWheel(with event: NSEvent) { surface?.wheel(event) }
    override func mouseDown(with event: NSEvent) { window?.makeFirstResponder(self); surface?.select(event, begin: true) }
    override func mouseDragged(with event: NSEvent) { surface?.select(event, begin: false) }
    override func mouseUp(with event: NSEvent) { surface?.followLink(event) }
    @objc func copy(_ sender: Any?) { surface?.copySelection() }
    override func keyDown(with event: NSEvent) {
        if event.modifierFlags.contains(.command), event.charactersIgnoringModifiers == "c" { copy(nil) }
        else { super.keyDown(with: event) }
    }
}
final class RegionSurfaceMac: NSView {
    private weak var controller: RegionController?
    let scroller = ChainingScrollView(frame: .zero)
    private let document = FlippedView(frame: .zero)
    private let ink = RegionInkView(frame: .zero)
    private var observer: NSObjectProtocol?
    private let profiles = NativeProfileAccount()
    private var profile: NativeProfile?
    private(set) var raster: RegionRaster?
    private var image: CGImage?
    private var presentation: RegionPresentation?
    private var selections: [UInt64: NSRange] = [:]
    private var anchor: (artifact: UInt64, index: Int, phase: RegionRasterRequest)?
    private var dragged = false
    private var visible: RegionVisibleWitness?
    private let pendingLabel = NSTextField(labelWithString: "Preparing viewport…")
    private var certificate: [String: Any] = [:]
    private var layerPublications = 0
    private var lastWheel: [String: Any] = [:]
    override var isFlipped: Bool { true }
    var scrollOffset: CGPoint { scroller.contentView.bounds.origin }
    init(controller: RegionController) {
        self.controller = controller
        super.init(frame: .zero)
        wantsLayer = true
        scroller.documentView = document; scroller.drawsBackground = false
        scroller.hasVerticalScroller = true; scroller.hasHorizontalScroller = false
        scroller.scrollerStyle = .overlay; scroller.autohidesScrollers = true
        scroller.scrollsX = false; scroller.scrollsY = true; scroller.containY = true
        scroller.automaticallyAdjustsContentInsets = false; scroller.contentInsets = NSEdgeInsetsZero
        scroller.contentView.postsBoundsChangedNotifications = true
        addSubview(scroller)
        ink.surface = self; ink.wantsLayer = true
        pendingLabel.font = .systemFont(ofSize: 13); pendingLabel.isSelectable = false
        pendingLabel.frame = CGRect(x: 8,y: 8,width: 360,height: 24)
        ink.addSubview(pendingLabel)
        // Above the scrolling document, below overlay scrollbars.
        scroller.addSubview(ink, positioned: .above, relativeTo: scroller.contentView)
        observer = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification,
            object: scroller.contentView, queue: .main) { [weak self] _ in self?.invalidatePhase() }
    }
    required init?(coder: NSCoder) { fatalError("programmatic region surface") }
    deinit { if let observer { NotificationCenter.default.removeObserver(observer) } }
    override func layout() {
        super.layout()
        scroller.frame = bounds; scroller.tile()
        ink.frame = scroller.contentView.frame
        invalidatePhase()
    }
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        controller?.geometryChanged()
    }
    func invalidatePhase() {
        if !displayable { clearVisible() }
        ink.needsDisplay = true
    }
    private var displayable: Bool {
        guard controller?.validateAppearance() == true, let visible, let presentation, let profile, let image,
              let raster, let current = currentRequest(for: presentation), raster.request.samePixels(as: current),
              let shown = ink.layer?.contents as AnyObject?, shown === image else { return false }
        return visible.matches(publication: presentation.snapshot.publication, size: ink.bounds.size,
            scroll: scrollOffset, profile: profile.bytes, scale: Int(window?.backingScaleFactor ?? 0))
    }
    func publish(_ raster: RegionRaster, image: CGImage, presentation: RegionPresentation) {
        let sourceChanged = self.presentation?.snapshot.publication != presentation.snapshot.publication
        self.raster = raster; self.image = image; self.presentation = presentation
        if sourceChanged { selections.removeAll(); anchor = nil }
        // Only accepted pixels can change the extent used by actual scrolling.
        document.frame = CGRect(origin: .zero, size: CGSize(
            width: max(ink.bounds.width, presentation.extent.width),
            height: max(ink.bounds.height, presentation.extent.height)))
        scroller.contentView.scroll(to: raster.request.scroll)
        scroller.reflectScrolledClipView(scroller.contentView)
        ink.needsDisplay = true
    }
    private func backgroundAndEligibility() -> [CGFloat]? {
        var found: [CGFloat]?
        var view: NSView? = superview
        while let v = view {
            guard !v.isHidden, v.alphaValue == 1,
                  v.layer?.animationKeys()?.isEmpty != false else { return nil }
            if let n = v as? NodeView {
                guard n.translate == .zero, n.scale == 1, n.rotate == 0,
                      n.clipPath == nil, n.materialView == nil else { return nil }
                if found == nil, let channels = n.channels("background_color", dark: n.drawsDark), channels.count == 4 {
                    if channels[3] == 255 { found = channels.map { CGFloat($0) / 255 } }
                    else if channels[3] != 0 { return nil }
                }
            }
            view = v.superview
        }
        // The presenter paints the page background when no authored ancestor does.
        if let found { return found }
        guard let color = (superview as? NodeView)?.presenter?.pageBackground.usingColorSpace(.deviceRGB),
              color.alphaComponent == 1 else { return nil }
        return [color.redComponent, color.greenComponent, color.blueComponent, 1]
    }
    /// A terminal refusal clears both the display and every input owner now,
    /// rather than waiting for a display pass which may never be scheduled.
    func suspend() {
        anchor = nil
        clearVisible()
        raster = nil; image = nil; presentation = nil
        selections.removeAll(); anchor = nil
        ink.needsDisplay = true
    }
    private func clearVisible() {
        visible = nil
        // Hiding stale selection pixels does not cancel their ongoing owner.
        // Every other phase/identity change still retires that owner immediately.
        if !selectionAnchorIsCurrent { anchor = nil }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        ink.layer?.contents = nil
        CATransaction.commit()
        pendingLabel.isHidden = false
    }
    /// Publish a tagged CGImage directly. A display-list CGContext has no public
    /// bitmap format; cacheDisplay's separate representation cannot certify it.
    /// Model-layer publication is observable, physical presentation is not.
    fileprivate func updateInk() {
        guard let controller, controller.validateAppearance(), let layer = ink.layer,
              let candidate = controller.candidate, let current = currentRequest(for: candidate) else {
            clearVisible(); pendingLabel.stringValue = controller?.failure ?? "Preparing viewport…"; return
        }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        layer.backgroundColor = CGColor(colorSpace: CGColorSpaceCreateDeviceRGB(), components: current.background)
        layer.isOpaque = true; layer.contentsScale = CGFloat(current.scale)
        layer.contentsGravity = .resize; layer.contentsRect = CGRect(x: 0,y: 0,width: 1,height: 1)
        layer.minificationFilter = .nearest; layer.magnificationFilter = .nearest
        CATransaction.commit()
        controller.requestRaster(size: current.size, scale: current.scale, profile: current.profile,
            format: current.format, background: current.background, selectionColor: current.selectionColor,
            scroll: current.scroll, selections: selections)
        // Also match current selection/background when retaining a cached image.
        // An older publication may remain readable while a new width is pending,
        // but an image hidden by the current phase must never supply hits.
        if let raster, let image, let presentation, let shown = currentRequest(for: presentation),
           raster.request.samePixels(as: shown), raster.request.scroll == scrollOffset,
           MainActor.assumeIsolated({ raster.accepts(image, size: shown.size, scale: shown.scale, profile: shown.profile) }) {
            CATransaction.begin(); CATransaction.setDisableActions(true)
            layer.contents = image
            CATransaction.commit()
            if visible?.publication != raster.request.publication || !displayable { layerPublications += 1 }
            visible = RegionVisibleWitness(publication: raster.request.publication, size: raster.request.size,
                scroll: raster.request.scroll, profile: shown.profile.bytes, scale: raster.request.scale)
            pendingLabel.isHidden = true
        } else {
            clearVisible(); pendingLabel.stringValue = controller.failure ?? "Preparing viewport…"
        }
    }
    /// Re-read native state on delivery, even between a bounds notification and
    /// updateLayer. No last-drawn witness is a certificate for a new publication.
    func accepts(_ request: RegionRasterRequest) -> Bool {
        guard let candidate = controller?.candidate, let current = currentRequest(for: candidate) else { return false }
        return request.samePixels(as: current)
    }
    private func currentRequest(for presentation: RegionPresentation) -> RegionRasterRequest? {
        guard let controller, controller.validateAppearance(), let layer = ink.layer, let window else { return nil }
        guard let background = backgroundAndEligibility() else {
            controller.refuse("region opacity/ancestor presentation unsupported"); return nil
        }
        let scale = window.backingScaleFactor
        let backing = ink.convertToBacking(ink.bounds)
        let windowBacking = window.convertToBacking(ink.convert(ink.bounds, to: nil))
        let visibleBounds = ink.visibleRect
        let edges = [backing.minX,backing.minY,backing.maxX,backing.maxY,
                     windowBacking.minX,windowBacking.minY,windowBacking.maxX,windowBacking.maxY,
                     visibleBounds.minX * scale,visibleBounds.minY * scale,
                     visibleBounds.maxX * scale,visibleBounds.maxY * scale]
        guard scale == 1 || scale == 2,
              edges.allSatisfy({ $0.isFinite && $0.rounded() == $0 }),
              CATransform3DIsIdentity(layer.transform), CATransform3DIsIdentity(layer.sublayerTransform),
              layer.animationKeys()?.isEmpty != false else {
            controller.refuse("region actual layer geometry refused"); return nil
        }
        guard let profile = windowProfile() else { return nil }
        certificate = ["kind": "profiled CGImage/model-layer publication; not destination bitmap or physical presentation",
            "window": window.windowNumber, "scale": scale,
            "windowBounds": [windowBacking.minX,windowBacking.minY,windowBacking.width,windowBacking.height],
            "clip": [visibleBounds.minX,visibleBounds.minY,visibleBounds.width,visibleBounds.height],
            "profileBytes": profile.bytes.count]
        let color = NSColor.selectedTextBackgroundColor.withAlphaComponent(0.45).usingColorSpace(.deviceRGB) ?? .blue
        let selectionColor = [color.redComponent,color.greenComponent,color.blueComponent,color.alphaComponent]
        let desired = CGPoint(x: 0, y: min(max(0, scrollOffset.y), max(0, presentation.extent.height - ink.bounds.height)))
        var rows = presentation.rows
        for i in rows.indices { rows[i].selection = selections[rows[i].artifact] ?? NSRange(location: 0, length: 0) }
        return RegionRasterRequest(serial: 0, publication: presentation.snapshot.publication,
            generation: controller.currentGeneration,
            rows: rows, scroll: desired, size: ink.bounds.size, scale: Int(scale), profile: profile,
            format: CGImageAlphaInfo.premultipliedLast.rawValue, background: background, selectionColor: selectionColor)
    }
    /// Window ICC is the explicit worker/output profile. This says nothing
    /// about the window server's private backing format or unknown font AA.
    private func windowProfile() -> NativeProfile? {
        guard let space = window?.colorSpace?.cgColorSpace ?? window?.screen?.colorSpace?.cgColorSpace else {
            controller?.refuse("region window ICC unavailable"); return nil
        }
        if let profile, let expected = profile.makeSpace(), CFEqual(space, expected) { return profile }
        let captured = NativeProfile.capture(original: space, data: space.copyICCData(), account: profiles)
        guard let owner = captured.owner else {
            controller?.refuse("region window ICC refused: \(String(describing: captured.refusal))"); return nil
        }
        profile = owner; return owner
    }
    fileprivate func wheel(_ event: NSEvent) {
        scroller.scrollWheel(with: event)
        lastWheel = ["dx": event.scrollingDeltaX, "dy": event.scrollingDeltaY,
            "phase": event.phase.rawValue, "precise": event.hasPreciseScrollingDeltas]
    }
    var diagnostics: [String: Any] {
        ["certificate": certificate, "layerPublications": layerPublications, "displayable": displayable,
         "documentSize": [document.frame.width,document.frame.height],
         "clipSize": [scroller.contentView.bounds.width,scroller.contentView.bounds.height],
         "acceptedExtent": [presentation?.extent.width ?? 0,presentation?.extent.height ?? 0],
         "lastWheel": lastWheel,
         "visiblePublication": String(displayable ? visible?.publication ?? 0 : 0)]
    }
    private func hit(_ event: NSEvent) -> (artifact: RegionArtifact, index: Int, frame: RegionFrame, point: CGPoint)? {
        guard displayable, let presentation else { return nil }
        let local = ink.convert(event.locationInWindow, from: nil)
        let point = CGPoint(x: local.x + scrollOffset.x, y: local.y + scrollOffset.y)
        let frames = presentation.snapshot.frames.filter { $0.artifact != 0 }
        guard let frame = frames.min(by: { $0.hitDistance(point) < $1.hitDistance(point) }),
              let artifact = presentation.artifacts[frame.artifact] else { return nil }
        return (artifact, artifact.metadata.index(at: point, in: frame.box), frame, point)
    }
    private var selectionAnchorIsCurrent: Bool {
        guard let anchor, let presentation, let controller,
              controller.currentGeneration == anchor.phase.generation,
              controller.candidate?.snapshot.publication == anchor.phase.publication,
              presentation.snapshot.publication == anchor.phase.publication,
              presentation.artifacts[anchor.artifact]?.generation == anchor.phase.generation,
              let current = currentRequest(for: presentation) else { return false }
        return anchor.phase.sameInkAndGeometry(as: current)
    }
    fileprivate func select(_ event: NSEvent, begin: Bool) {
        if begin {
            anchor = nil
            guard let (artifact, index, _, _) = hit(event), let phase = raster?.request,
                  controller?.candidate?.snapshot.publication == phase.publication else { return }
            anchor = (artifact.id, index, phase); dragged = false; selections.removeAll()
            if event.clickCount >= 3 { selections[artifact.id] = NSRange(location: 0, length: artifact.metadata.source.utf16Count); dragged = true }
        } else {
            // This is continuation of one already admitted owner, not a fresh
            // hit on invisible pixels. Its exact accepted source/phase must live.
            guard let anchor, selectionAnchorIsCurrent, let presentation,
                  let artifact = presentation.artifacts[anchor.artifact],
                  let frame = presentation.snapshot.frames.first(where: { $0.artifact == anchor.artifact }) else {
                self.anchor = nil; return
            }
            let local = ink.convert(event.locationInWindow, from: nil)
            let point = CGPoint(x: local.x + scrollOffset.x, y: local.y + scrollOffset.y)
            let index = artifact.metadata.index(at: point, in: frame.box)
            dragged = true; selections[artifact.id] = NSRange(location: min(index, anchor.index), length: abs(index - anchor.index))
        }
        invalidatePhase()
    }
    fileprivate func followLink(_ event: NSEvent) {
        defer { anchor = nil }
        guard let anchor, selectionAnchorIsCurrent, !dragged, let hit = hit(event),
              hit.artifact.id == anchor.artifact,
              let href = hit.artifact.metadata.link(at: hit.point, in: hit.frame.box),
              let owner = superview as? NodeView, let session = owner.presenter?.session else { return }
        session.delegate?.exactSession(session, command: "openURL", args: [href])
    }
    fileprivate func copySelection() {
        guard displayable, let presentation else { return }
        let text = presentation.snapshot.frames.compactMap { frame -> String? in
            guard let selected = selections[frame.artifact], let artifact = presentation.artifacts[frame.artifact] else { return nil }
            return artifact.metadata.copy(selected)
        }.joined(separator: "\n")
        NSPasteboard.general.clearContents(); NSPasteboard.general.setString(text, forType: .string)
    }
}
#endif
