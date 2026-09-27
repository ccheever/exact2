// `<photo-editor>`'s UIKit canvas: hand-written gestures over a UIImageView.
// Pinch zooms about the pinch centroid, two fingers rotate about theirs, a
// drag moves the photo against rubber-band bounds and flings with its
// release velocity, the crop rectangle's corner and edge handles drag, and a
// double tap resets. Pinch, rotation and pan recognize together.
#if os(iOS)
import UIKit

private final class CropOverlay: UIView {
    var crop = CGRect.zero
    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        isUserInteractionEnabled = false
        contentMode = .redraw
    }
    required init?(coder: NSCoder) { nil }
    override func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        drawCrop(crop, in: bounds, context: ctx)
    }
}

/// A grip's touch target: 44 points, for fingers.
private final class CropHandle: UIView {
    let grip: CropGrip
    init(_ grip: CropGrip) {
        self.grip = grip
        super.init(frame: CGRect(x: 0, y: 0, width: 44, height: 44))
        backgroundColor = .clear
        isAccessibilityElement = false
    }
    required init?(coder: NSCoder) { nil }
}

final class PhotoCanvas: UIView, UIGestureRecognizerDelegate {
    var onEdit: ((PhotoEdit) -> Void)?
    var image: CGImage? {
        didSet { imageView.image = image.map { UIImage(cgImage: $0) }; setNeedsLayout() }
    }
    private(set) var edit = PhotoEdit()
    private let imageView = UIImageView()
    private let overlay = CropOverlay(frame: .zero)
    private var handles: [CropHandle] = []
    private var active = Set<UIGestureRecognizer>()
    private var lastScale: CGFloat = 1, lastRotation: CGFloat = 0, dragRaw = CGPoint.zero
    private var cropStart = CGRect.zero

