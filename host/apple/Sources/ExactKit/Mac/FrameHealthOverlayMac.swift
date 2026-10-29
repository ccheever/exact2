#if os(macOS)
import AppKit

private final class FrameHealthMacHandle: NSButton {
    var move: ((CGPoint) -> Void)?
    var finished: (() -> Void)?
    override func mouseDown(with event: NSEvent) {
        guard let window else { return }
        let origin = event.locationInWindow
        while let next = window.nextEvent(matching: [.leftMouseDragged, .leftMouseUp]) {
            if next.type == .leftMouseUp { finished?(); return }
            move?(CGPoint(x: next.locationInWindow.x - origin.x, y: next.locationInWindow.y - origin.y))
        }
        finished?()
    }
}

/// A small opaque host card. Only its heading and controls intercept events.
final class FrameHealthOverlayMac: NSView {
    weak var health: FrameHealth?
    private let card = NSView()
    private let heading = FrameHealthMacHandle(title: "≡  Frame Health", target: nil, action: nil)
    private let close = NSButton(image: NSImage(systemSymbolName: "xmark.circle.fill", accessibilityDescription: "Hide frame health")!, target: nil, action: nil)
    private let details = NSButton(image: NSImage(systemSymbolName: "info.circle", accessibilityDescription: "Frame health details")!, target: nil, action: nil)
    private let values = NSTextField(labelWithString: "")
    private let coverage = NSTextField(labelWithString: "Callback proxy · presentation unknown")
    private var dragOrigin: CGPoint?
    private var placed = false
    var desiredVisible = false
    var showDetails: (() -> Void)?

    init(health: FrameHealth) {
        self.health = health
        super.init(frame: .zero)
        identifier = NSUserInterfaceItemIdentifier("exact.host.frame-health-overlay")
        setAccessibilityIdentifier("exact.host.frame-health-overlay")
        isHidden = true
        card.wantsLayer = true
        card.layer?.backgroundColor = NSColor.controlBackgroundColor.cgColor
        card.layer?.cornerRadius = 12
        card.layer?.borderWidth = 1
        card.layer?.borderColor = NSColor.separatorColor.cgColor
        addSubview(card)
        for button in [heading, close, details] { button.isBordered = false; card.addSubview(button) }
        heading.alignment = .left
        heading.setAccessibilityLabel("Move frame health")
        heading.move = { [weak self] delta in
            guard let self else { return }
            if self.dragOrigin == nil { self.dragOrigin = self.card.frame.origin; self.health?.hudDragging = true }
            if let origin = self.dragOrigin { self.place(at: CGPoint(x: origin.x + delta.x, y: origin.y + delta.y)) }
        }
        heading.finished = { [weak self] in self?.dragOrigin = nil; self?.health?.hudDragging = false; self?.refresh() }
        close.target = self; close.action = #selector(hide)
        details.target = self; details.action = #selector(openDetails)
        values.font = .monospacedDigitSystemFont(ofSize: 12, weight: .regular)
        values.maximumNumberOfLines = 4
        coverage.font = .systemFont(ofSize: 10)
        coverage.textColor = .secondaryLabelColor
        card.addSubview(values); card.addSubview(coverage)
        setAccessibilityCustomActions([
            NSAccessibilityCustomAction(name: "Reset position", target: self, selector: #selector(resetPosition)),
            NSAccessibilityCustomAction(name: "Move left", handler: { [weak self] in self?.nudge(x: -32, y: 0) ?? false }),
            NSAccessibilityCustomAction(name: "Move right", handler: { [weak self] in self?.nudge(x: 32, y: 0) ?? false }),
            NSAccessibilityCustomAction(name: "Move up", handler: { [weak self] in self?.nudge(x: 0, y: 32) ?? false }),
            NSAccessibilityCustomAction(name: "Move down", handler: { [weak self] in self?.nudge(x: 0, y: -32) ?? false })])
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override var isFlipped: Bool { false }
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        effectiveAppearance.performAsCurrentDrawingAppearance {
            card.layer?.backgroundColor = NSColor.controlBackgroundColor.cgColor
            card.layer?.borderColor = NSColor.separatorColor.cgColor
        }
    }
    func setVisible(_ visible: Bool) {
        desiredVisible = visible; isHidden = !visible
        health?.setHUDVisible(visible)
        if visible { health?.changed = { [weak self] in self?.refresh() }; refresh() }
        else { health?.changed = nil; health?.hudDragging = false }
        needsLayout = true
    }
    func refresh() {
        guard desiredVisible, dragOrigin == nil, let health else { return }
        values.stringValue = FrameHealthReadout.compact(health)
        needsLayout = true
    }
    override func layout() {
        super.layout()
        card.setFrameSize(CGSize(width: min(300, max(0, bounds.width - 16)), height: 146))
        if !placed { card.setFrameOrigin(CGPoint(x: bounds.minX + 8, y: bounds.maxY - 154)); placed = true }
        place(at: card.frame.origin)
        let width = card.bounds.width
        heading.frame = CGRect(x: 10, y: 110, width: max(0, width - 84), height: 32)
        details.frame = CGRect(x: width - 72, y: 110, width: 32, height: 32)
        close.frame = CGRect(x: width - 40, y: 110, width: 32, height: 32)
        values.frame = CGRect(x: 12, y: 28, width: max(0, width - 24), height: 80)
        coverage.frame = CGRect(x: 12, y: 8, width: max(0, width - 24), height: 16)
        card.isHidden = bounds.height < 162 || bounds.width < 160
        health?.hudEffectiveVisible = desiredVisible && !card.isHidden
        health?.hudBounds = card.frame
    }
    func place(at point: CGPoint) {
        card.setFrameOrigin(CGPoint(x: min(max(bounds.minX + 8, point.x), max(bounds.minX + 8, bounds.maxX - card.frame.width - 8)),
            y: min(max(bounds.minY + 8, point.y), max(bounds.minY + 8, bounds.maxY - card.frame.height - 8))))
        health?.hudBounds = card.frame
    }
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard desiredVisible, !card.isHidden else { return nil }
        let hit = super.hitTest(point)
        return [heading, close, details].contains { hit === $0 || hit?.isDescendant(of: $0) == true } ? hit : nil
    }
    private func nudge(x: CGFloat, y: CGFloat) -> Bool { place(at: CGPoint(x: card.frame.minX + x, y: card.frame.minY + y)); return true }
    @objc private func hide() { setVisible(false) }
    @objc private func openDetails() { showDetails?() }
    @objc private func resetPosition() -> Bool { placed = false; needsLayout = true; return true }
}

