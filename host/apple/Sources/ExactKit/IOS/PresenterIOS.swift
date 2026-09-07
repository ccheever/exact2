// The UIKit presenter (LLP 1008 §9): the viewport scroll view over a
// content-sized document, one `NodeView` per kernel node, the host's
// batches applied, the keyboard's inset and the field it reveals. It
// belongs to one session (LLP 1031 D1) and reaches the session's canvases,
// web views, and menus through it.
#if os(iOS)
import UIKit

final class Presenter {
    /// The session this presenter shows (LLP 1031 D1).
    weak var session: ExactSession?
    /// The document: the roots live here, content-sized like a page.
    let root = PlainView(frame: .zero)
    /// The viewport over it: the window's content, scrolling like a browser's.
    let viewport = ScrollView(frame: .zero)
    var views: [UInt32: NodeView] = [:]
    /// The native menu arm (LLP 1021 D3).
    lazy var menus = MenuHost(presenter: self)
    /// The input being edited, if any (UIKit exposes no first responder):
    /// what a canvas painted through its surface captures every frame for
    /// (LLP 1014 D4 d), and what the keyboard reveals.
    weak var editing: NodeView?
    /// The first root's `viewportFit` prop (`"cover"` or nothing), as of the
    /// last batch; `onViewportFit` fires when it changes.
    private(set) var viewportFit: String?
    var onViewportFit: (() -> Void)?
    /// The first root's `interactiveWidget` prop: `resizes-content` ends the
    /// layout viewport at the keyboard's top (the controller lays out again,
    /// inside the keyboard's animation, so every frame that moves moves with
    /// it); anything else is the default, `resizes-visual` — the inset below.
    private(set) var interactiveWidget: String?
    /// The keyboard's top edge in the window while one is shown, else nil.
    private(set) var keyboardTop: CGFloat?
    /// The controller's: frame the viewport again (`Controller.fit`).
    var onKeyboardResize: (() -> Void)?
    /// The safe-area insets the kernel was given (LLP 1008 §9): the
    /// screen's under `viewport-fit=cover`, zero when the viewport is the
    /// safe area itself. Reported to the agent as `env`.
    var insets = UIEdgeInsets.zero
    /// The keyboard's inset on the viewport: the points of the screen's
    /// viewport a software keyboard covers. By default the web's visual
    /// viewport — the layout viewport does not change; the viewport insets
    /// its content by this and reveals the field being edited, in the
    /// keyboard's own animation. Under `resizes-content` the controller
    /// sets it, measured against the viewport it would frame without a
    /// keyboard.
    var keyboardInset: CGFloat = 0

    init() {
        viewport.addSubview(root)
        viewport.contentInsetAdjustmentBehavior = .never
        viewport.backgroundColor = .white
    }

