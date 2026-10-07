#if os(macOS)
import AppKit

/// The actual transcript viewport, never an estimate from message lengths.
/// Only stable-geometry movement away from the end rests the composer; layout
/// changes, history insertion and initial scroll-follow adoption do not.
final class T3Timeline {
    private weak var element: ExactElement?
    private var observation: NSObjectProtocol?
    private var monitor: Any?
    private var gestureUntil: TimeInterval = 0
    private let changed: (String) -> Void
    private var owner = ""
    private var eligible = false
    private var previous: Sample?
    private var movement: CGFloat = 0
    private(set) var resting = false
    private(set) var atEnd = true
    /// r4-composer: wheel or key gestures toward the end made while already at
    /// it. A composer that grew over the transcript holds its new reservation
    /// back until the reader asks for more (r4-composer-overlay.ts).
    private(set) var pulls = 0
    private var lastPull: TimeInterval = 0
    private struct Sample {
        let top: CGFloat
        let height: CGFloat
        let content: CGFloat
        let width: CGFloat
    }
    init(changed: @escaping (String) -> Void) { self.changed = changed }

    func install(_ element: ExactElement) {
        guard element.hook == .t3Transcript else { return }
        let nextOwner = element.data[.timelineOwner] ?? ""
        let nextEligible = element.data[.timelineRest] == "yes"
        if self.element !== element {
            destroy()
            self.element = element
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.scrollWheel, .keyDown]) { [weak self] event in
                self?.handle(event)
                return event
            }
            if let scroll = element.scrollView {
                observation = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification,
                    object: scroll.contentView, queue: .main) { [weak self] _ in self?.observe() }
            }
        }
        if owner != nextOwner { owner = nextOwner; previous = nil; movement = 0; resting = false; gestureUntil = 0 }
        eligible = nextEligible
        if !eligible && resting { resting = false; changed("t3.status") }
        observe()
    }
    func remove(_ element: ExactElement) { if self.element === element { destroy() } }
    func destroy() {
        if let observation { NotificationCenter.default.removeObserver(observation) }
        if let monitor { NSEvent.removeMonitor(monitor) }
        observation = nil; monitor = nil; element = nil; previous = nil; movement = 0; gestureUntil = 0
    }
    var status: [String: Any] { ["owner": owner, "resting": resting, "atEnd": atEnd, "transcriptPulls": pulls] }

    // Programmatic scroll-follow/history restoration never arms this gate.
    // The agent's `tap … wheel` goes through NSApp.sendEvent since exact2 #186, so it reaches this
    // monitor at its point as a hand's wheel does.
    func handle(_ event: NSEvent, at now: TimeInterval = ProcessInfo.processInfo.systemUptime) {
        guard let scroll = element?.scrollView, let window = scroll.window,
              event.windowNumber == window.windowNumber else { return }
        let wheel = event.type == .scrollWheel && scroll.bounds.contains(scroll.convert(event.locationInWindow, from: nil))
        let responder = window.firstResponder as? NSView
        let key = event.type == .keyDown && [UInt16(115), 119, 116, 121, 125, 126, 49].contains(event.keyCode)
            && responder?.isDescendant(of: scroll) == true
        guard wheel || key else { return }
        let towardEnd = wheel ? event.scrollingDeltaY < 0 : [UInt16(119), 121, 125, 49].contains(event.keyCode)
        if towardEnd, atEnd, now - lastPull > 0.5 { lastPull = now; pulls += 1; changed("t3.status") }
        guard eligible else { return }
        gestureUntil = now + 0.8
    }

    private func observe() {
        guard let scroll = element?.scrollView, let document = scroll.documentView else { return }
        let sample = Sample(top: scroll.contentView.bounds.minY, height: scroll.contentView.bounds.height,
            content: document.frame.height, width: scroll.contentView.bounds.width)
        let end = sample.content <= sample.height + 2 || sample.top >= sample.content - sample.height - 2
        let oldEnd = atEnd, oldResting = resting
        if end { resting = false; movement = 0 }
        else if eligible, ProcessInfo.processInfo.systemUptime <= gestureUntil, let previous,
                abs(previous.height - sample.height) < 0.5,
                abs(previous.content - sample.content) < 0.5,
                abs(previous.width - sample.width) < 0.5 {
            movement += abs(sample.top - previous.top)
            if movement >= 24 { resting = true }
        } else { movement = 0; gestureUntil = 0 }
        previous = sample; atEnd = end
        if oldEnd != atEnd || oldResting != resting { changed("t3.status") }
    }
}
#endif
