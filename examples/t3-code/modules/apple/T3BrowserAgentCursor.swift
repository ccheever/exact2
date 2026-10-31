#if os(macOS)
import AppKit
import WebKit

/// The agent's cursor over its tab's page (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
/// apps/web/src/components/preview/AgentBrowserCursor.tsx, agentBrowserCursorLogic.ts): an arrow that glides to the
/// agent's point (150 ms), pings on a click, and settles to 35 % after 700 ms (18 % while the person has the page).
/// It is a layer of the page's own view (the frame hands it the `t3-browser-page` hatch, T3BrowserPage.swift), so it
/// travels with the page and never takes a click; the framework has no overlay on a frame's guest yet (B10).
enum T3BrowserAgentCursor {
    static let activeMs = 700
    private static let key = "t3-agent-cursor"

    static func opacity(active: Bool, controller: String) -> Float { active ? 1 : controller == "human" ? 0.18 : 0.35 }

    static func show(on web: WKWebView, phase: String, x: Double, y: Double, sequence: Int, controller: String) {
        web.wantsLayer = true
        guard let root = web.layer else { return }
        let cursor = root.sublayers?.first(where: { $0.name == key }) ?? make(in: root)
        let scale = web.pageZoom * web.magnification
        // The page's point in the view's own coordinates, then in its layer's. The layer's axes follow every flipped
        // ancestor, which its own `isGeometryFlipped` does not say: reading that alone put the cursor at the page's
        // height minus the target's y.
        let position = web.convertToLayer(NSPoint(x: x * scale, y: web.isFlipped ? y * scale : web.bounds.height - y * scale))
        orient(cursor, down: layerPointsDown(web))
        CATransaction.begin()
        CATransaction.setAnimationDuration(0.15)
        CATransaction.setAnimationTimingFunction(CAMediaTimingFunction(name: .easeOut))
        cursor.position = position
        cursor.opacity = opacity(active: true, controller: controller)
        CATransaction.commit()
        if phase == "click", let ping = cursor.sublayers?.first(where: { $0.name == "ping" }) {
            let grow = CABasicAnimation(keyPath: "transform.scale"); grow.fromValue = 0.6; grow.toValue = 2.2
            let fade = CABasicAnimation(keyPath: "opacity"); fade.fromValue = 0.6; fade.toValue = 0
            let group = CAAnimationGroup(); group.animations = [grow, fade]; group.duration = 0.6
            ping.add(group, forKey: "ping")
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(activeMs)) { [weak cursor] in
            guard let cursor, cursor.value(forKey: "sequence") as? Int == sequence else { return }
            cursor.opacity = opacity(active: false, controller: controller)
        }
        cursor.setValue(sequence, forKey: "sequence")
    }

    /// Whether the web view's layer draws its y axis downward on screen: a step down in the view (its y grows when it is
    /// flipped) against the same step in its layer.
    static func layerPointsDown(_ web: NSView) -> Bool {
        let top = web.convertToLayer(NSPoint.zero), below = web.convertToLayer(NSPoint(x: 0, y: web.isFlipped ? 1 : -1))
        return below.y > top.y
    }

    /// The tip at the cursor's top left, the arrow drawn y-down, in the page layer's current orientation (the frame is
    /// scaled into a fixed viewport's box).
    private static func orient(_ cursor: CALayer, down: Bool) {
        if cursor.value(forKey: "down") as? Bool == down { return }
        cursor.setValue(down, forKey: "down")
        cursor.anchorPoint = CGPoint(x: 0.1, y: down ? 0.1 : 0.9)
        guard let arrow = cursor.sublayers?.first(where: { $0.name == "arrow" }) as? CAShapeLayer else { return }
        let path = CGMutablePath()
        let point = { (x: CGFloat, y: CGFloat) in CGPoint(x: x, y: down ? y : 20 - y) } // drawn y-down, the tip at the top left
        cursor.sublayers?.first(where: { $0.name == "ping" })?.position = point(2, 2)
        path.move(to: point(2, 2)); path.addLine(to: point(8.5, 18)); path.addLine(to: point(10.8, 10.8)); path.addLine(to: point(18, 8.5)); path.closeSubpath()
        arrow.path = path
    }

    private static func make(in root: CALayer) -> CALayer {
        let cursor = CALayer()
        cursor.name = key
        cursor.bounds = CGRect(x: 0, y: 0, width: 20, height: 20)
        cursor.zPosition = 1_000
        let ping = CALayer()
        ping.name = "ping"
        ping.bounds = CGRect(x: 0, y: 0, width: 16, height: 16)
        ping.cornerRadius = 8
        ping.backgroundColor = NSColor.controlAccentColor.withAlphaComponent(0.25).cgColor
        ping.opacity = 0
        cursor.addSublayer(ping)
        // lucide mouse-pointer-2: an arrow filled with the page's background, stroked in the accent colour (its path and
        // the ping's place are `orient`'s).
        let arrow = CAShapeLayer()
        arrow.name = "arrow"
        arrow.fillColor = NSColor.windowBackgroundColor.cgColor
        arrow.strokeColor = NSColor.controlAccentColor.cgColor
        arrow.lineWidth = 1.6
        arrow.lineJoin = .round
        arrow.frame = cursor.bounds
        arrow.shadowOpacity = 0.25; arrow.shadowRadius = 1.5; arrow.shadowOffset = .zero
        cursor.addSublayer(arrow)
        root.addSublayer(cursor)
        return cursor
    }
}
#endif