    func observeKeyboard() {
        let c = NotificationCenter.default
        c.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
        c.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillHideNotification, object: nil)
    }

    /// The keyboard is about to move: inset the viewport by what it will
    /// cover and reveal the field, inside an animation with the keyboard's
    /// own duration and curve — Core Animation runs both in the same
    /// transaction, so the content moves in lockstep with the keyboard,
    /// never a frame behind it.
    @objc func keyboardChanged(_ n: Notification) {
        guard let info = n.userInfo, let window = viewport.window, viewport.superview != nil else { return }
        let end = (info[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect) ?? .zero
        let hiding = n.name == UIResponder.keyboardWillHideNotification
        // The keyboard's frame is the screen's; the viewport's, the window's.
        let keyboard = window.convert(end, from: window.screen.coordinateSpace)
        let shown = !hiding && keyboard.minY < window.bounds.maxY
        let top: CGFloat? = shown ? keyboard.minY : nil
        let duration = info[UIResponder.keyboardAnimationDurationUserInfoKey] as? Double ?? 0.25
        let curve = info[UIResponder.keyboardAnimationCurveUserInfoKey] as? UInt ?? 7
        // A focus moving from one field to another comes as a burst of
        // notifications with no duration, over a few turns — the height
        // jittering between the two keyboards (335, 308, 335 on the
        // simulator) — and laying out for each flashed the page. Those wait
        // 80 ms for the last of them, which usually changes nothing. An
        // animated change (the show, the hide) is applied at once, in the
        // keyboard's own transaction, so it stays in step with the keyboard.
        keyboardDebounce?.cancel()
        keyboardDebounce = nil
        if duration > 0 {
            applyKeyboard(top: top, duration: duration, curve: curve)
        } else {
            let work = DispatchWorkItem { [weak self] in
                self?.keyboardDebounce = nil
                self?.applyKeyboard(top: top, duration: 0, curve: curve)
            }
            keyboardDebounce = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.08, execute: work)
        }
    }
    /// The last no-duration keyboard change, waiting to be applied.
    private var keyboardDebounce: DispatchWorkItem?

    /// The keyboard's top edge (nil: hidden) takes effect: the viewport is
    /// inset by the overlap (`resizes-visual`, the default) or laid out to
    /// end there (`resizes-content`, the controller's `fit`), the field
    /// being edited revealed after — inside an animation with the keyboard's
    /// own duration and curve, so the frames the batch sets are Core
    /// Animation moves in the keyboard's transaction, never a frame behind.
    func applyKeyboard(top: CGFloat?, duration: Double, curve: UInt) {
        guard let window = viewport.window, let parent = viewport.superview else { return }
        keyboardTop = top
        let change = {
            if self.interactiveWidget == "resizes-content" {
                self.onKeyboardResize?()
            } else {
                let frame = parent.convert(self.viewport.frame, to: window)
                let overlap = top.map { min(max(0, frame.maxY - max($0, frame.minY)), frame.height) } ?? 0
                self.setKeyboardInset(overlap)
            }
            self.reveal(self.editing ?? self.views.values.first { $0.field?.isFirstResponder == true })
        }
        // Under the agent (LLP 1012) the change applies at once, as the
        // agent's wheel scrolls at once: its world is settled between calls,
        // and UIKit hit-tests a scroll view at its presentation offset while
        // the keyboard's spring is still settling — a tap there would miss.
        if ExactEnv.agentMode || duration <= 0 { change(); return }
        UIView.animate(withDuration: duration, delay: 0, options: [UIView.AnimationOptions(rawValue: curve << 16), .beginFromCurrentState], animations: change)
    }

    func setKeyboardInset(_ h: CGFloat) {
        keyboardInset = h
        var inset = viewport.contentInset
        inset.bottom = h
        viewport.contentInset = inset
        var indicators = viewport.verticalScrollIndicatorInsets
        indicators.bottom = h
        viewport.verticalScrollIndicatorInsets = indicators
    }

    /// Scroll a node into the part of the viewport the keyboard leaves —
    /// the browser's rule for a focused field — through every scroll
    /// container above it, each moving only as far as it must, never past
    /// its edges. Animated by whatever animation block this runs inside.
    func reveal(_ node: NodeView?) {
        guard let node, node.window != nil else { return }
        var v: UIView? = node.superview
        while let cur = v {
            if let sv = cur as? ScrollView {
                // The node's box in the container's content space, with a
                // little air; the visible part of that space.
                let r = node.convert(node.bounds, to: sv).insetBy(dx: 0, dy: -8)
                let visible = sv.bounds.inset(by: sv.adjustedContentInset)
                var o = sv.contentOffset
                if r.maxY > visible.maxY { o.y += r.maxY - visible.maxY } else if r.minY < visible.minY { o.y -= visible.minY - r.minY }
                if r.maxX > visible.maxX { o.x += r.maxX - visible.maxX } else if r.minX < visible.minX { o.x -= visible.minX - r.minX }
                let i = sv.adjustedContentInset
                o.y = min(max(o.y, -i.top), max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height))
                o.x = min(max(o.x, -i.left), max(-i.left, sv.contentSize.width + i.right - sv.bounds.width))
                if o != sv.contentOffset { sv.contentOffset = o }
            }
            v = cur.superview
        }
    }

    /// The viewport's size in points: what the kernel lays out under.
    var viewportSize: CGSize { viewport.bounds.size }
    /// The first root's frame size, zero before the first batch.
    var rootSize: CGSize { root.subviews.first?.frame.size ?? .zero }

    /// An asset's bytes changed (LLP 1030 D10): every image showing it loads
    /// it again — the old picture stays until the new one is decoded, as a
    /// browser keeps the old `src`.
    func assetChanged(_ name: String) {
        for v in views.values where v.kind == "image" && v.imageSource == name { v.loadImage(name) }
    }

    /// A restart: every view goes.
    func reset() {
        session?.canvases.reset()
        views.values.forEach { $0.forget() }
        root.subviews.forEach { $0.removeFromSuperview() }
        views.removeAll()
        NodeView.imagesLoaded.removeAll()
    }

    /// Size the document to its roots, never smaller than the viewport.
    func fitDocument() {
        var size = viewport.bounds.size
        for r in root.subviews {
            size.width = max(size.width, r.frame.maxX)
            size.height = max(size.height, r.frame.maxY)
        }
        if root.frame.size != size { root.frame = CGRect(origin: .zero, size: size) }
        if viewport.contentSize != size { viewport.contentSize = size }
    }
    var onPress: ((UInt32) -> Void)?
    var onChange: ((UInt32, String) -> Void)?
    var onIntrinsic: ((UInt32, CGSize?) -> Void)?
    /// A capability an action called (LLP 1005 §3), after its commit.
    var onCommand: ((String, [Any]) -> Void)?
    /// The events beyond press and change (LLP 1005 §3).
    var onHover: ((UInt32, Bool) -> Void)?
    var onFocus: ((UInt32) -> Void)?
    var onBlur: ((UInt32) -> Void)?
    var onKey: ((UInt32, String) -> Void)?
    var onSubmit: ((UInt32) -> Void)?
    var onLoad: ((UInt32) -> Void)?
    var onMessage: ((UInt32, String) -> Void)?
    /// The node the pointer is over, of those with a hover handler: it hears
    /// the leave when the pointer moves onto another (the agent's `hover`).
    weak var hovered: NodeView?

    func press(_ id: UInt32) { onPress?(id) }
    func change(_ id: UInt32, _ value: String) { onChange?(id, value) }
    /// An event a view reports: sent only while the presenter still has the
    /// view (the platform fires editing-ended as a destroyed field leaves the
    /// window; the browser fires no blur on removal, so neither does this
    /// host), and never while a batch is being applied — it waits for the
    /// batch to finish, then goes if its view survived it.
    private var applying = false
    private var waiting: [(UInt32, () -> Void)] = []
    private func send(_ id: UInt32, _ f: @escaping () -> Void) {
        guard views[id] != nil else { return }
        if applying { waiting.append((id, f)) } else { f() }
    }
    func hover(_ view: NodeView, _ over: Bool) {
        guard views[view.id] === view else { return }
        if over {
            if let h = hovered, h !== view { send(h.id) { [self] in onHover?(h.id, false) } }
            hovered = view
            send(view.id) { [self] in onHover?(view.id, true) }
        } else {
            if hovered === view { hovered = nil }
            send(view.id) { [self] in onHover?(view.id, false) }
        }
    }
    func focus(_ id: UInt32) { send(id) { [self] in onFocus?(id) } }
    func blur(_ id: UInt32) { send(id) { [self] in onBlur?(id) } }
    func key(_ id: UInt32, _ name: String) { send(id) { [self] in onKey?(id, name) } }
    func submit(_ id: UInt32) { send(id) { [self] in onSubmit?(id) } }
    func load(_ id: UInt32) { send(id) { [self] in onLoad?(id) } }
    func message(_ id: UInt32, _ value: String) { send(id) { [self] in onMessage?(id, value) } }
    func intrinsic(_ id: UInt32, _ size: CGSize?) { onIntrinsic?(id, size) }

    func apply(_ batch: Batch) {
        if let e = batch.error { FileHandle.standardError.write(Data("exact: \(e)\n".utf8)) }
        let outermost = !applying
        applying = true
        defer {
            if outermost {
                applying = false
                let q = waiting
                waiting = []
                for (id, f) in q where views[id] != nil { f() }
            }
        }
        for op in batch.ops {
            guard let kind = op["op"] as? String else { continue }
            let id = UInt32(op["id"] as? Int ?? 0)
            if kind == "children" { touched(id, children: true) } else if kind != "roots" && kind != "create" { touched(id, textChanged: kind == "props" || kind == "style" || kind == "destroy") }
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
                if parent.kind == "text" { parent.setTextChildren(want); continue }
                for child in want { child.textParent = nil }
                let container = parent.container
                for child in container.subviews where !(want as [UIView]).contains(child) && child is NodeView { child.removeFromSuperview() }
                // In order, below anything else in the container (a scroll
                // view's indicators): inserting a subview at an index moves
                // it when it is already there.
                for (i, child) in want.enumerated() { container.insertSubview(child, at: i) }
            case "surface":
                if let v = views[id] { session?.canvases.surface(view: v, name: op["name"] as? String ?? "", values: op["values"] as? [Any] ?? []) }
            case "command":
                onCommand?(op["name"] as? String ?? "", op["args"] as? [Any] ?? [])
            case "destroy":
                session?.canvases.destroy(view: id)
                views[id]?.forget()
                // Out of the map before out of the window: the editing-ended
                // notification removal fires finds no view to send for.
                let gone = views.removeValue(forKey: id)
                gone?.removeFromSuperview()
            case "roots":
                root.subviews.forEach { $0.removeFromSuperview() }
                for r in (op["ids"] as? [Int] ?? []).compactMap({ views[UInt32($0)] }) { root.addSubview(r) }
            case "frame":
                guard let v = views[id] else { continue }
                // A frame is set untransformed (UIKit's `frame` is undefined
                // under a transform); the presentation goes back on after.
                v.transform = .identity
                v.frame = CGRect(x: op["x"] as? Double ?? 0, y: op["y"] as? Double ?? 0, width: op["w"] as? Double ?? 0, height: op["h"] as? Double ?? 0)
                v.scroll?.frame = v.bounds
                v.field?.frame = v.fieldBox()
                v.layoutTextArea()
                v.metal?.frame = v.bounds
                v.overlay?.frame = v.bounds
                v.web?.frame = v.bounds
                v.fitScroll()
                v.applyTransform()
            case "content":
                guard let v = views[id] else { continue }
                v.content = CGSize(width: op["w"] as? Double ?? 0, height: op["h"] as? Double ?? 0)
                v.fitScroll()
            case "present":
                guard let v = views[id] else { continue }
                let x = CGFloat(op["x"] as? Double ?? 0)
                switch op["property"] as? String {
                case "translate": v.translate = CGPoint(x: x, y: CGFloat(op["y"] as? Double ?? 0)); v.applyTransform()
                case "scale": v.scale = x; v.applyTransform()
                case "rotate": v.rotate = x; v.applyTransform()
                case "opacity": v.alpha = x
                default: break
                }
            default: break
            }
        }
        fitDocument()
        paintCanvas()
        let first = root.subviews.first as? NodeView
        interactiveWidget = first?.props["interactiveWidget"]
        let fit = first?.props["viewportFit"]
        if fit != viewportFit { viewportFit = fit; onViewportFit?() }
        session?.canvases.captureIfNeeded()
        menus.sync()
    }

    /// The page's canvas colour — behind the document and into the safe
    /// areas the layout keeps out of — is the first root's background, as
    /// Safari paints the root element's background under the status bar
    /// and the home indicator; white when the root sets none.
    var onCanvasColor: ((UIColor) -> Void)?
    func paintCanvas() {
        let color = (root.subviews.first as? NodeView)?.color("background_color", .white) ?? .white
        if viewport.backgroundColor != color { viewport.backgroundColor = color; onCanvasColor?(color) }
    }

    /// An op touched a node (LLP 1014 D4 a): every canvas it is painted
    /// through captures again at the end of the batch — the canvas above
    /// it, and itself for its own `children` op.
    func touched(_ id: UInt32, children: Bool = false, textChanged: Bool = false) {
        guard let start = views[id] else { return }
        var paragraph: NodeView? = start
        while let node = paragraph, node.kind == "text" {
            if textChanged || children { node.invalidateText() }
            node.setNeedsDisplay()
            paragraph = node.textParent ?? node.superview as? NodeView
        }
        if children, start.overlay != nil { start.needsCapture = true }
        if let c = start.paragraphOwner.canvasAbove { c.needsCapture = true }
    }
}

