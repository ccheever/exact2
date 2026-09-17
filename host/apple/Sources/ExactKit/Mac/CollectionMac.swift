// AppKit collection observations are independent of authored event handlers.
#if os(macOS)
import AppKit

extension CollectionHost {
    func pointerDown(_ view: UInt32?, event: NSEvent) {
        guard contactEvent !== event else { return }
        contactEvent = event
        pointer(view)
    }

    func geometry(_ id: UInt32) -> CollectionFacts? {
        guard let node = presenter?.views[id], let scroll = node.scroll,
              !node.isHiddenOrHasHiddenAncestor else { return nil }
        let bounds = scroll.contentView.bounds
        let content = node.contentBox()
        let left = content.minX, right = node.bounds.width - content.maxX
        let available = max(0, bounds.width - left - right)
        let width = entries[id]?.snapshot.rows.first.flatMap { rowWidth($0.view) }.map { CGFloat($0) } ?? available
        guard bounds.width.isFinite, bounds.height.isFinite, width.isFinite else { return nil }
        return CollectionFacts(top: Double(max(0, bounds.minY - content.minY)),
            portWidth: Double(max(0, bounds.width)), portHeight: Double(max(0, bounds.height)),
            rowWidth: Double(width), measurements: [], focus: nil, interaction: nil)
    }
    func rowWidth(_ id: UInt32) -> Double? {
        presenter?.views[id].map { Double($0.bounds.width) }
    }
    func height(_ id: UInt32) -> Double? {
        guard let node = presenter?.views[id], !node.isHiddenOrHasHiddenAncestor else { return nil }
        return Double(node.bounds.height)
    }
    func correct(_ id: UInt32, top: Double, extent: Double) {
        guard let node = presenter?.views[id], let scroll = node.scroll,
              let document = scroll.documentView else { return }
        let content = node.contentBox()
        let bottom = node.bounds.height - content.maxY
        let height = max(scroll.contentView.bounds.height, CGFloat(extent) + content.minY + bottom)
        // Preserve the measured kernel extent when it already contains more
        // than the provisional index (a newly measured row can be taller).
        let size = NSSize(width: document.frame.width, height: max(height, node.content.height))
        if document.frame.size != size { document.setFrameSize(size) }
        let maximum = max(0, document.frame.height - scroll.contentView.bounds.height)
        let target = NSPoint(x: scroll.contentView.bounds.minX,
            y: min(maximum, max(0, CGFloat(top) + content.minY)))
        if scroll.contentView.bounds.origin != target {
            scroll.contentView.scroll(to: target)
            scroll.reflectScrolledClipView(scroll.contentView)
        }
    }
    func focusedView() -> UInt32? {
        guard let presenter, let responder = presenter.viewport.window?.firstResponder else { return nil }
        for node in presenter.views.values {
            if responder === node || responder === node.textArea ||
                (node.field.flatMap { $0.currentEditor() }.map { responder === $0 } ?? false) { return node.id }
        }
        return nil
    }
    private func nodeID(_ hit: NSView?) -> UInt32? {
        var view = hit
        while let current = view {
            if let node = current as? NodeView, presenter?.views[node.id] === node { return node.id }
            view = current.superview
        }
        return nil
    }
    func startTracking() {
        let monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseUp,
            .rightMouseDown, .rightMouseUp, .otherMouseDown, .otherMouseUp, .keyDown]) { [weak self] event in
            guard let self, let presenter, event.window === presenter.viewport.window else { return event }
            switch event.type {
            case .leftMouseDown, .rightMouseDown, .otherMouseDown:
                let point = presenter.viewport.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow
                pointerDown(nodeID(presenter.viewport.hitTest(point)), event: event)
            case .leftMouseUp, .rightMouseUp, .otherMouseUp:
                // Release after AppKit delivers this event and any authored
                // handler. Captured generation is checked by the weak helper.
                releaseInteractionLater()
            case .keyDown:
                if event.keyCode == 53 { pointer(nil) }
            default: break
            }
            return event
        }
        let observer = NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification,
            object: nil, queue: .main) { [weak self] notification in
                guard let self, let window = notification.object as? NSWindow,
                      window === presenter?.viewport.window else { return }
                pointer(nil)
            }
        stopTracking = {
            if let monitor { NSEvent.removeMonitor(monitor) }
            NotificationCenter.default.removeObserver(observer)
        }
    }
}
#endif