extension ExactView {
    @objc func toggleFrameHealth(_ sender: Any?) {
        guard let container = session.frameHealth.macContainer else { return }
        let health = session.frameHealth
        let overlay: FrameHealthOverlayMac
        if let existing = health.desktopOverlay, existing.superview === container { overlay = existing }
        else {
            health.desktopOverlay?.removeFromSuperview()
            overlay = FrameHealthOverlayMac(health: health)
            overlay.frame = container.bounds
            overlay.autoresizingMask = [.width, .height]
            container.addSubview(overlay)
            health.desktopOverlay = overlay
            health.ended = { [weak overlay] in overlay?.setVisible(false); overlay?.removeFromSuperview() }
            overlay.showDetails = { [weak self] in self?.showFrameHealthDetails(nil) }
        }
        overlay.setVisible(!health.hudVisible)
    }
    func validateFrameHealthItem(_ item: NSMenuItem) -> Bool {
        item.title = session.frameHealth.hudVisible ? "Hide Frame Health" : "Show Frame Health"
        return session.frameHealth.macContainer != nil
    }
    @objc func showFrameHealthDetails(_ sender: Any?) { FrameHealthInspector.show(health: session.frameHealth, window: window) }
    @objc func shareFrameHealth(_ sender: Any?) {
        FrameHealthReport.prepare(health: session.frameHealth.reportSource, session: session) { [weak self] data in
            guard let self else { return }
            NSSharingServicePicker(items: [String(decoding: data, as: UTF8.self)]).show(relativeTo: self.visibleRect, of: self, preferredEdge: .minY)
        }
    }
}
#endif
