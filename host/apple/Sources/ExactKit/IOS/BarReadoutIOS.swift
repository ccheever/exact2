// @ref LLP 1075.003 §9.15 — a header's readout (`role="status"`: a "● Clock"
// beside the title, which nothing presses) as a bar item, as a hand-built
// UIKit bar has one: `UIBarButtonItem(customView:)` holding labels and a dot,
// which on iOS 26 and later sits in the bar's glass like any item. Its parts
// are the author's as written (`ChipFace`, read with `readout`: fonts,
// colours, line heights, the dot, the gap, the padding); what the author left
// unsaid is UIKit's (the label colour, the body font, and beside words a
// title item's inset, `HeaderShape.Item.titleInset`). The item
// is kept across projections and its view follows the readout in place, so a
// changing readout does not rebuild the bar's items. VoiceOver reads it as
// static text, by its `aria-label` or its texts, and speaks a change
// politely, as ARIA's `status` asks.
#if os(iOS) || os(tvOS)
import UIKit

/// What a readout's view shows besides its face: its padding, direction,
/// words and how they are spoken.
struct ReadoutFacts: Equatable {
    var padding: UIEdgeInsets = .zero
    var rtl = false
    var label = ""
    /// Not `aria-live="off"`: a change is spoken.
    var live = true
    /// `aria-hidden` or `inert`, on it or around it: no accessibility element.
    var hidden = false
}

/// A readout's custom view: its face's parts in a row.
final class ReadoutView: UIView {
    let stack = UIStackView()
    private(set) var face: ChipFace?
    private(set) var spoken: String?
    private(set) var facts = ReadoutFacts()
    /// The readout, whose face is read again for new text size or Bold Text.
    weak var node: NodeView?
    // Physical, as CSS's `padding-left` is in either direction.
    private var edges: [NSLayoutConstraint] = []

    init() {
        super.init(frame: .zero)
        stack.axis = .horizontal
        stack.alignment = .center
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        edges = [stack.leftAnchor.constraint(equalTo: leftAnchor), rightAnchor.constraint(equalTo: stack.rightAnchor),
                 stack.topAnchor.constraint(equalTo: topAnchor), bottomAnchor.constraint(equalTo: stack.bottomAnchor)]
        NSLayoutConstraint.activate(edges)
        isAccessibilityElement = true
        accessibilityTraits = .staticText
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self, UITraitLegibilityWeight.self]) { (view: ReadoutView, _: UITraitCollection) in
            guard let node = view.node, let face = ChipFace(node, readout: true, traits: view.traitCollection) else { return }
            view.show(face, view.facts)
        }
    }
    required init?(coder: NSCoder) { nil }


    override var intrinsicContentSize: CGSize {
        let size = stack.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
        return CGSize(width: size.width + edges[0].constant + edges[1].constant, height: size.height + edges[2].constant + edges[3].constant)
    }

    /// Show `face` as `facts` say. A change of what it says is announced
    /// when it is live, spoken, on screen and not VoiceOver's focus, which
    /// reads its new label itself.
    func show(_ face: ChipFace, _ facts: ReadoutFacts) {
        let previous = self.facts
        self.facts = facts
        let padding = facts.padding
        let direction: UISemanticContentAttribute = facts.rtl ? .forceRightToLeft : .forceLeftToRight
        if semanticContentAttribute != direction { semanticContentAttribute = direction; stack.semanticContentAttribute = direction }
        if facts.hidden != previous.hidden || accessibilityElementsHidden != facts.hidden {
            isAccessibilityElement = !facts.hidden
            accessibilityElementsHidden = facts.hidden
        }
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
        let label = facts.label
        if label != spoken {
            let focused = UIAccessibility.focusedElement(using: .notificationVoiceOver) as AnyObject? === self
            if spoken != nil, facts.live, !facts.hidden, !focused, window != nil, !label.isEmpty {
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
            let image = UIImageView(image: FaceSymbol.image(name, points: points, configuration: UIImage.SymbolConfiguration(pointSize: points)))
            // Keep UIKit from dimming the status's symbol beside its words, as a status in the content keeps it (TintDimmingIOS.swift)
            image.tintColor = ink(tint); image.tintAdjustmentMode = .normal
            return image
        case .text(let text, let font, let color):
            let label = UILabel()
            label.text = text
            label.font = font
            // The platform's body (no font written) follows the text size.
            label.adjustsFontForContentSizeCategory = font.fontDescriptor.object(forKey: .textStyle) != nil
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
    /// Whether `node` is off the accessibility tree as authored: `inert`
    /// (its own, a logical ancestor's, or behind a modal presentation), or
    /// `aria-hidden` on it or a logical ancestor (a tabpanel UIKit holds its
    /// route apart from). The bar's own hiding of the header is `isHidden`,
    /// which this does not read.
    static func ariaHidden(_ node: NodeView) -> Bool {
        node.inert || node.presenter.map { $0.logicalAncestry(of: node).contains { ($0 as? NodeView)?.props["accessibilityElementsHidden"] == "true" } } == true
    }

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
            guard let face = i.readout, let item = c.readouts[i.id], let view = item.customView as? ReadoutView,
                  let node = presenter.views[i.id] else { continue }
            view.node = node
            // Off screen the view has no traits of its own yet: the node's face.
            let drawn = view.window == nil ? face : ChipFace(node, readout: true, traits: view.traitCollection) ?? face
            view.show(drawn, ReadoutFacts(padding: i.readoutPadding, rtl: (node.style["direction"]?.string ?? presenter.documentDirection) == "rtl",
                                          label: i.label ?? "", live: node.props["accessibilityLive"] != "off", hidden: Self.ariaHidden(node)))
            view.accessibilityIdentifier = i.testId
            item.accessibilityIdentifier = i.testId
        }
    }
}
#endif
