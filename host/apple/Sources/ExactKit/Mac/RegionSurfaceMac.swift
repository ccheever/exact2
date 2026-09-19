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
/// A fixed-size image child uses NSView's flipped placement, not a resizing
/// contents layer on the viewport. Input always belongs to RegionInkView.
private final class RegionImageView: NSView {
    override var isFlipped: Bool { true }
    override var wantsUpdateLayer: Bool { true }
    override func updateLayer() {}
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}
final class RegionSurfaceMac: NSView {
    private weak var controller: RegionController?
    let scroller = ChainingScrollView(frame: .zero)
    private let document = FlippedView(frame: .zero)
    private let ink = RegionInkView(frame: .zero)
    private let picture = RegionImageView(frame: .zero)
    private var retained: RegionRetainedWitness?
    private var retention = RegionRetentionValidity()
    private var observer: NSObjectProtocol?
    private let profiles = NativeProfileAccount()
    private var profile: NativeProfile?
    private(set) var raster: RegionRaster?
    private var image: CGImage?
    private var presentation: RegionPresentation?
    private var selections: [UInt64: NSRange] = [:]
    private var contact: RegionContact?
    private var contactPhase: RegionRasterRequest?
    private var gestureSerial: UInt64 = 0
    private var pendingLink: (gesture: UInt64, phase: RegionRasterRequest, href: String)?
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
        ink.layer?.masksToBounds = true
        picture.wantsLayer = true
        ink.addSubview(picture)
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
        if !displayable && !showCurrent() && !showRetained() { clearVisible() }
        ink.needsDisplay = true
    }
    private var displayable: Bool {
        guard retention.valid, retained == nil,
              controller?.validateAppearance() == true, let visible, let presentation, let profile, let image,
              let raster, let current = currentRequest(for: presentation), raster.request.samePixels(as: current),
              picture.frame == CGRect(origin: .zero,size: raster.request.size),
              let shown = picture.layer?.contents as AnyObject?, shown === image else { return false }
        return visible.matches(publication: presentation.snapshot.publication, size: ink.bounds.size,
            scroll: scrollOffset, profile: profile.bytes, scale: Int(window?.backingScaleFactor ?? 0))
    }
    func publish(_ raster: RegionRaster, image: CGImage, presentation: RegionPresentation) {
        // A mutation invalidation may have left the kernel displaying old A
        // while B is pending. Such an A answer is not permission to rearm it.
        guard retention.valid || controller?.currentSourcePublication == raster.request.publication else { return }
        retention.acceptedPixels()
        let sourceChanged = self.presentation?.snapshot.publication != presentation.snapshot.publication
        self.raster = raster; self.image = image; self.presentation = presentation
        if sourceChanged { selections.removeAll(); cancelContact() }
        if let answer = raster.interaction { acceptInteraction(answer,phase: raster.request) }
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
        retention.invalidate()
        cancelContact()
        clearVisible()
        raster = nil; image = nil; presentation = nil
        selections.removeAll(); cancelContact()
        ink.needsDisplay = true
    }
    /// Called before protected source/style/tree changes and on sampled palette
    /// changes. Dropping the cache prevents requestRaster's exact-A cache hit
    /// from silently rearming it; pending worker ownership is not released here.
    func invalidateRetainedSource() {
        retention.invalidate()
        cancelContact(); clearVisible()
        raster = nil; image = nil
        ink.needsDisplay = true
    }
    private func clearVisible() {
        visible = nil; retained = nil
        // Hiding stale selection pixels does not cancel their ongoing owner.
        // Every other phase/identity change still retires that owner immediately.
        if !selectionAnchorIsCurrent { contact = nil; contactPhase = nil }
        if let pendingLink, !phaseIsCurrent(pendingLink.phase) { self.pendingLink = nil }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        picture.layer?.contents = nil
        CATransaction.commit()
        pendingLabel.isHidden = false
    }
    /// Publish a tagged CGImage directly. A display-list CGContext has no public
    /// bitmap format; cacheDisplay's separate representation cannot certify it.
    /// Model-layer publication is observable, physical presentation is not.
    fileprivate func updateInk() {
        guard let controller, controller.validateAppearance(),
              let candidate = controller.candidate, let current = currentRequest(for: candidate) else {
            clearVisible(); pendingLabel.stringValue = controller?.failure ?? "Preparing viewport…"; return
        }
        // Check A against the live context before issuing a new intent. A
        // palette/DPR ABA may not redisplay cached pixels merely by returning.
        invalidateChangedContext(current)
        setBackground(current)
        guard retention.valid || controller.currentSourcePublication == candidate.snapshot.publication else {
            clearVisible(); return
        }
        controller.requestRaster(size: current.size, scale: current.scale, profile: current.profile,
            format: current.format, background: current.background, selectionColor: current.selectionColor,
            scroll: current.scroll, selections: selections, interaction: current.interaction)
        if showCurrent() {
            finishPendingLink()
        } else if !showRetained() {
            clearVisible(); pendingLabel.stringValue = controller.failure ?? "Preparing viewport…"
        }
    }
    /// Delivery can replace the accepted raster before the next updateLayer.
    /// Assign qualified current pixels before clearing their predecessor's
    /// witness. Invalidation neither requests work nor dispatches pending links.
    @discardableResult private func showCurrent() -> Bool {
        guard retention.valid, let raster, let image, let presentation,
              let shown = currentRequest(for: presentation),
              raster.request.samePixels(as: shown), raster.request.scroll == scrollOffset,
              MainActor.assumeIsolated({ raster.accepts(image, size: shown.size, scale: shown.scale, profile: shown.profile) }) else { return false }
        let changed = visible?.publication != raster.request.publication || !displayable
        place(image,request: raster.request,frame: CGRect(origin: .zero,size: raster.request.size))
        retained = nil
        if changed { layerPublications += 1 }
        visible = RegionVisibleWitness(publication: raster.request.publication, size: raster.request.size,
            scroll: raster.request.scroll, profile: shown.profile.bytes, scale: raster.request.scale)
        pendingLabel.isHidden = true
        return true
    }
    private func invalidateChangedContext(_ current: RegionRasterRequest) {
        guard let old = raster?.request else { return }
        if old.generation != current.generation || old.scale != current.scale || old.profile != current.profile ||
            old.format != current.format || old.background != current.background || old.selectionColor != current.selectionColor {
            invalidateRetainedSource()
        }
    }
    /// No new raster, source adoption or input certificate. Reposition the same
    /// provider at its original size; uncovered strips are the original palette.
    @discardableResult private func showRetained() -> Bool {
        guard retention.valid, let presentation, let current = currentRequest(for: presentation) else { return false }
        invalidateChangedContext(current)
        guard retention.valid, let raster, let image,
              !raster.request.samePixels(as: current),
              let mapping = RegionRetainedWitness(accepted: raster.request,current: current,
                  actualScroll: scrollOffset,extent: presentation.extent,clip: ink.visibleRect),
              MainActor.assumeIsolated({ raster.accepts(image,size: raster.request.size,
                  scale: current.scale,profile: current.profile) }) else { return false }
        cancelContact()
        visible = nil; retained = mapping
        place(image,request: raster.request,frame: mapping.imageFrame)
        pendingLabel.isHidden = true
        return true
    }
    private func setBackground(_ request: RegionRasterRequest) {
        CATransaction.begin(); CATransaction.setDisableActions(true)
        ink.layer?.backgroundColor = CGColor(colorSpace: CGColorSpaceCreateDeviceRGB(),components: request.background)
        ink.layer?.isOpaque = true; ink.layer?.contentsScale = CGFloat(request.scale)
        CATransaction.commit()
    }
    private func place(_ image: CGImage, request: RegionRasterRequest, frame: CGRect) {
        setBackground(request)
        CATransaction.begin(); CATransaction.setDisableActions(true)
        picture.frame = frame
        picture.bounds = CGRect(origin: .zero,size: request.size)
        if let layer = picture.layer {
            layer.contentsScale = CGFloat(request.scale)
            layer.contentsGravity = .resize; layer.contentsRect = CGRect(x: 0,y: 0,width: 1,height: 1)
            layer.minificationFilter = .nearest; layer.magnificationFilter = .nearest
            layer.contents = image
        }
        CATransaction.commit()
    }
    /// Re-read native state on delivery, even between a bounds notification and
    /// updateLayer. No last-drawn witness is a certificate for a new publication.
    func accepts(_ request: RegionRasterRequest) -> Bool {
        guard let candidate = controller?.candidate, let current = currentRequest(for: candidate),
              retention.valid || controller?.currentSourcePublication == candidate.snapshot.publication else { return false }
        return request.sameOutput(as: current)
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
            format: CGImageAlphaInfo.premultipliedLast.rawValue, background: background, selectionColor: selectionColor, interaction: contact?.query)
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
    private var retainedDisplayable: Bool {
        guard retention.valid, let retained, let raster, let image, let presentation,
              let current = currentRequest(for: presentation),
              let mapping = RegionRetainedWitness(accepted: raster.request,current: current,
                  actualScroll: scrollOffset,extent: presentation.extent,clip: ink.visibleRect),
              mapping == retained, picture.frame == mapping.imageFrame,
              let shown = picture.layer?.contents as AnyObject?, shown === image else { return false }
        return true
    }
    var diagnostics: [String: Any] {
        ["certificate": certificate, "layerPublications": layerPublications, "displayable": displayable,
         "documentSize": [document.frame.width,document.frame.height],
         "clipSize": [scroller.contentView.bounds.width,scroller.contentView.bounds.height],
         "acceptedExtent": [presentation?.extent.width ?? 0,presentation?.extent.height ?? 0],
         "lastWheel": lastWheel,
         "retainedDisplayOnly": retainedDisplayable,
         "retainedPublication": String(retained?.publication ?? 0),
         "retainedCoverage": retained.map { [$0.coverage.minX,$0.coverage.minY,$0.coverage.width,$0.coverage.height] } ?? [],
         "retainedTranslation": retained.map { [$0.imageFrame.minX,$0.imageFrame.minY] } ?? [],
         "retainedCoversViewport": retained?.coversViewport ?? false,
         "visiblePublication": String(displayable ? visible?.publication ?? 0 : 0)]
    }
    private func target(_ event: NSEvent) -> (RegionArtifact, RegionFrame, CGPoint)? {
        guard displayable, let presentation else { return nil }
        let local = ink.convert(event.locationInWindow, from: nil)
        let point = CGPoint(x: local.x + scrollOffset.x, y: local.y + scrollOffset.y)
        let frames = presentation.snapshot.frames.filter { $0.artifact != 0 }
        guard point.x.isFinite, point.y.isFinite,
              let frame = frames.min(by: { $0.hitDistance(point) < $1.hitDistance(point) }),
              let artifact = presentation.artifacts[frame.artifact] else { return nil }
        return (artifact,frame,point)
    }
    private func phaseIsCurrent(_ phase: RegionRasterRequest) -> Bool {
        guard let presentation, let controller,
              controller.currentGeneration == phase.generation,
              controller.candidate?.snapshot.publication == phase.publication,
              presentation.snapshot.publication == phase.publication,
              let current = currentRequest(for: presentation) else { return false }
        return phase.sameInkAndGeometry(as: current)
    }
    private var selectionAnchorIsCurrent: Bool {
        guard let contact, let phase = contactPhase, let presentation,
              presentation.artifacts[contact.artifact]?.generation == phase.generation else { return false }
        return phaseIsCurrent(phase)
    }
    private func cancelContact() {
        contact = nil; contactPhase = nil; pendingLink = nil
    }
    private func acceptInteraction(_ reply: RegionPointReply, phase: RegionRasterRequest) {
        guard selectionAnchorIsCurrent, var contact, contact.accept(reply) else { return }
        if let selection = reply.selection { selections[reply.query.artifact] = selection }
        if reply.query.terminal {
            let nearest = presentation?.snapshot.frames.filter { $0.artifact != 0 }
                .min(by: { $0.hitDistance(reply.query.point) < $1.hitDistance(reply.query.point) })
            if !reply.query.dragged, !reply.query.selectAll, nearest?.artifact == reply.query.artifact,
               let href = reply.link { pendingLink = (contact.gesture,phase,href) }
            self.contact = nil; contactPhase = nil
        } else { self.contact = contact }
    }
    private func resolveCached(_ artifact: RegionArtifact, frame: RegionFrame, point: CGPoint) {
        guard let query = contact?.query, let phase = contactPhase, let raster,
              let index = artifact.metadata.cachedIndex(at: point,in: frame.box,artifact: artifact.id,hits: raster.hits) else { return }
        // A new down and a resolved anchor's next point are local. If down is
        // still unresolved, never replace it with the latest point's index.
        let anchor: Int
        if let known = query.anchor { anchor = known }
        else if let begin = query.begin,
                let first = artifact.metadata.cachedIndex(at: begin,in: frame.box,artifact: artifact.id,hits: raster.hits) { anchor = first }
        else { return }
        let selection: NSRange? = query.selectAll ? NSRange(location: 0,length: artifact.metadata.source.utf16Count)
            : query.dragged ? NSRange(location: min(anchor,index),length: abs(index-anchor)) : nil
        let reply = RegionPointReply(query: query,anchor: anchor,index: index,
            link: artifact.metadata.link(at: point,in: frame.box,exactIndex: index),selection: selection)
        acceptInteraction(reply,phase: phase)
    }
    fileprivate func select(_ event: NSEvent, begin: Bool) {
        if begin {
            cancelContact()
            guard let (artifact,frame,point) = target(event), let phase = raster?.request,
                  controller?.candidate?.snapshot.publication == phase.publication,
                  gestureSerial < UInt64.max else { return }
            gestureSerial += 1
            contact = RegionContact(gesture: gestureSerial,artifact: artifact.id,point: point,selectAll: event.clickCount >= 3)
            contactPhase = phase; dragged = false; selections.removeAll()
            resolveCached(artifact,frame: frame,point: point)
            invalidatePhase()
        } else { continueContact(event,terminal: false) }
    }
    private func continueContact(_ event: NSEvent, terminal: Bool) {
        guard selectionAnchorIsCurrent, var contact, let presentation,
              let artifact = presentation.artifacts[contact.artifact],
              let frame = presentation.snapshot.frames.first(where: { $0.artifact == contact.artifact }) else {
            cancelContact(); return
        }
        let local = ink.convert(event.locationInWindow,from: nil)
        let point = CGPoint(x: local.x + scrollOffset.x,y: local.y + scrollOffset.y)
        if !terminal { dragged = true }
        guard contact.update(point: point,dragged: dragged,terminal: terminal) else { return }
        self.contact = contact
        resolveCached(artifact,frame: frame,point: point)
        invalidatePhase()
        finishPendingLink()
    }
    fileprivate func followLink(_ event: NSEvent) { continueContact(event,terminal: true) }
    private func finishPendingLink() {
        guard let link = pendingLink else { return }
        guard link.gesture == gestureSerial, phaseIsCurrent(link.phase) else { pendingLink = nil; return }
        guard displayable, let owner = superview as? NodeView,
              let session = owner.presenter?.session, !session.isApplyingPresentation else { return }
        pendingLink = nil
        session.delegate?.exactSession(session,command: "openURL",args: [link.href])
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
