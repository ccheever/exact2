#if os(macOS)
import AppKit

/// An assistant citation's jump target (AssistantCitationSource): the
/// transcript and its answers (`hatch="t3-turn"`, `data-turn="cite:<id>"`), the
/// answer brought min(120pt, a third of the viewport) under the top edge and kept
/// there while the rows around it settle. An answer the virtualized list has not
/// mounted is first approached by its row position, then placed exactly once its
/// row exists. The minimap's turns on screen and its jumps are Contract's
/// (`intersect=` and keyed `scrollIntoView` in app-main.contract).
final class T3TimelineTurns {
    private weak var transcript: ExactElement?
    private var rows: [String: Weak] = [:]
    private struct Weak { weak var element: ExactElement? }
    /// Each jump's number: a newer jump or `cancel()` (a minimap jump, Contract's `scrollIntoView`) stops older retries.
    private var placement = 0

    func install(_ element: ExactElement) {
        if element.hatch == .t3Transcript { transcript = element }
        if element.hatch == .t3Turn, let id = element.data[.turn], !id.isEmpty {
            rows = rows.filter { $0.value.element != nil && $0.value.element !== element }
            rows[id] = Weak(element: element)
        }
    }
    func remove(_ element: ExactElement) {
        if element === transcript { destroy(); return }
        if element.hatch == .t3Turn { rows = rows.filter { $0.value.element != nil && $0.value.element !== element } }
    }
    func destroy() {
        cancel()
        transcript = nil
    }
    /// Ends the placement in progress and its relayout hold, so another writer can move the transcript.
    func cancel() { placement += 1; release() }
    /// A row's rect in the clip view's coordinates, the space its bounds scroll in.
    private func frame(of element: ExactElement, in scroll: NSScrollView) -> NSRect? {
        guard let view = element.view, view.window != nil, let document = scroll.documentView, view.isDescendant(of: document) else { return nil }
        return view.convert(view.bounds, to: scroll.contentView)
    }

    /// A short wait for TypeScript flows that watch the projection settle
    /// (Edit from here waits for its rollback's outcome); at most two seconds.
    static func sleep(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let delay = min(2.0, max(0.01, (request["ms"] as? Double ?? 200) / 1000))
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { reply(["ok": true, "generation": generation, "value": ["slept": delay]]) }
    }

    /// For two seconds after a jump lands, a relayout that moves the
    /// target (rows above it measured as they mount, a page of history) places it
    /// again, as AssistantCitationSource re-checks on size and position changes.
    private var holding: [NSObjectProtocol] = []
    private func release() { holding.forEach(NotificationCenter.default.removeObserver); holding = [] }
    private func hold(id: String, scroll: NSScrollView, document: NSView, lead: @escaping (NSClipView) -> CGFloat) {
        release()
        let until = ProcessInfo.processInfo.systemUptime + 2
        document.postsFrameChangedNotifications = true
        let replace: (Notification) -> Void = { [weak self, weak scroll] _ in
            guard let self, let scroll, ProcessInfo.processInfo.systemUptime < until else { self?.release(); return }
            guard let element = self.rows[id]?.element, let rect = self.frame(of: element, in: scroll) else { return }
            let clip = scroll.contentView, content = document.convert(document.bounds, to: clip)
            let maxY = max(content.minY, content.maxY - clip.bounds.height)
            let target = min(max(content.minY, clip.isFlipped ? rect.minY - lead(clip) : rect.maxY + lead(clip) - clip.bounds.height), maxY)
            guard abs(clip.bounds.minY - target) > 0.5 else { return }
            clip.setBoundsOrigin(NSPoint(x: clip.bounds.minX, y: target))
            scroll.reflectScrolledClipView(clip)
        }
        holding.append(NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: document, queue: .main, using: replace))
        if let view = rows[id]?.element?.view {
            view.postsFrameChangedNotifications = true
            holding.append(NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: view, queue: .main, using: replace))
        }
    }

    /// Scrolls the transcript so the cited answer lands at its lead (24pt for any other row).
    func jump(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let id = request["id"] as? String ?? ""
        let index = request["index"] as? Int ?? 0, count = max(1, request["count"] as? Int ?? 1)
        guard let scroll = transcript?.scrollView, let document = scroll.documentView, !id.isEmpty else {
            return reply(["ok": false, "generation": generation, "error": ["kind": "Timeline", "message": "The transcript is not on screen.", "uncertain": false]])
        }
        // A citation's source lands min(120pt, a third of the viewport) under the top edge
        // (AssistantCitationSource), its `inset` the distance from the hooked row to the cited content.
        let inset = CGFloat(request["inset"] as? Double ?? 0)
        let lead: (NSClipView) -> CGFloat = request["lead"] as? String == "citation" ? { clip in min(120, clip.bounds.height / 3) - inset } : { _ in 24 }
        cancel()
        place(id: id, index: index, count: count, scroll: scroll, document: document, attempt: 0, lead: lead, placement: placement)
        reply(["ok": true, "generation": generation, "value": ["id": id]])
    }
    private func place(id: String, index: Int, count: Int, scroll: NSScrollView, document: NSView, attempt: Int, lead: @escaping (NSClipView) -> CGFloat = { _ in 24 }, placement: Int) {
        guard placement == self.placement else { return }
        let clip = scroll.contentView, flipped = clip.isFlipped
        let content = document.convert(document.bounds, to: clip)
        let minY = content.minY, maxY = max(content.minY, content.maxY - clip.bounds.height)
        var target: CGFloat
        let exact: Bool
        if let element = rows[id]?.element, let rect = frame(of: element, in: scroll) {
            target = flipped ? rect.minY - lead(clip) : rect.maxY + lead(clip) - clip.bounds.height
            exact = true
        } else {
            let fraction = CGFloat(index) / CGFloat(max(1, count - 1))
            target = flipped ? content.minY + fraction * content.height : content.maxY - fraction * content.height - clip.bounds.height
            exact = false
        }
        target = min(max(minY, target), maxY)
        let origin = NSPoint(x: clip.bounds.minX, y: target)
        let again = { [weak self] () -> Void in
            self?.place(id: id, index: index, count: count, scroll: scroll, document: document, attempt: attempt + 1, lead: lead, placement: placement)
        }
        if exact && attempt == 0 {
            // Rows the scroll brings into view are measured as they mount, which moves the
            // target; once the animation ends the row is placed again until it holds (attempts 1–8).
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.25
                clip.animator().setBoundsOrigin(origin)
            }, completionHandler: { [weak self] in
                guard placement == self?.placement else { return }
                scroll.reflectScrolledClipView(clip)
                self?.hold(id: id, scroll: scroll, document: document, lead: lead)
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: again)
            })
            return
        }
        let moved = abs(clip.bounds.minY - target) > 0.5
        if moved {
            clip.setBoundsOrigin(origin)
            scroll.reflectScrolledClipView(clip)
        }
        if exact && (!moved || attempt >= 8) { if attempt == 1 || !moved { hold(id: id, scroll: scroll, document: document, lead: lead) }; return }
        guard attempt < 8 else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: again)
    }
}
#endif
