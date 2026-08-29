// The presenter: one NSView per kernel node, in a flipped root, applying the
// host's batches. Backgrounds, borders, and text are drawn; frames come from
// the kernel's layout; transforms and opacity from presentation values.
import AppKit

final class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

/// Milliseconds from script start to the first node's first draw.
nonisolated(unsafe) var firstDrawMs: Double? = nil
/// Milliseconds from script start to the first node's first layout pass.
nonisolated(unsafe) var firstLayoutMs: Double? = nil

final class NodeView: NSView, NSTextFieldDelegate {
    let id: UInt32
    let kind: String
    var props: [String: String] = [:]
    var style: [String: Any] = [:]
    var handlers: Set<String> = []
    var translate = CGPoint.zero
    var scale: CGFloat = 1
    var rotate: CGFloat = 0
    weak var presenter: Presenter?
    var field: NSTextField?
    var scroll: NSScrollView?
    var pressed = false

    init(id: UInt32, kind: String, presenter: Presenter) {
        self.id = id
        self.kind = kind
        self.presenter = presenter
        super.init(frame: .zero)
        wantsLayer = true
        // A frame change during live resize repaints at the new width
        // instead of stretching stale pixels.
        layerContentsRedrawPolicy = .duringViewResize
        if kind == "scroll" || kind == "list" {
            let sv = NSScrollView(frame: .zero)
            sv.drawsBackground = false
            sv.hasVerticalScroller = true
            sv.autohidesScrollers = true
            sv.documentView = FlippedView(frame: .zero)
            sv.autoresizingMask = [.width, .height]
            addSubview(sv)
            scroll = sv
        }
        if kind == "input" {
            let f = NSTextField(frame: .zero)
            f.isBordered = false
            f.drawsBackground = false
            f.focusRingType = .none
            f.delegate = self
            f.autoresizingMask = [.width, .height]
            addSubview(f)
            field = f
        }
    }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }

    /// Where children go: the scroll document view, or this view.
    var container: NSView { scroll?.documentView ?? self }

    func color(_ key: String, _ fallback: NSColor) -> NSColor {
        guard let c = style[key] as? [Double], c.count == 4 else { return fallback }
        return NSColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
    }
    func number(_ key: String, _ fallback: CGFloat = 0) -> CGFloat {
        if let n = style[key] as? Double { return CGFloat(n) }
        return fallback
    }

    func applyProps(set: [String: String], clear: [String]) {
        for k in clear { props.removeValue(forKey: k) }
        for (k, v) in set { props[k] = v }
        if let f = field {
            if let v = props["value"], f.stringValue != v { f.stringValue = v }
            f.placeholderString = props["placeholder"]
        }
        setAccessibilityIdentifier(props["testId"])
        setAccessibilityLabel(props["accessibilityLabel"])
        needsDisplay = true
    }

    func applyStyle(_ s: [String: Any]) {
        style = s
        if let f = field {
            f.font = Text.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), italic: false)
            f.textColor = color("text_color", .black)
        }
        needsDisplay = true
    }

    func applyTransform() {
        let b = bounds
        var t = CGAffineTransform(translationX: translate.x, y: translate.y)
        t = t.translatedBy(x: b.midX, y: b.midY).rotated(by: rotate * .pi / 180).scaledBy(x: scale, y: scale).translatedBy(x: -b.midX, y: -b.midY)
        layer?.setAffineTransform(t)
    }

    override func layout() {
        if firstLayoutMs == nil { firstLayoutMs = now() }
        super.layout()
    }

    override func draw(_ rect: NSRect) {
        if firstDrawMs == nil { firstDrawMs = now() }
        let radius = number("border_radius", number("border_radius_top_left"))
        let path = NSBezierPath(roundedRect: bounds, xRadius: radius, yRadius: radius)
        let bg = color("background_color", .clear)
        if bg.alphaComponent > 0 {
            bg.setFill()
            path.fill()
        }
        let borderColor = color("border_color", .clear)
        let uniform = number("border_width")
        let sides: [(String, NSRect)] = [
            ("border_width_top", NSRect(x: 0, y: 0, width: bounds.width, height: number("border_width_top", uniform))),
            ("border_width_bottom", NSRect(x: 0, y: bounds.height - number("border_width_bottom", uniform), width: bounds.width, height: number("border_width_bottom", uniform))),
            ("border_width_left", NSRect(x: 0, y: 0, width: number("border_width_left", uniform), height: bounds.height)),
            ("border_width_right", NSRect(x: bounds.width - number("border_width_right", uniform), y: 0, width: number("border_width_right", uniform), height: bounds.height)),
        ]
        for (key, r) in sides where number(key, uniform) > 0 {
            color(key.replacingOccurrences(of: "width", with: "color"), borderColor).setFill()
            r.fill()
        }
        if kind == "text", let text = props["text"] {
            // The same paragraph the kernel measured at this width, painted.
            let spec = textSpec(text)
            Text.draw(Text.paragraph(spec, width: bounds.width), spec: spec, in: bounds)
        }
    }

    /// The paragraph spec from this node's rows, with CSS's defaults for the
    /// rows it does not set (the kernel's defaults are CSS's).
    func textSpec(_ text: String) -> Spec {
        let align: Int
        switch style["text_align"] as? String { case "center": align = 1; case "right": align = 2; case "justify": align = 3; default: align = 0 }
        let c = (style["text_color"] as? [Double]) ?? [0, 0, 0, 255]
        return Spec(
            runs: [Run(text: text, size: number("font_size", 16), weight: Int(number("font_weight", 400)), italic: (style["font_style"] as? String) == "italic", lineHeight: number("line_height"), letterSpacing: number("letter_spacing"))],
            align: align, lineClamp: Int(number("line_clamp")), color: c)
    }

    // Press: down and up inside the bounds.
    override func mouseDown(with event: NSEvent) {
        if handlers.contains("press") { pressed = true } else { super.mouseDown(with: event) }
    }
    override func mouseUp(with event: NSEvent) {
        guard pressed else { return super.mouseUp(with: event) }
        pressed = false
        if bounds.contains(convert(event.locationInWindow, from: nil)) { presenter?.press(id) }
    }
    func controlTextDidChange(_ obj: Notification) {
        if handlers.contains("change") { presenter?.change(id, field?.stringValue ?? "") }
    }
}