/// Pixels a view's subtree was painted into: premultiplied RGBA, rows
/// top-down, `width * 4` bytes per row, owned by the context.
struct Bitmap {
    let context: CGContext
    let width: Int
    let height: Int
    var bytes: UnsafeMutableRawPointer? { context.data }
    var bytesPerRow: Int { context.bytesPerRow }
    /// Empty pixels for the module to fill (a readback).
    static func blank(width: Int, height: Int) -> Bitmap? {
        guard width > 0, height > 0, let space = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue)
        else { return nil }
        return Bitmap(context: ctx, width: width, height: height)
    }
    var image: UIImage? { context.makeImage().map { UIImage(cgImage: $0) } }
}

/// A view's subtree as pixels (LLP 1014 D3).
enum Capture {
    /// A capture is drawing: its draws are not repaints (D4 b).
    nonisolated(unsafe) static var capturing = false
    /// Guest pictures when this turn chooses arm snapshots; their remote
    /// platform views are hidden while the nodes draw these (LLP 1020 D4/D6).
    nonisolated(unsafe) static var web: [UInt32: ExactWebImage] = [:]
    /// EXACT_CAPTURE=cpu: the Core Graphics capture even where Metal is
    /// present — the measure's baseline, and the fixture's oracle.
    static let cpu = ProcessInfo.processInfo.environment["EXACT_CAPTURE"] == "cpu"
    /// The subtree painted at `scale`: premultiplied RGBA, rows top-down,
    /// transparent where nothing painted.
    static func bitmap(of view: UIView, scale: CGFloat) -> Bitmap? {
        let w = Int((view.bounds.width * scale).rounded()), h = Int((view.bounds.height * scale).rounded())
        guard let bitmap = Bitmap.blank(width: w, height: h) else { return nil }
        // The GPU, where there is one (`Shadow`); Core Graphics otherwise.
        if !cpu, let shadow = Shadow.shared {
            capturing = true
            let ok = shadow.render(view, scale: scale, into: bitmap)
            capturing = false
            if ok {
                // EXACT_CAPTURE_DUMP=<dir>: this render and the CPU one of the
                // same frame, as PNGs, to compare the two by eye.
                if let dir = ProcessInfo.processInfo.environment["EXACT_CAPTURE_DUMP"], dumped < 4 {
                    dumped += 1
                    try? bitmap.image?.pngData()?.write(to: URL(fileURLWithPath: dir).appendingPathComponent("gpu-\(dumped).png"))
                    if let cpuBitmap = Bitmap.blank(width: w, height: h) { draw(view, scale: scale, into: cpuBitmap); try? cpuBitmap.image?.pngData()?.write(to: URL(fileURLWithPath: dir).appendingPathComponent("cpu-\(dumped).png")) }
                }
                return bitmap
            }
        }
        draw(view, scale: scale, into: bitmap)
        return bitmap
    }
    nonisolated(unsafe) static var dumped = 0

