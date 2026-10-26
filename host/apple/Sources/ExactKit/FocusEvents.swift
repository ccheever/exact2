// DOM's bubbling focus transition, shared by AppKit and UIKit. Native first
// responders notify the presenter even when only an ancestor listens.
#if os(macOS) || os(iOS) || os(tvOS)
#if os(macOS)
import AppKit
private typealias FocusPlatformView = NSView
#else
import UIKit
private typealias FocusPlatformView = UIView
#endif

private struct FocusSnapshot {
    let node: UInt32
    let name: String
    let path: [UInt32]
    let inbound: [UInt32]
    let outbound: [UInt32]
}

extension Presenter {
    /// A bubbling `focusin` or `focusout` at a listening ancestor (#283).
    var onFocusEvent: ((UInt32, String, String, String?) -> Void)? {
        get { focusEvents.onEvent }
        set { focusEvents.onEvent = newValue }
    }
    func focus(_ id: UInt32) {
        #if os(macOS)
        tabStart = nil
        #endif
        focusEvents.focus(id)
        #if os(iOS)
        session?.view?.setNeedsLayout()
        #endif
        MainActor.assumeIsolated { HeldState.focusMoved(to: textHost(id)) } // the focus coming into a held focus's node or leaving it lets it go (HeldState.swift)
    }
    func blur(_ id: UInt32) {
        #if os(iOS) || os(tvOS)
        if menus.preservesFocus(id) { return }
        #endif
        focusEvents.blur(id)
        #if os(iOS)
        session?.view?.setNeedsLayout()
        #endif
        // (After the turn: the focus that comes next, so one moving within the node keeps its hold.)
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated { HeldState.focusMoved(to: self?.focusedNode) }
            #if os(iOS)
            self?.session?.view?.syncKeyboardFocus()
            #endif
        }
    }
}

final class FocusEvents {
    private weak var presenter: Presenter?
    var onEvent: ((UInt32, String, String, String?) -> Void)?
    private var current: FocusSnapshot?
    private var pending: FocusSnapshot?

    init(_ presenter: Presenter) { self.presenter = presenter }
    func reset() { current = nil; pending = nil }

    private func snapshot(_ id: UInt32) -> FocusSnapshot? {
        guard let presenter, let view = presenter.textHost(id) else { return nil }
        var path: [UInt32] = [], inbound: [UInt32] = [], outbound: [UInt32] = []
        var run = presenter.inlineText(id)
        while let current = run {
            path.append(current.id)
            if current.handlers.contains("focusin") { inbound.append(current.id) }
            if current.handlers.contains("focusout") { outbound.append(current.id) }
            run = presenter.inlineText(current.parent)
        }
        var cursor: FocusPlatformView? = view
        while let next = cursor {
            if let node = next as? NodeView {
                path.append(node.id)
                if node.handlers.contains("focusin") { inbound.append(node.id) }
                if node.handlers.contains("focusout") { outbound.append(node.id) }
            }
            // Its authored parent, across a native container's view (LLP 1123 D2)
            cursor = presenter.eventParent(of: next)
        }
        return FocusSnapshot(node: id, name: (presenter.inlineText(id)?.props ?? view.props)["id"] ?? "", path: path, inbound: inbound, outbound: outbound)
    }

    private func named(_ state: FocusSnapshot) -> FocusSnapshot {
        guard let presenter, let view = presenter.textHost(state.node) else { return state }
        return FocusSnapshot(node: state.node, name: (presenter.inlineText(state.node)?.props ?? view.props)["id"] ?? "", path: state.path,
                             inbound: state.inbound, outbound: state.outbound)
    }

    private func emit(_ state: FocusSnapshot, entering: Bool, related: String?, afterBlur: Bool = false) {
        guard let presenter else { return }
        let kind = entering ? "focusin" : "focusout"
        let path = entering ? state.inbound : state.outbound
        presenter.afterBatch { [weak presenter] in
            // (A focus a reset moves, a sheet it dismisses giving it back, is the old tree's: never the runtime replacing it.)
            guard let presenter, !presenter.resetting else { return }
            let enclosingStopped = presenter.propagationStopped
            defer { presenter.propagationStopped = enclosingStopped }
            presenter.propagationStopped = false
            // A blur may detach its subtree before focusout starts. Its old
            // connected ancestors are no longer on that event's path.
            let detached = afterBlur && presenter.textHost(state.node) == nil
            let detachedPath = detached ? Set(state.path.prefix { presenter.textHost($0) == nil }) : []
            for id in path where detached ? detachedPath.contains(id) : presenter.textHost(id) != nil {
                presenter.onFocusEvent?(id, kind, state.name, related)
                if presenter.propagationStopped { break }
            }
        }
    }

    private func direct(_ id: UInt32, entering: Bool) {
        presenter?.afterBatch { [weak presenter] in
            guard let presenter, !presenter.resetting,
                  (presenter.inlineText(id)?.handlers ?? presenter.views[id]?.handlers ?? []).contains(entering ? "focus" : "blur") else { return }
            if entering { presenter.onFocus?(id) } else { presenter.onBlur?(id) }
        }
    }

    // (Nothing a reset moves is kept: the old tree's focus, never the replacing tree's to blur or skip.)
    func focus(_ id: UInt32) {
        guard presenter?.resetting != true, let next = snapshot(id) else { return }
        if current?.node == id, pending == nil { return }
        let old = (pending ?? current).map { named($0) }
        pending = nil
        current = next
        // A node blurred and focused again had no other element on either side.
        let other = old?.node == id ? nil : old
        if let old { emit(old, entering: false, related: other == nil ? nil : next.name, afterBlur: true) }
        direct(id, entering: true)
        emit(next, entering: true, related: other?.name)
    }

    func blur(_ id: UInt32) {
        guard presenter?.resetting != true, current?.node == id, let old = current else { return }
        current = nil
        pending = named(old)
        direct(id, entering: false)
        DispatchQueue.main.async { [weak self] in self?.flush() }
    }

    func removed(_ id: UInt32) {
        // Blur already began this transition. Wait for its related responder,
        // or flush, even if its handler removes the old views in the meantime.
        if pending?.path.contains(id) == true { return }
        guard presenter?.resetting != true, let old = current, old.path.contains(id) else { return }
        current = nil
        pending = nil
        emit(named(old), entering: false, related: nil)
    }

    private func flush() {
        guard let old = pending else { return }
        pending = nil
        emit(named(old), entering: false, related: nil, afterBlur: true)
    }
}
#endif
