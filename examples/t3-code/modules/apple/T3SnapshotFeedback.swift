import AppKit
import QuartzCore

/// Capture feedback, after the reference desktop SnapShotTransition and
/// SnapShotFlash. Every begun flight ends exactly once — landing, dismissal,
/// the reference's six-second deadline, disable or destroy — and only then
/// runs its completions. Completion and cancellation are identity-safe: a
/// stale callback can never end a newer flight with the same capture id.
final class T3SnapshotFeedback {
    /// Reference SnapShotTransition TIMEOUT_MS; independent of draft adoption.
    static let flightTimeout: TimeInterval = 6
    /// Where a flight lands: the attachment frame and its painted styling.
    struct Destination { let frame: CGRect; let radius: CGFloat; let border: CGFloat; let borderColor: NSColor; let background: NSColor }
    final class Flight {
        let id: String
        let panel: NSPanel
        let card: CALayer
        let image: CALayer
        let source: CGRect
        fileprivate(set) var landing = false
        fileprivate(set) var finished = false
        fileprivate(set) var landed = false
        fileprivate var completions: [(Bool) -> Void] = []
        fileprivate var deadline: DispatchWorkItem?
        init(id: String, panel: NSPanel, card: CALayer, image: CALayer, source: CGRect) { self.id = id; self.panel = panel; self.card = card; self.image = image; self.source = source }
    }
    private var sound: NSSound?
    private var flash: NSPanel?
    private var flashTimer: Timer?
    private(set) var flights: [String: Flight] = [:]
    var timeout = T3SnapshotFeedback.flightTimeout
    var reduceMotion: () -> Bool = { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }
    var screens: () -> [CGRect] = { NSScreen.screens.map(\.frame) }
    /// Every ended flight (landed, dismissed, timed out, cancelled) reports its id once.
    var ended: (String) -> Void = { _ in }

    func play(_ choice: String) -> Bool {
        let file = choice == "soft-pop" ? "snap-shot-whoosh.mp3" : choice == "camera-shutter" ? "snap-shot-click.mp3" : ""
        guard !file.isEmpty else { return false }
        var candidates: [URL] = []
        if let root = ProcessInfo.processInfo.environment["EXACT_ASSETS"], root.hasPrefix("/") { candidates.append(URL(fileURLWithPath: root).appendingPathComponent("assets/\(file)")) }
        if let resources = Bundle.main.resourceURL { candidates.append(resources.appendingPathComponent("assets/\(file)")) }
        if let executable = Bundle.main.executableURL { candidates.append(executable.deletingLastPathComponent().appendingPathComponent("assets/\(file)")) }
        guard let path = candidates.first(where: { FileManager.default.fileExists(atPath: $0.path) }), let next = NSSound(contentsOf: path, byReference: false) else { return false }
        sound?.stop(); sound = next; return next.play()
    }

