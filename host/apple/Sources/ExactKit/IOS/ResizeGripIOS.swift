#if os(iOS)
import UIKit

/// A textarea's `resize` grip (`ResizeGrip.swift`): above its text view, a
/// pan from its corner follows the finger. It takes touches in its own
/// square alone, so a tap on the text beside it reaches the text view.
final class ResizeGrip: UIView {
    weak var owner: NodeView?
    var rtl = false { didSet { if rtl != oldValue { setNeedsDisplay() } } }
    private var start: (local: CGPoint, size: CGSize)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        backgroundColor = .clear
        contentMode = .redraw
        addGestureRecognizer(UIPanGestureRecognizer(target: self, action: #selector(pan(_:))))
        registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitAccessibilityContrast.self]) { (view: ResizeGrip, _: UITraitCollection) in
            view.setNeedsDisplay()
        }
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override func draw(_ rect: CGRect) {
        UIColor.secondaryLabel.setStroke()
        let path = UIBezierPath()
        path.lineWidth = 1
        for (from, to) in ResizeGripShape.strokes(rtl: rtl) {
            path.move(to: from)
            path.addLine(to: to)
        }
        path.stroke()
    }

    /// The size the last move asked for, sent at the display's next frame:
    /// each send is a commit and a layout, and a ProMotion screen's touches
    /// come at 120 Hz.
    private var pending: (width: CGFloat?, height: CGFloat?)?

    @objc private func pan(_ recognizer: UIPanGestureRecognizer) {
        guard let owner else { return }
        switch recognizer.state {
        case .began: start = (recognizer.location(in: owner), owner.laidOutFrame.size)
        case .changed:
            guard let start else { return }
            pending = owner.resizeGripSize(pointer: recognizer.location(in: owner), start: start.local, size: start.size,
                                           movedX: recognizer.translation(in: owner.superview).x)
            if pending != nil {
                FrameClock.shared.want(self, .resizeGrip, rate: FrameClock.full(on: window?.screen)) { [weak self] _ in self?.send() }
            }
        default:
            // The last move's size goes at once.
            send()
            start = nil
        }
    }

    private func send() {
        FrameClock.shared.drop(self)
        guard let owner, let size = pending else { return }
        pending = nil
        owner.resizeGripSend(size)
    }
}

extension NodeView {
    /// Shows the grip at the corner `resize` gives one, or takes it away.
    func layoutResizeGrip() {
        let existing = subviews.lazy.compactMap { $0 as? ResizeGrip }.first
        let axes = resizeAxes
        guard axes.width || axes.height else { existing?.removeFromSuperview(); return }
        let grip = existing ?? ResizeGrip(frame: .zero)
        if grip.superview == nil {
            grip.owner = self
            addSubview(grip)
        } else if subviews.last !== grip {
            bringSubviewToFront(grip)
        }
        grip.rtl = resizeGripRTL
        grip.frame = resizeGripFrame
    }
}
#endif
