// @ref LLP 1075.003 §9.15 — a header's readout (`role="status"`: a "● Clock"
// beside the title, which nothing presses) as a bar item, as a hand-built
// UIKit bar has one: `UIBarButtonItem(customView:)` holding labels and a dot,
// which on iOS 26 and later sits in the bar's glass like any item. Its parts
// are the author's as written (`ChipFace`, read with `readout`: fonts,
// colours, line heights, the dot, the gap, the padding); what the author left
// unsaid is UIKit's (the label colour, the body font, no padding). The item
// is kept across projections and its view follows the readout in place, so a
// changing readout does not rebuild the bar's items. VoiceOver reads it as
// static text, by its `aria-label` or its texts, and speaks a change
// politely, as ARIA's `status` asks.
#if os(iOS) || os(tvOS)
import UIKit

/// A readout's custom view: its face's parts in a row.
final class ReadoutView: UIView {
    private let stack = UIStackView()
    private(set) var face: ChipFace?
    private(set) var spoken: String?
    private var edges: [NSLayoutConstraint] = []

    init() {
        super.init(frame: .zero)
        stack.axis = .horizontal
        stack.alignment = .center
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        edges = [stack.leadingAnchor.constraint(equalTo: leadingAnchor), trailingAnchor.constraint(equalTo: stack.trailingAnchor),
                 stack.topAnchor.constraint(equalTo: topAnchor), bottomAnchor.constraint(equalTo: stack.bottomAnchor)]
        NSLayoutConstraint.activate(edges)
        isAccessibilityElement = true
        accessibilityTraits = .staticText
    }
    required init?(coder: NSCoder) { nil }

    override var intrinsicContentSize: CGSize {
        let size = stack.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
        return CGSize(width: size.width + edges[0].constant + edges[1].constant, height: size.height + edges[2].constant + edges[3].constant)
    }

    /// Show `face` inside `padding`, spoken as `label`. A change of what it
    /// says is announced when `live` (the readout is not `aria-live="off"`)
    /// and it is on screen.
    func show(_ face: ChipFace, padding: UIEdgeInsets, label: String, live: Bool) {
        let insets = [padding.left, padding.right, padding.top, padding.bottom]
        if zip(edges, insets).contains(where: { $0.constant != $1 }) {
            for (edge, inset) in zip(edges, insets) { edge.constant = inset }
            invalidateIntrinsicContentSize()
            frame.size = intrinsicContentSize
        }
        if face != self.face {
            self.face = face
            for view in stack.arrangedSubviews { view.removeFromSuperview() }
            for ((part, alpha), line) in zip(zip(face.parts, face.opacities), face.lineHeights) {
                let view = Self.view(of: part)
                view.alpha = alpha
                if let line, view is UILabel { view.heightAnchor.constraint(equalToConstant: line).isActive = true }
                stack.addArrangedSubview(view)
            }
            stack.spacing = face.gap
            invalidateIntrinsicContentSize()
            frame.size = intrinsicContentSize
        }
        if label != spoken {
            if spoken != nil, live, window != nil, !label.isEmpty {
                UIAccessibility.post(notification: .announcement, argument: NSAttributedString(string: label,
                    attributes: [.accessibilitySpeechQueueAnnouncement: true]))
            }
            spoken = label
            accessibilityLabel = label
        }
    }

    /// A colour as written, light and dark; nil is UIKit's label colour.
    private static func ink(_ pair: [[Double]]?) -> UIColor {
        guard let pair else { return .label }
        return UIColor { TextEngine.color(pair[$0.userInterfaceStyle == .dark ? 1 : 0]) }
    }

    private static func view(of part: ChipFace.Part) -> UIView {
        switch part {
        case .dot(let size, let fill, let corners):
            let dot = ReadoutDot(color: ink(fill), corners: corners)
            dot.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([dot.widthAnchor.constraint(equalToConstant: size.width), dot.heightAnchor.constraint(equalToConstant: size.height)])
            return dot
        case .symbol(let name, let points, let tint):
            let image = UIImageView(image: UIImage(systemName: name, withConfiguration: UIImage.SymbolConfiguration(pointSize: points)))
            image.tintColor = ink(tint)
            return image
        case .text(let text, let font, let color):
            let label = UILabel()
            label.text = text
            label.font = font
            label.textColor = ink(color)
            return label
        }
    }
}

/// A readout's dot: a filled box at its authored size and corners.
private final class ReadoutDot: UIView {
    private let color: UIColor, corners: [CGSize]
    init(color: UIColor, corners: [CGSize]) {
        self.color = color
        self.corners = corners
        super.init(frame: .zero)
        isOpaque = false
        backgroundColor = .clear
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (dot: ReadoutDot, _: UITraitCollection) in dot.setNeedsDisplay() }
    }
    required init?(coder: NSCoder) { nil }
    override func draw(_ rect: CGRect) {
        color.setFill()
        UIBezierPath(cgPath: BorderPaint.roundedRect(bounds, corners, shape: nil)).fill()
    }
}

extension NavigationHost {
    /// The bar item of a readout: the route's own, kept across projections.
    func readoutItem(_ i: HeaderShape.Item, _ c: RouteController) -> UIBarButtonItem {
        if let item = c.readouts[i.id] { return item }
        let item = UIBarButtonItem(customView: ReadoutView())
        c.readouts[i.id] = item
        return item
    }

    /// Each readout's view shows its face as it is now; the items of
    /// readouts the bar no longer shows are dropped.
    func showReadouts(_ shape: HeaderShape?, in c: RouteController) {
        let shown = ((shape?.leading ?? []) + (shape?.trailing ?? [])).filter { $0.readout != nil }
        c.readouts = c.readouts.filter { id, _ in shown.contains { $0.id == id } }
        for i in shown {
            guard let face = i.readout, let item = c.readouts[i.id], let view = item.customView as? ReadoutView else { continue }
            view.show(face, padding: i.readoutPadding, label: i.label ?? "", live: presenter.views[i.id]?.props["accessibilityLive"] != "off")
            view.accessibilityIdentifier = i.testId
            item.accessibilityIdentifier = i.testId
        }
    }
}
#endif