final class Presenter {
    /// The document: the roots live here, content-sized like a page.
    let root = FlippedView(frame: .zero)
    /// The viewport over it: the window's content view, scrolling like a browser's.
    let viewport = NSScrollView(frame: .zero)
    var views: [UInt32: NodeView] = [:]

    init() {
        viewport.documentView = root
        viewport.hasVerticalScroller = true
        viewport.hasHorizontalScroller = true
        viewport.autohidesScrollers = true
        viewport.drawsBackground = true
        viewport.backgroundColor = .white
    }

    /// Size the document to its roots, never smaller than the viewport.
    func fitDocument() {
        var size = viewport.contentSize
        for r in root.subviews {
            size.width = max(size.width, r.frame.maxX)
            size.height = max(size.height, r.frame.maxY)
        }
        if root.frame.size != size { root.frame = NSRect(origin: .zero, size: size) }
    }
    var onPress: ((UInt32) -> Void)?
    var onChange: ((UInt32, String) -> Void)?

    func press(_ id: UInt32) { onPress?(id) }
    func change(_ id: UInt32, _ value: String) { onChange?(id, value) }

    func apply(_ batch: Batch) {
        if let e = batch.error { FileHandle.standardError.write(Data("exact: \(e)\n".utf8)) }
        for op in batch.ops {
            guard let kind = op["op"] as? String else { continue }
            let id = UInt32(op["id"] as? Int ?? 0)
            switch kind {
            case "create":
                let v = NodeView(id: id, kind: op["kind"] as? String ?? "view", presenter: self)
                v.handlers = Set(op["handlers"] as? [String] ?? [])
                v.applyStyle(op["style"] as? [String: Any] ?? [:])
                v.applyProps(set: op["props"] as? [String: String] ?? [:], clear: [])
                views[id] = v
            case "props":
                views[id]?.applyProps(set: op["set"] as? [String: String] ?? [:], clear: op["clear"] as? [String] ?? [])
            case "style":
                views[id]?.applyStyle(op["style"] as? [String: Any] ?? [:])
            case "children":
                guard let parent = views[id] else { continue }
                let want = (op["ids"] as? [Int] ?? []).compactMap { views[UInt32($0)] }
                let container = parent.container
                for child in container.subviews where !(want as [NSView]).contains(child) && child is NodeView { child.removeFromSuperview() }
                for (i, child) in want.enumerated() {
                    if child.superview !== container { container.addSubview(child) }
                    if container.subviews.firstIndex(of: child) != i {
                        child.removeFromSuperview()
                        container.addSubview(child, positioned: .above, relativeTo: i > 0 ? want[i - 1] : nil)
                    }
                }
            case "destroy":
                views[id]?.removeFromSuperview()
                views.removeValue(forKey: id)
            case "roots":
                root.subviews.forEach { $0.removeFromSuperview() }
                for r in (op["ids"] as? [Int] ?? []).compactMap({ views[UInt32($0)] }) { root.addSubview(r) }
            case "frame":
                guard let v = views[id] else { continue }
                v.frame = NSRect(x: op["x"] as? Double ?? 0, y: op["y"] as? Double ?? 0, width: op["w"] as? Double ?? 0, height: op["h"] as? Double ?? 0)
                v.scroll?.frame = v.bounds
                v.field?.frame = v.bounds
                v.applyTransform()
            case "content":
                views[id]?.scroll?.documentView?.frame = NSRect(x: 0, y: 0, width: op["w"] as? Double ?? 0, height: op["h"] as? Double ?? 0)
            case "present":
                guard let v = views[id] else { continue }
                let x = CGFloat(op["x"] as? Double ?? 0)
                switch op["property"] as? String {
                case "translate": v.translate = CGPoint(x: x, y: CGFloat(op["y"] as? Double ?? 0)); v.applyTransform()
                case "scale": v.scale = x; v.applyTransform()
                case "rotate": v.rotate = x; v.applyTransform()
                case "opacity": v.alphaValue = x
                default: break
                }
            default: break
            }
        }
        fitDocument()
    }
}