    /// The CPU capture: Core Graphics rasterizes the subtree into `bitmap`.
    static func draw(_ view: UIView, scale: CGFloat, into bitmap: Bitmap) {
        let h = bitmap.height
        let ctx = bitmap.context
        // UIKit's geometry — y down from the top — into a context whose y
        // is up from the bottom: the first row in memory is then the top.
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: scale, y: -scale)
        // A subtree painted through its canvas composites at alpha 0; paint
        // it opaque into the bitmap regardless.
        let alpha = view.alpha
        view.alpha = 1
        // A canvas nested under this one that is painted through its own
        // surface: its picture comes by readback (its draw), not from its
        // overlay's views, which the render would paint regardless of their
        // alpha — so those are hidden for the duration.
        var hidden: [UIView] = []
        func hide(_ v: UIView) {
            for s in v.subviews {
                // A nested canvas paints its readback in `draw` (LLP 1014):
                // drop the layer's cached picture so the render calls `draw`
                // instead of copying what it drew last time (its placements
                // are read there too, so the cards of a deck in the sky move).
                if let n = s as? NodeView, n.kind == "canvas" { n.layer.contents = nil; n.setNeedsDisplay() }
                if let n = s as? NodeView, let o = n.overlay, o.alpha == 0, !o.isHidden { o.isHidden = true; hidden.append(o); continue }
                hide(s)
            }
        }
        hide(view)
        capturing = true
        UIGraphicsPushContext(ctx)
        view.layer.render(in: ctx)
        UIGraphicsPopContext()
        capturing = false
        for o in hidden { o.isHidden = false }
        view.alpha = alpha
    }
}

extension CGRect {
    /// The rect inside the given edges (never negative in size).
    func insetBy(left: CGFloat, top: CGFloat, right: CGFloat, bottom: CGFloat) -> CGRect {
        CGRect(x: minX + left, y: minY + top, width: max(0, width - left - right), height: max(0, height - top - bottom))
    }
}
#endif