    private static func overlay(_ frame: CGRect) -> NSPanel {
        let panel = NSPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isOpaque = false; panel.backgroundColor = .clear; panel.hasShadow = false
        panel.ignoresMouseEvents = true; panel.level = .screenSaver; panel.isReleasedWhenClosed = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]
        return panel
    }
    /// Reference SnapShotFlash: a white 0.08 window over the capture, held 60 ms
    /// (static) or faded out over 180 ms (animated).
    func show(bounds: CGRect, animated: Bool) {
        flashTimer?.invalidate(); flashTimer = nil; flash?.close(); flash = nil
        guard bounds.width > 0, bounds.height > 0, let primary = screens().first else { return }
        let panel = Self.overlay(Self.screenFrame(bounds, screenHeight: primary.maxY))
        panel.backgroundColor = .white; panel.alphaValue = 0.08
        flash = panel; panel.orderFrontRegardless()
        let duration = animated ? 0.18 : 0.06, started = ProcessInfo.processInfo.systemUptime
        flashTimer = Timer.scheduledTimer(withTimeInterval: 0.016, repeats: true) { [weak self, weak panel] timer in
            let elapsed = ProcessInfo.processInfo.systemUptime - started
            if animated { panel?.alphaValue = max(0, 0.08 * CGFloat(1 - elapsed / duration)) }
            guard elapsed >= duration else { return }
            timer.invalidate(); panel?.close()
            if let self, self.flash === panel { self.flash = nil; self.flashTimer = nil }
        }
    }
    static func screenFrame(_ bounds: CGRect, screenHeight: CGFloat) -> CGRect {
        CGRect(x: bounds.minX, y: screenHeight - bounds.maxY, width: bounds.width, height: bounds.height)
    }
    /// Reference snapShotAnimationDurationMs: 280–680 ms by flight distance.
    static func duration(from source: CGRect, to target: CGRect) -> TimeInterval {
        let distance = hypot(source.midX - target.midX, source.midY - target.midY)
        return (680 - (680 - 280) * exp(-Double(distance) / 2000)) / 1000
    }
    static let easing = CAMediaTimingFunction(controlPoints: 0.2, 0.8, 0.2, 1)

    /// Covers every display, as the reference overlay does, with the captured
    /// window as a 12 pt card over its source. Returns false when motion is off.
    @discardableResult
    func begin(id: String, image: CGImage, bounds: CGRect, flash: Bool) -> Bool {
        // The reference keeps one active transition; a newer capture replaces it.
        cancelFlights()
        let displays = screens()
        guard !reduceMotion(), bounds.width > 0, bounds.height > 0, let primary = displays.first else { return false }
        let union = displays.dropFirst().reduce(primary) { $0.union($1) }
        let panel = Self.overlay(union)
        // Layer-hosting: the card's geometry and animation are this module's own.
        let root = NSView(frame: CGRect(origin: .zero, size: union.size)); root.layer = CALayer(); root.wantsLayer = true
        panel.contentView = root
        let source = Self.screenFrame(bounds, screenHeight: primary.maxY)
        let card = CALayer(), picture = CALayer()
        card.frame = source.offsetBy(dx: -union.minX, dy: -union.minY)
        card.cornerRadius = 12; card.masksToBounds = false; card.backgroundColor = NSColor.white.cgColor
        card.shadowColor = NSColor.black.cgColor; card.shadowOpacity = 0.24; card.shadowRadius = 35; card.shadowOffset = CGSize(width: 0, height: -24)
        picture.frame = card.bounds; picture.contents = image; picture.contentsGravity = .resizeAspectFill
        picture.cornerRadius = 12; picture.masksToBounds = true
        card.addSublayer(picture)
        if flash {
            // Reference startCaptureFlash keyframes over 300 ms.
            let white = CALayer(); white.frame = card.bounds; white.backgroundColor = NSColor.white.cgColor; white.cornerRadius = 12; white.opacity = 0
            let pulse = CAKeyframeAnimation(keyPath: "opacity")
            pulse.values = [0.08, 0.08, 0.02, 0]; pulse.keyTimes = [0, 0.38, 0.68, 1]; pulse.duration = 0.3; pulse.timingFunction = Self.easing
            white.add(pulse, forKey: "flash"); card.addSublayer(white)
        }
        root.layer?.addSublayer(card)
        let flight = Flight(id: id, panel: panel, card: card, image: picture, source: source)
        let deadline = DispatchWorkItem { [weak self, weak flight] in self?.finish(flight, landed: false) }
        flight.deadline = deadline
        flights[id] = flight; panel.orderFrontRegardless()
        DispatchQueue.main.asyncAfter(deadline: .now() + timeout, execute: deadline)
        return true
    }

    /// Fly onto the painted attachment, then end. `reveal` runs before the
    /// overlay closes so the real tile is never missing for a frame. Without a
    /// live flight (none, dismissed, timed out) completion runs at once.
    func land(id: String, target: Destination?, reveal: @escaping () -> Void = {}, completion: @escaping (Bool) -> Void) {
        guard let flight = flights[id], !flight.finished else { reveal(); completion(true); return }
        flight.completions.append(completion)
        guard !flight.landing else { return }
        flight.landing = true
        guard !reduceMotion(), let target, target.frame.width > 0, target.frame.height > 0 else { reveal(); finish(flight, landed: true); return }
        let origin = flight.panel.frame.origin
        let frame = target.frame.offsetBy(dx: -origin.x, dy: -origin.y)
        let duration = Self.duration(from: flight.source, to: target.frame)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        CATransaction.setCompletionBlock { [weak self, weak flight] in
            guard let self, let flight, !flight.finished else { return }
            reveal()
            // Close after the revealed tile has been displayed.
            DispatchQueue.main.async { [weak self, weak flight] in self?.finish(flight, landed: true) }
        }
        func animate(_ layer: CALayer, _ key: String, _ value: Any) {
            let animation = CABasicAnimation(keyPath: key)
            animation.fromValue = layer.value(forKeyPath: key); animation.toValue = value
            animation.duration = duration; animation.timingFunction = Self.easing
            layer.setValue(value, forKeyPath: key); layer.add(animation, forKey: key)
        }
        let size = CGRect(origin: .zero, size: frame.size)
        animate(flight.card, "bounds", NSValue(rect: size)); animate(flight.card, "position", NSValue(point: CGPoint(x: frame.midX, y: frame.midY)))
        animate(flight.image, "bounds", NSValue(rect: size)); animate(flight.image, "position", NSValue(point: CGPoint(x: size.midX, y: size.midY)))
        animate(flight.card, "cornerRadius", target.radius); animate(flight.image, "cornerRadius", target.radius)
        animate(flight.card, "shadowOpacity", Float(0)); animate(flight.card, "backgroundColor", target.background.cgColor)
        animate(flight.card, "borderWidth", target.border); animate(flight.card, "borderColor", target.borderColor.cgColor)
        CATransaction.commit()
    }
    private func finish(_ flight: Flight?, landed: Bool) {
        guard let flight, !flight.finished else { return }
        flight.finished = true; flight.landed = landed
        flight.deadline?.cancel(); flight.deadline = nil
        flight.panel.orderOut(nil); flight.panel.close()
        if flights[flight.id] === flight { flights.removeValue(forKey: flight.id) }
        ended(flight.id)
        let callbacks = flight.completions; flight.completions = []
        callbacks.forEach { $0(landed) }
    }
    func dismiss(id: String) { finish(flights[id], landed: false) }
    func cancelFlights() { for flight in Array(flights.values) { finish(flight, landed: false) } }
    func destroy() { sound?.stop(); sound = nil; flashTimer?.invalidate(); flashTimer = nil; flash?.close(); flash = nil; cancelFlights() }
}
