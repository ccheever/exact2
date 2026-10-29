#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    private var needsContextRecognizer: Bool {
        #if os(iOS)
        handlers.contains("contextmenu") || hasInlineContextMenu
        #else
        handlers.contains("contextmenu")
        #endif
    }
    func updateContextGestures() {
        if needsContextRecognizer, contextRecognizer == nil {
            let g = UILongPressGestureRecognizer(target: self, action: #selector(openContext(_:)))
            g.delegate = self
            // Recognition and scroll arbitration remain UIKit's.
            g.delaysTouchesEnded = false
            addGestureRecognizer(g)
            contextRecognizer = g
        }
        if !needsContextRecognizer, let g = contextRecognizer {
            removeGestureRecognizer(g)
            contextRecognizer = nil
        }
        if handlers.contains("dblclick"), doubleRecognizer == nil {
            let g = UITapGestureRecognizer(target: self, action: #selector(doubleClicked(_:)))
            g.delegate = self
            // The web's order (LLP 1057.001 §1 rule 5): the second tap still presses.
            g.numberOfTapsRequired = 2; g.delaysTouchesEnded = false; g.cancelsTouchesInView = false
            addGestureRecognizer(g)
            doubleRecognizer = g
        }
        if !handlers.contains("dblclick"), let g = doubleRecognizer {
            removeGestureRecognizer(g)
            doubleRecognizer = nil
        }
        syncPointerRecognizer()
    }
    @objc func openContext(_ gesture: UILongPressGestureRecognizer) {
        guard gesture.state == .began, !formDisabled else { return }
        let point = gesture.location(in: self)
        #if os(iOS)
        let run = inlineContextTarget(at: point)
        guard run != nil || hasOwnContextMenu else { return }
        deliverContext(at: point, inline: run?.id)
        presenter?.menus.agentContext(self, inline: run?.id)
        #else
        deliverContext(at: point)
        presenter?.menus.agentContext(self)
        #endif
    }

    @objc func doubleClicked(_ gesture: UITapGestureRecognizer) {
        guard gesture.state == .ended, !disabled else { return } // then after this touch's own press
        DispatchQueue.main.async { [weak self, token = incarnation] in if let self, self.incarnation == token, !self.disabled, self.presenter?.views[self.id] === self { self.presenter?.dblclick(self.id) } }
    }

    #if os(iOS)
    var hasOwnContextMenu: Bool { handlers.contains("contextmenu") || props["contextPopover"]?.isEmpty == false }
    var hasInlineContextMenu: Bool { inlineText.contains { $0.handlers.contains("contextmenu") || $0.props["contextPopover"]?.isEmpty == false } }
    var hasInlinePlainContextMenu: Bool { inlineText.contains { $0.handlers.contains("contextmenu") && $0.props["contextPopover"]?.isEmpty != false } }
    func inlineContextTarget(at point: CGPoint) -> InlineText? {
        var run = inlineTarget(at: point)
        while let current = run {
            guard presenter?.inlineInert(current.id) == false else { return nil }
            if current.handlers.contains("contextmenu") || current.props["contextPopover"]?.isEmpty == false { return current }
            run = presenter?.inlineText(current.parent)
        }
        return nil
    }

    #endif

    func deliverContext(at point: CGPoint, inline inlineID: UInt32? = nil) {
        guard let presenter else { return }
        if let inlineID {
            guard presenter.textHost(inlineID) === self, !presenter.inlineInert(inlineID) else { return }
        }
        inlinePressed = nil; linkPressed = nil; pressed = false
        let target = inlineID ?? id
        let run = inlineID.flatMap(presenter.inlineText)
        guard run.map({ $0.handlers.contains("contextmenu") }) ?? handlers.contains("contextmenu") else { return }
        let box = run.flatMap { inlineRects($0).first } ?? contentBox()
        let client = presenter.client(convert(point, to: nil))
        let sample = PointerSample(x: Double(point.x - box.minX), y: Double(point.y - box.minY), buttons: 1, pressure: 0.5, type: "touch", id: 2,
                                   clientX: Double(client.x), clientY: Double(client.y))
        presenter.contextmenu(target, line: sample.line)
    }
}
#endif