    override init(frame: CGRect) {
        super.init(frame: frame)
        clipsToBounds = true
        backgroundColor = UIColor(red: 0.07, green: 0.075, blue: 0.094, alpha: 1)
        imageView.layer.minificationFilter = .trilinear
        addSubview(imageView)
        addSubview(overlay)
        for grip: CropGrip in [[.left, .top], [.right, .top], [.left, .bottom], [.right, .bottom], .left, .right, .top, .bottom] {
            let handle = CropHandle(grip)
            handle.addGestureRecognizer(UIPanGestureRecognizer(target: self, action: #selector(dragHandle(_:))))
            addSubview(handle)
            handles.append(handle)
        }
        let pinch = UIPinchGestureRecognizer(target: self, action: #selector(pinched(_:)))
        let rotate = UIRotationGestureRecognizer(target: self, action: #selector(rotated(_:)))
        let pan = UIPanGestureRecognizer(target: self, action: #selector(panned(_:)))
        pan.maximumNumberOfTouches = 2
        let double = UITapGestureRecognizer(target: self, action: #selector(doubleTapped(_:)))
        double.numberOfTapsRequired = 2
        for g in [pinch, rotate, pan, double] as [UIGestureRecognizer] { g.delegate = self; addGestureRecognizer(g) }
        isAccessibilityElement = true
        accessibilityTraits = .image
        accessibilityLabel = "Photo"
        accessibilityHint = "Pinch to zoom, turn two fingers to rotate, drag to move, drag the crop corners or edges, double-tap to reset. The Rotate 90° and Reset buttons below work without gestures."
    }
    required init?(coder: NSCoder) { nil }

    // MARK: geometry

    /// The photo fitted into the canvas, at scale 1.
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

    override func layoutSubviews() {
        super.layoutSubviews()
        apply()
    }

    private func apply() {
        let f = fit
        imageView.transform = .identity
        imageView.bounds = CGRect(origin: .zero, size: f)
        imageView.center = CGPoint(x: centre.x + edit.offset.x, y: centre.y + edit.offset.y)
        imageView.transform = CGAffineTransform(rotationAngle: edit.angle).scaledBy(x: edit.scale, y: edit.scale)
        overlay.frame = bounds
        overlay.crop = cropRect
        overlay.setNeedsDisplay()
        let r = cropRect
        for h in handles {
            let x = h.grip.contains(.left) ? r.minX : h.grip.contains(.right) ? r.maxX : r.midX
            let y = h.grip.contains(.top) ? r.minY : h.grip.contains(.bottom) ? r.maxY : r.midY
            h.center = CGPoint(x: x, y: y)
        }
        accessibilityValue = summary(edit)
    }

    /// After the last gesture lets go: the scale back into range and the
    /// photo back inside its bounds, then one report of where it rests.
    private func settle(velocity: CGPoint = .zero) {
        guard active.isEmpty else { return }
        var target = edit
        target.scale = max(target.scale, 1)
        if target.scale != edit.scale { target.offset = CGPoint(x: edit.offset.x * target.scale / edit.scale, y: edit.offset.y * target.scale / edit.scale) }
        target.offset = target.resting(after: velocity, fit: fit)
        rest(target, duration: hypot(velocity.x, velocity.y) > 50 ? 0.6 : 0.3)
    }

    /// Where an edit comes to rest is known when the gesture ends: reported
    /// then, and eased to on screen.
    private let motion = EditMotion()
    private func rest(_ target: PhotoEdit, duration: Double = 0.35) {
        onEdit?(target)
        motion.run(from: edit, to: target, duration: duration) { [weak self] e in self?.edit = e; self?.apply() }
    }
    private func land() { if let target = motion.land() { edit = target; apply() } }

    func setTurns(_ turns: Int, animated: Bool) {
        land()
        var target = edit
        target.turns = turns
        guard animated else { edit = target; apply(); return }
        rest(target)
    }

    func reset(turns: Int) { land(); rest(PhotoEdit(turns: turns)) }

    // MARK: gestures

    func gestureRecognizer(_ g: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        !(touch.view is CropHandle)
    }
    func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        !(g is UITapGestureRecognizer) && !(other is UITapGestureRecognizer) && other.view === self
    }

    private func track(_ g: UIGestureRecognizer) -> Bool {
        switch g.state {
        case .began: active.insert(g); land(); return true
        case .changed: return true
        default: active.remove(g); return false
        }
    }

    @objc private func pinched(_ g: UIPinchGestureRecognizer) {
        if g.state == .began { lastScale = 1 }
        guard track(g) else { return settle() }
        edit.zoom(by: g.scale / lastScale, about: fromCentre(g.location(in: self)))
        lastScale = g.scale
        dragRaw = edit.offset
        apply()
    }

    @objc private func rotated(_ g: UIRotationGestureRecognizer) {
        if g.state == .began { lastRotation = 0 }
        guard track(g) else { return settle() }
        edit.turn(by: g.rotation - lastRotation, about: fromCentre(g.location(in: self)))
        lastRotation = g.rotation
        dragRaw = edit.offset
        apply()
    }

    @objc private func panned(_ g: UIPanGestureRecognizer) {
        if g.state == .began { dragRaw = edit.offset }
        guard track(g) else { return settle(velocity: g.numberOfTouches <= 1 ? g.velocity(in: self) : .zero) }
        let t = g.translation(in: self)
        g.setTranslation(.zero, in: self)
        dragRaw = CGPoint(x: dragRaw.x + t.x, y: dragRaw.y + t.y)
        edit.offset = edit.banded(dragRaw, fit: fit)
        apply()
    }

    @objc private func doubleTapped(_ g: UITapGestureRecognizer) {
        reset(turns: edit.turns)
    }

    @objc private func dragHandle(_ g: UIPanGestureRecognizer) {
        guard let handle = g.view as? CropHandle, stage.width > 0, stage.height > 0 else { return }
        switch g.state {
        case .began: land(); cropStart = edit.crop
        case .changed:
            let t = g.translation(in: self)
            edit.crop = handle.grip.moved(cropStart, by: CGPoint(x: t.x / stage.width, y: t.y / stage.height))
            apply()
        default: onEdit?(edit)
        }
    }
}
#endif
