// The agent API on UIKit (LLP 1012; the shared half is `Agent.swift`).
// Under EXACT_AGENT=1 the driver (`scripts/agent.mjs`) owns this process
// over a Unix socket named by EXACT_AGENT_SOCKET — a simulator app has no
// stdin. `layout` reads the views as they sit in the viewport, scroll
// folded in (the web's getBoundingClientRect); `tap` hit-tests through the
// window (UIKit's own, placements included) and delivers the press by the
// responder-chain rule a touch gets — UIKit offers no public touch
// synthesis, the one declared deviation from LLP 1012's contract — or
// applies a wheel to the first scroll container up the chain that can take
// it (the web's chaining rule); `type` puts text through the field's own
// `insertText`; `screenshot` draws the viewport's hierarchy to a PNG (Metal
// layers included, so `window: true` is the same picture).
import UIKit

extension Agent {
    /// Listen on the socket; when the driver connects, `ready` goes out and
    /// requests are answered until it hangs up (which ends the process).
    static func start(ready: [String: Any]) {
        guard let path = ProcessInfo.processInfo.environment["EXACT_AGENT_SOCKET"] else {
            FileHandle.standardError.write(Data("exact: EXACT_AGENT=1 needs EXACT_AGENT_SOCKET=<path> on iOS\n".utf8))
            return
        }
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { FileHandle.standardError.write(Data("exact: socket: \(String(cString: strerror(errno)))\n".utf8)); return }
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        let capacity = MemoryLayout.size(ofValue: addr.sun_path)
        guard bytes.count < capacity else { FileHandle.standardError.write(Data("exact: socket path \(path) is longer than \(capacity - 1) bytes\n".utf8)); return }
        withUnsafeMutablePointer(to: &addr.sun_path) { p in
            p.withMemoryRebound(to: UInt8.self, capacity: capacity) { dst in
                for (i, b) in bytes.enumerated() { dst[i] = b }
                dst[bytes.count] = 0
            }
        }
        unlink(path)
        let len = socklen_t(MemoryLayout<sockaddr_un>.size)
        let bound = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, len) } }
        guard bound == 0, listen(fd, 1) == 0 else {
            FileHandle.standardError.write(Data("exact: \(path): \(String(cString: strerror(errno)))\n".utf8))
            return
        }
        Thread {
            let client = accept(fd, nil, nil)
            guard client >= 0 else { FileHandle.standardError.write(Data("exact: accept: \(String(cString: strerror(errno)))\n".utf8)); return }
            out = FileHandle(fileDescriptor: client, closeOnDealloc: false)
            reply(ready)
            serve(fd: client)
        }.start()
    }

    /// A view's box in the viewport: the viewport's content space less its
    /// offset — every enclosing scroll node's offset folded in — with the
    /// presentation transform applied (UIKit's conversion carries `transform`),
    /// as the web's `getBoundingClientRect` includes CSS transforms.
    static func box(_ v: UIView) -> CGRect {
        let vp = presenter.viewport
        let o = vp.contentOffset
        // Under a child a canvas's surface has placed (LLP 1014 D5): the box
        // where it is seen, through the placement, not the kernel's.
        if let n = v as? NodeView, let placed = n.placedAncestor, let h = placed.placement, let overlay = placed.superview, let canvas = overlay.superview as? NodeView {
            let corners = [CGPoint(x: 0, y: 0), CGPoint(x: v.bounds.width, y: 0), CGPoint(x: v.bounds.width, y: v.bounds.height), CGPoint(x: 0, y: v.bounds.height)]
                .map { NodeView.map(h, placed.convert($0, from: v)) }
            let xs = corners.map { $0.x }, ys = corners.map { $0.y }
            let inCanvas = CGRect(x: xs.min()!, y: ys.min()!, width: xs.max()! - xs.min()!, height: ys.max()! - ys.min()!)
            let r = canvas.convert(inCanvas, to: vp)
            return CGRect(x: r.origin.x - o.x, y: r.origin.y - o.y, width: r.width, height: r.height)
        }
        let r = v.convert(v.bounds, to: vp)
        return CGRect(x: r.origin.x - o.x, y: r.origin.y - o.y, width: r.width, height: r.height)
    }

    static func layout() -> [String: Any] {
        let vp = presenter.viewport
        var nodes: [[String: Any]] = []
        for (id, v) in presenter.views.sorted(by: { $0.key < $1.key }) where v.window != nil {
            let r = box(v)
            var n: [String: Any] = ["id": Int(id), "x": r2(r.origin.x), "y": r2(r.origin.y), "w": r2(r.width), "h": r2(r.height)]
            if let sv = v.scroll { n["sx"] = r2(sv.contentOffset.x); n["sy"] = r2(sv.contentOffset.y) }
            nodes.append(n)
        }
        // The page's environment (LLP 1012 §1): the insets the kernel was
        // given, and the keyboard's inset on the viewport, by the web's
        // `env()` names.
        let i = presenter.insets
        let env: [String: Any] = ["safe-area-inset-top": r2(i.top), "safe-area-inset-right": r2(i.right), "safe-area-inset-bottom": r2(i.bottom), "safe-area-inset-left": r2(i.left), "keyboard-inset-height": r2(presenter.keyboardInset)]
        return ["clock": now(), "viewport": ["w": r2(vp.bounds.width), "h": r2(vp.bounds.height)], "env": env, "nodes": nodes]
    }

    static func view(_ req: [String: Any]) -> NodeView? {
        guard let id = req["id"] as? Int else { return nil }
        return presenter.views[UInt32(id)]
    }

    static func tap(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), let win = v.window else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        let b = box(v)
        // The middle of the box as seen — through a surface's placement when
        // there is one (LLP 1014 D5) — as a point in the window.
        let vp = presenter.viewport
        let p = vp.convert(CGPoint(x: b.midX + vp.contentOffset.x, y: b.midY + vp.contentOffset.y), to: nil)
        let at = [r2(b.midX), r2(b.midY)]
        let hit = win.hitTest(p, with: nil) ?? v
        if req["hover"] as? Bool == true {
            // The pointer onto the target: the node with a hover handler at
            // the hit point enters, whatever was hovered leaves (UIKit
            // offers no pointer synthesis; a real one is the hover recognizer).
            var n: UIView? = hit
            while let cur = n, !((cur as? NodeView)?.handlers.contains("hover") ?? false) { n = cur.superview }
            if let node = n as? NodeView { presenter.hover(node, true) } else if let h = presenter.hovered { presenter.hover(h, false) }
            return ["tapped": Int(v.id), "hover": true, "at": at]
        }
        if let wheel = req["wheel"] as? [Double], wheel.count == 2 {
            // The web's sign (a positive dy scrolls down), points.
            guard wheel.allSatisfy(\.isFinite) else { return ["error": "wheel deltas must be finite"] }
            scroll(from: hit, dx: CGFloat(wheel[0]), dy: CGFloat(wheel[1]))
            return ["tapped": Int(v.id), "wheel": wheel, "at": at]
        }
        var n: UIView? = hit
        while let cur = n, !(cur is NodeView) { n = cur.superview }
        // What a touch up does first (`NodeView.touchesEnded`, up the
        // responder chain): the nearest node that takes the focus takes it
        // — an input's field, a node with a focus/blur/key handler — and
        // whatever had it (a field, and the keyboard with it) lets go.
        var f: UIView? = n
        var took = false
        while let cur = f {
            if let node = cur as? NodeView, let field = node.field { if !field.isFirstResponder { _ = field.becomeFirstResponder() }; took = true; break }
            if cur.canBecomeFirstResponder { if !cur.isFirstResponder { _ = cur.becomeFirstResponder() }; took = true; break }
            f = cur.superview
        }
        // Nothing took the focus: the field being edited loses it (a page
        // blurs its input on a click anywhere else), and the keyboard goes
        // — unless the tap is on a control that sits on the field itself
        // (a password-reveal).
        if !took, !presenter.keepsEditing(at: p) { win.endEditing(true) }
        (n as? NodeView)?.activate(at: p)
        return ["tapped": Int(v.id), "at": at]
    }

    /// The web's chaining rule (`overscroll-behavior: auto`, LLP 1010): from
    /// the hit view up, the first scroll container that can move in the
    /// wheel's dominant direction takes it — what it can of both axes — and
    /// no other moves. Whole points, bounded.
    static func scroll(from hit: UIView, dx: CGFloat, dy: CGFloat) {
        let dx = min(max(dx.rounded(), -1_000_000), 1_000_000), dy = min(max(dy.rounded(), -1_000_000), 1_000_000)
        var v: UIView? = hit
        while let cur = v {
            if let sv = cur as? ScrollView {
                let maxX = max(0, sv.contentSize.width - sv.bounds.width), maxY = max(0, sv.contentSize.height - sv.bounds.height)
                let o = sv.contentOffset
                let takeX = sv.scrollsX && dx != 0 && maxX > 0 && ((dx > 0 && o.x < maxX) || (dx < 0 && o.x > 0))
                let takeY = sv.scrollsY && dy != 0 && maxY > 0 && ((dy > 0 && o.y < maxY) || (dy < 0 && o.y > 0))
                if abs(dy) >= abs(dx) ? takeY : takeX {
                    let target = CGPoint(x: takeX ? min(max(o.x + dx, 0), maxX) : o.x, y: takeY ? min(max(o.y + dy, 0), maxY) : o.y)
                    sv.setContentOffset(target, animated: false)
                    return
                }
            }
            v = cur.superview
        }
    }

    /// Set an input's text as typing does: the field focused, all selected,
    /// the text inserted — the field sends one change with the new value.
    static func type(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), v.window != nil else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        if let key = req["key"] as? String {
            // A key at the target: the field's (Enter, as its delegate would
            // hear it) or a focused node's, by the web's name — delivered as
            // the responder-chain rule would (UIKit synthesizes no presses).
            if let f = v.field { if !f.isFirstResponder { _ = f.becomeFirstResponder() } } else if v.canBecomeFirstResponder { if !v.isFirstResponder { _ = v.becomeFirstResponder() } } else { return ["error": "view \(v.id) takes no key"] }
            // Enter at a field is what its delegate would hear: a submit,
            // and a key for a `key` handler (the field's own or an ancestor's).
            if key == "Enter", v.field != nil, v.handlers.contains("submit") { presenter.submit(v.id) }
            var n: UIView? = v
            while let cur = n, !((cur as? NodeView)?.handlers.contains("key") ?? false) { n = cur.superview }
            if let node = n as? NodeView { presenter.key(node.id, key) } else if !(key == "Enter" && v.handlers.contains("submit")) { return ["error": "no key handler at view \(v.id)"] }
            return ["typed": Int(v.id), "key": key, "value": v.field?.text ?? ""]
        }
        guard let f = v.field else { return ["error": "view \(v.id) is not an input"] }
        let text = req["text"] as? String ?? ""
        f.becomeFirstResponder()
        f.selectAll(nil)
        f.insertText(text)
        return ["typed": Int(v.id), "value": f.text ?? ""]
    }

    static func screenshot(_ req: [String: Any]) -> [String: Any] {
        guard let path = req["path"] as? String else { return ["error": "screenshot needs a path"] }
        let vp = presenter.viewport
        let scale = vp.window?.screen.scale ?? vp.traitCollection.displayScale
        let format = UIGraphicsImageRendererFormat()
        format.scale = scale
        format.opaque = true
        // 8-bit sRGB: on a wide-color screen the renderer would write a
        // 16-bit PNG, which nothing downstream (scripts/png.mjs) reads.
        format.preferredRange = .standard
        let size = vp.bounds.size
        let png = UIGraphicsImageRenderer(size: size, format: format).pngData { _ in
            vp.drawHierarchy(in: CGRect(origin: .zero, size: size), afterScreenUpdates: true)
        }
        do { try png.write(to: URL(fileURLWithPath: path)) } catch { return ["error": "write \(path): \(error)"] }
        var r: [String: Any] = ["screenshot": path, "w": r2(size.width), "h": r2(size.height), "scale": r2(scale)]
        if req["window"] as? Bool == true { r["window"] = true }
        return r
    }
}
