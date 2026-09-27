// `<photo-editor>`'s AppKit canvas: the photo drawn with its transform in
// `draw(_:)` (so the host's ordinary capture sees it exactly), and hand-
// written recognizers. A trackpad pinch zooms about the pinch location and a
// trackpad rotation turns about it; a mouse drag moves the photo against
// rubber-band bounds and coasts with its release velocity, or, started on a
// crop corner or edge, drags that; a double click resets. A mouse without a
// trackpad zooms with the scroll wheel at the pointer.
#if os(macOS)
import AppKit

final class PhotoCanvas: NSView, NSGestureRecognizerDelegate {
    var onEdit: ((PhotoEdit) -> Void)?
    var image: CGImage? { didSet { needsDisplay = true } }
    private(set) var edit = PhotoEdit() { didSet { needsDisplay = true; setAccessibilityValue(summary(edit)) } }
    private var grip: CropGrip = []
    private var cropStart = CGRect.zero, dragRaw = CGPoint.zero
    private var active = 0
    private var wheelSettle: Timer?

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        let magnify = NSMagnificationGestureRecognizer(target: self, action: #selector(magnified(_:)))
        let rotate = NSRotationGestureRecognizer(target: self, action: #selector(rotated(_:)))
        let pan = NSPanGestureRecognizer(target: self, action: #selector(panned(_:)))
        let double = NSClickGestureRecognizer(target: self, action: #selector(doubleClicked(_:)))
        double.numberOfClicksRequired = 2
        for g in [magnify, rotate, pan, double] as [NSGestureRecognizer] { g.delegate = self; addGestureRecognizer(g) }
        setAccessibilityElement(true)
        setAccessibilityRole(.image)
        setAccessibilityLabel("Photo")
        setAccessibilityHelp("Pinch on the trackpad to zoom, rotate with two fingers, drag to move, drag the crop corners or edges, double-click to reset. The Rotate 90° and Reset buttons below work without gestures.")
    }
    required init?(coder: NSCoder) { nil }

    override var isFlipped: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func setFrameSize(_ newSize: NSSize) { super.setFrameSize(newSize); needsDisplay = true }

    // MARK: geometry

    private var fit: CGSize {
        guard let image else { return stage.size }
        return edit.fit(CGSize(width: image.width, height: image.height), in: stage.size)
    }
    /// The area the photo fits and the crop spans: inset, so the handles at
    /// the crop's edges stay whole on screen.
    private var stage: CGRect { bounds.insetBy(dx: min(20, bounds.width / 4), dy: min(20, bounds.height / 4)) }
    private var centre: CGPoint { CGPoint(x: bounds.midX, y: bounds.midY) }
    private var cropRect: CGRect {
        let stage = self.stage
        return CGRect(x: stage.minX + stage.width * edit.crop.minX, y: stage.minY + stage.height * edit.crop.minY,
                      width: stage.width * edit.crop.width, height: stage.height * edit.crop.height)
    }
    private func fromCentre(_ p: CGPoint) -> CGPoint { CGPoint(x: p.x - centre.x, y: p.y - centre.y) }
    func summary(_ e: PhotoEdit) -> String { e.summary(stage: stage.size) }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.setFillColor(CGColor(srgbRed: 0.07, green: 0.075, blue: 0.094, alpha: 1))
        ctx.fill(bounds)
        if let image {
            let f = fit
            ctx.saveGState()
            ctx.clip(to: bounds)
            ctx.translateBy(x: centre.x + edit.offset.x, y: centre.y + edit.offset.y)
            ctx.rotate(by: edit.angle)
            ctx.scaleBy(x: edit.scale, y: edit.scale)
            // The view is flipped; an image draws upright under a second flip.
            ctx.scaleBy(x: 1, y: -1)
            ctx.interpolationQuality = .high
            ctx.draw(image, in: CGRect(x: -f.width / 2, y: -f.height / 2, width: f.width, height: f.height))
            ctx.restoreGState()
        }
        drawCrop(cropRect, in: bounds, context: ctx)
    }

    // MARK: motion

    private func settle(velocity: CGPoint = .zero) {
        guard active == 0 else { return }
        var target = edit
        target.scale = max(target.scale, 1)
        if target.scale != edit.scale { target.offset = CGPoint(x: edit.offset.x * target.scale / edit.scale, y: edit.offset.y * target.scale / edit.scale) }
        target.offset = target.resting(after: velocity, fit: fit)
        rest(target, duration: hypot(velocity.x, velocity.y) > 50 ? 0.6 : 0.3)
    }

    /// Where an edit comes to rest is known when the gesture ends: reported
    /// then, and eased to on screen. A new gesture lands it first.
    private let motion = EditMotion()
    private func rest(_ target: PhotoEdit, duration: Double = 0.35) {
        onEdit?(target)
        motion.run(from: edit, to: target, duration: duration) { [weak self] e in self?.edit = e }
    }
    private func land() { if let target = motion.land() { edit = target } }

    func setTurns(_ turns: Int, animated: Bool) {
        land()
        var target = edit
        target.turns = turns
        guard animated else { edit = target; return }
        rest(target)
    }

    func reset(turns: Int) { land(); rest(PhotoEdit(turns: turns)) }

    // MARK: gestures

    func gestureRecognizer(_ g: NSGestureRecognizer, shouldRecognizeSimultaneouslyWith other: NSGestureRecognizer) -> Bool {
        !(g is NSClickGestureRecognizer) && !(other is NSClickGestureRecognizer)
    }

    private func track(_ state: NSGestureRecognizer.State) -> Bool {
        switch state {
        case .began: active += 1; land(); return true
        case .changed: return true
        case .ended, .cancelled, .failed: active = max(0, active - 1); return false
        default: return false
        }
    }

    private var lastMagnification: CGFloat = 0, lastRotation: CGFloat = 0

    @objc private func magnified(_ g: NSMagnificationGestureRecognizer) {
        if g.state == .began { lastMagnification = 0 }
        guard track(g.state) else { return settle() }
        edit.zoom(by: (1 + g.magnification) / (1 + lastMagnification), about: fromCentre(g.location(in: self)))
        lastMagnification = g.magnification
    }

    @objc private func rotated(_ g: NSRotationGestureRecognizer) {
        if g.state == .began { lastRotation = 0 }
        guard track(g.state) else { return settle() }
        // AppKit's rotation is counterclockwise-positive; this view is flipped.
        edit.turn(by: -(g.rotation - lastRotation), about: fromCentre(g.location(in: self)))
        lastRotation = g.rotation
    }

    @objc private func panned(_ g: NSPanGestureRecognizer) {
        let p = g.location(in: self)
        if g.state == .began {
            land()
            grip = CropGrip.at(p, in: cropRect, reach: 12)
            cropStart = edit.crop
            dragRaw = edit.offset
        }
        if !grip.isEmpty {
            guard g.state == .changed || g.state == .began else { grip = []; onEdit?(edit); return }
            let t = g.translation(in: self)
            edit.crop = grip.moved(cropStart, by: CGPoint(x: t.x / max(stage.width, 1), y: t.y / max(stage.height, 1)))
            return
        }
        guard track(g.state) else {
            let v = g.velocity(in: self)
            return settle(velocity: CGPoint(x: v.x, y: v.y))
        }
        let t = g.translation(in: self)
        g.setTranslation(.zero, in: self)
        dragRaw = CGPoint(x: dragRaw.x + t.x, y: dragRaw.y + t.y)
        edit.offset = edit.banded(dragRaw, fit: fit)
    }

    @objc private func doubleClicked(_ g: NSClickGestureRecognizer) {
        reset(turns: edit.turns)
    }

    override func scrollWheel(with event: NSEvent) {
        // A trackpad's two-finger scroll is momentum the system already
        // shapes: it moves the photo. A mouse wheel zooms at the pointer.
        let p = fromCentre(convert(event.locationInWindow, from: nil))
        land()
        if event.hasPreciseScrollingDeltas {
            edit.offset = edit.banded(CGPoint(x: edit.offset.x + event.scrollingDeltaX, y: edit.offset.y + event.scrollingDeltaY), fit: fit)
        } else {
            edit.zoom(by: pow(1.0025, event.scrollingDeltaY * 10), about: p)
        }
        wheelSettle?.invalidate()
        wheelSettle = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: false) { [weak self] _ in self?.settle() }
    }
}
#endif
