// Paragraph input/AX projection. The run table owns identity; native elements
// are created only when accessibility requests them, never when mounting text.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension Presenter {
    func hoverInline(_ id: UInt32?) {
        let next = id.flatMap { id in textHost(id).flatMap { !$0.inert && !$0.disabled ? id : nil } }
        guard next != hoveredInline else { return }
        if let old = hoveredInline, inlineText(old) != nil { onHover?(old, false) }
        hoveredInline = next
        if let next {
            if let old = hovered { hover(old, false) }
            onHover?(next, true)
        }
    }
}

extension NodeView {
    func updateInlineInteraction() {
        #if os(macOS)
        updateTrackingAreas()
        needsDisplay = true
        #else
        if inlineText.contains(where: { $0.handlers.contains("hover") }), hoverRecognizer == nil {
            let gesture = UIHoverGestureRecognizer(target: self, action: #selector(hovering(_:)))
            gesture.delaysTouchesBegan = false; gesture.delaysTouchesEnded = false; gesture.cancelsTouchesInView = false
            addGestureRecognizer(gesture); hoverRecognizer = gesture
        }
        setNeedsDisplay()
        #endif
    }
    func updateTextAccessibility() {
        guard isParagraph else { return }
        let label = props["accessibilityLabel"] ?? paragraphSpec().runs.map(\.text).joined()
        #if os(macOS)
        setAccessibilityElement(true)
        setAccessibilityRole(.staticText)
        setAccessibilityLabel(label)
        setAccessibilityValue(label)
        #else
        isAccessibilityElement = true
        accessibilityTraits.insert(.staticText)
        accessibilityLabel = label
        #endif
    }
    func inlineRects(_ run: InlineText) -> [CGRect] {
        guard let paragraph = paragraphLayout() else { return [] }
        return paragraph.selectionRects(run.range, align: paragraphSpec().align, in: contentBox(), dirty: bounds)
    }
    func textAccessibilityChildren() -> [Any]? {
        let interactive = inlineText.filter { $0.props["href"] != nil || !$0.handlers.isEmpty || $0.props["accessibilityLabel"] != nil }
        guard !interactive.isEmpty else { return nil }
        let text = paragraphSpec().runs.map(\.text).joined() as NSString
        return interactive.map { InlineAccessibility(owner: self, run: $0, text: text) }
    }
}

#if os(macOS)
private final class InlineAccessibility: NSAccessibilityElement {
    weak var owner: NodeView?
    let id: UInt32
    init(owner: NodeView, run: InlineText, text: NSString) {
        self.owner = owner; id = run.id
        super.init()
        setAccessibilityParent(owner)
        setAccessibilityRole(run.props["href"] == nil ? .staticText : .link)
        setAccessibilityIdentifier(run.props["testId"])
        setAccessibilityLabel(run.props["accessibilityLabel"] ?? text.substring(with: run.range))
        if let href = run.props["href"] { setAccessibilityURL(URL(string: href)) }
    }
    override func accessibilityFrame() -> NSRect {
        guard let owner, let run = owner.presenter?.inlineText(id), let window = owner.window else { return .zero }
        let rect = owner.inlineRects(run).reduce(CGRect.null) { $0.union($1) }
        return rect.isNull ? .zero : window.convertToScreen(owner.convert(rect, to: nil))
    }
    override func accessibilityPerformPress() -> Bool { owner?.activateInline(id) ?? false }
}
#else
private final class InlineAccessibility: UIAccessibilityElement {
    weak var owner: NodeView?
    let id: UInt32
    init(owner: NodeView, run: InlineText, text: NSString) {
        self.owner = owner; id = run.id
        super.init(accessibilityContainer: owner)
        accessibilityIdentifier = run.props["testId"]
        accessibilityLabel = run.props["accessibilityLabel"] ?? text.substring(with: run.range)
        accessibilityTraits = run.props["href"] == nil ? .staticText : .link
    }
    override var accessibilityFrame: CGRect {
        get {
            guard let owner, let run = owner.presenter?.inlineText(id) else { return .zero }
            let rect = owner.inlineRects(run).reduce(CGRect.null) { $0.union($1) }
            return rect.isNull ? .zero : UIAccessibility.convertToScreenCoordinates(rect, in: owner)
        }
        set { }
    }
    override func accessibilityActivate() -> Bool { owner?.activateInline(id) ?? false }
}
#endif
