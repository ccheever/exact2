// @ref LLP 1075.003.000 — nodes the Contract marks `hook="word"`: the app
// module's `element` hook receives each one's view and the platform object
// of its kind, after the batch that mounts it (`built`), when its `data-*`
// words change (`changed`), and before its view leaves (`ended`); a cold
// launch's are replayed once the module connects. A hooked node leaves the
// fast path, and this says so: on iOS it is never a flat leaf and its list
// row is never parked (FlatLeavesIOS, NodePoolIOS, by its `hook` prop),
// unless its hook made it `reusable` (LLP 1075.003.000.000 §8); the journal names what
// each word gave up, once, and warns once for a word in a list's row;
// `state.hooks` counts them. Shared by the AppKit and UIKit presenters; the
// module side is ExactNativeModule.swift, the call NativeHooks.swift.
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

final class ElementHooks {
    unowned let presenter: Presenter
    private struct Entry {
        let node: NodeView
        var told: Bool
        var data: String
        let inList: Bool
    }
    private var nodes: [UInt32: Entry] = [:]
    /// The words the journal has named, those it has warned for, and those
    /// it has said a reusable hook frees.
    private var said: Set<String> = [], warned: Set<String> = [], freed: Set<String> = []
    /// Calls by word and moment, since launch.
    private var calls: [String: [String: Int]] = [:]

    init(_ presenter: Presenter) { self.presenter = presenter }

    /// What a hooked node gives up on this host: on iOS its flat leaf, and
    /// its row's reuse unless its hook is `reusable`.
    static func lost(reusable: Bool) -> [String] {
        #if os(iOS)
        reusable ? ["flat"] : ["flat", "pool"]
        #else
        []
        #endif
    }

    // MARK: The moments

    /// A node the batch created: told once the batch is applied.
    func created(_ node: NodeView) {
        guard node.props["hook"] != nil else { return }
        nodes[node.id] = Entry(node: node, told: false, data: node.props["dataset"] ?? "", inList: false)
        presenter.afterBatch { [weak self, weak node] in if let self, let node { self.build(node) } }
    }

    private func build(_ node: NodeView) {
        guard let known = nodes[node.id], known.node === node, !known.told, presenter.views[node.id] === node,
              let natives = presenter.session?.natives, natives.hooksConnected, let word = node.props["hook"] else { return }
        let entry = Entry(node: node, told: true, data: known.data, inList: Self.list(holding: node) != nil)
        nodes[node.id] = entry
        let reusable = call(.built, entry)
        say(word, node: node, inList: entry.inList, reusable: reusable)
    }

    /// A hooked node's props changed: its hook hears new `data-*` words.
    func propsChanged(_ id: UInt32) {
        guard var entry = nodes[id], entry.node.props["dataset"] ?? "" != entry.data else { return }
        entry.data = entry.node.props["dataset"] ?? ""
        nodes[id] = entry
        guard entry.told else { return }
        presenter.afterBatch { [weak self] in
            guard let self, let now = self.nodes[id], now.node === entry.node, let word = now.node.props["hook"] else { return }
            let reusable = self.call(.changed, now)
            self.say(word, node: now.node, inList: now.inList, reusable: reusable)
        }
    }

    /// A held heavy leaf (iOS: a video, iframe or native view in a list) is
    /// made after its hook heard `built`: the hook hears `changed`, its
    /// platform object there now.
    func realized(_ node: NodeView) {
        guard let entry = nodes[node.id], entry.node === node, entry.told else { return }
        call(.changed, entry)
    }

    /// Before a batch's ops: every hooked node it destroys ends now, its view
    /// still in its row. A row's root is destroyed before its children, and
    /// the node pool decides at the root whether the row parks, so a
    /// `reusable` hook must have undone its additions by then (iOS).
    func begin(_ batch: Batch) {
        guard !nodes.isEmpty else { return }
        for op in batch.ops where op.op == .destroy { destroyed(op.id) }
    }

    /// A node the batch destroys: its hook hears `ended` while its view is
    /// still there.
    func destroyed(_ id: UInt32) {
        guard let entry = nodes.removeValue(forKey: id), entry.told else { return }
        call(.ended, entry)
    }

    /// Every hooked node is leaving (a reload, the session's end).
    func reset() {
        for id in nodes.keys.sorted() { destroyed(id) }
        nodes = [:]
    }

    /// The module connected after these were built (LLP 1075.003 Q3 (c)).
    func replay() {
        for id in nodes.keys.sorted() { if let entry = nodes[id], !entry.told { build(entry.node) } }
    }

    /// The call, and whether the hook made the node `reusable` (its view
    /// carries the answer to the node pool, LLP 1075.003.000.000 §8).
    @discardableResult
    private func call(_ event: RouteHookEvent, _ entry: Entry) -> Bool {
        let id = entry.node.id
        let word = entry.node.props["hook"] ?? ""
        calls[word, default: [:]][event.name, default: 0] += 1
        // In a list's row, the first of each moment is journaled; `state`
        // counts the rest, so a fling does not flood the journal.
        let quiet = entry.inList && (calls[word]?[event.name] ?? 0) > 1
        let reusable = presenter.session?.natives.elementHook(entry.node, event: event.rawValue, platform: Self.platform(of: entry.node, presenter), quiet: quiet) ?? false
        #if os(iOS)
        // The answer is the view's while it is still this node's: a click
        // inside the hook (after the batch) can have replaced the node and
        // given its view to another. At `ended` it is the last word.
        if event == .ended || (nodes[id]?.node === entry.node && presenter.views[id] === entry.node) {
            entry.node.hookReusable = reusable
        }
        #endif
        return reusable
    }

    // MARK: Saying so (LLP 1075.003.000 §3.5)

    private func say(_ word: String, node: NodeView, inList: Bool, reusable: Bool) {
        let lost = Self.lost(reusable: reusable)
        if said.insert(word).inserted {
            let gave = lost.isEmpty ? "nothing beyond the call on this host"
                : lost.contains("pool") ? "a view, not a flat leaf; its row is not reused"
                : "a view, not a flat leaf; its row is reused, its hook undoing what it adds"
            presenter.session?.log("hook element \(word): \(gave) (LLP 1075.003.000)")
            if !lost.isEmpty, !lost.contains("pool") { freed.insert(word) }
        } else if !lost.isEmpty, !lost.contains("pool"), freed.insert(word).inserted {
            presenter.session?.log("hook element \(word): its hook undoes what it adds; its row is reused (LLP 1075.003.000.000 §8)")
        }
        if inList, warned.insert(word).inserted, let list = Self.list(holding: node) {
            let name = list.props["testId"].map { "list \($0)" } ?? "list #\(list.id)"
            presenter.session?.log("hook element \(word) is in a row of \(name): each row's mount calls its hook on the main thread"
                + (lost.contains("pool") ? ", and its row is never reused" : ""))
        }
    }

    /// The virtualized list whose row holds `node`, if any.
    private static func list(holding node: NodeView) -> NodeView? {
        var at = node.superview
        while let view = at {
            if let n = view as? NodeView, n.kind == "list" { return n }
            at = view.superview
        }
        return nil
    }

    /// `state.hooks`: per word, how many are live, what they gave up, and
    /// the calls so far.
    var observation: [String: Any] {
        var live: [String: Int] = [:], reused: [String: Int] = [:]
        for entry in nodes.values {
            let word = entry.node.props["hook"] ?? ""
            live[word, default: 0] += 1
            #if os(iOS)
            if entry.node.hookReusable { reused[word, default: 0] += 1 }
            #endif
        }
        var out: [String: Any] = [:]
        for word in Set(live.keys).union(calls.keys) {
            // A word whose live nodes are all reusable gives up only its flat leaf.
            let reusable = (reused[word] ?? 0) > 0 && reused[word] == live[word]
            out[word] = ["live": live[word] ?? 0, "reusable": reused[word] ?? 0, "lost": Self.lost(reusable: reusable),
                         "calls": calls[word] ?? [:]] as [String: Any]
        }
        return out
    }

    #if os(iOS)
    private var reported: Set<String> = []
    /// Before each batch, in a development build (LLP 1075.003 §3.5,
    /// 1075.003.000 §3.6): what Exact owns of each hooked node's scroll view,
    /// text field or text view against what it writes, journaled once.
    func checkOwned() {
        guard NavigationHost.checksOwnership, !nodes.isEmpty, presenter.session?.natives.hooksConnected == true else { return }
        for entry in nodes.values where entry.told {
            let node = entry.node
            var changed: [String] = []
            if let sv = node.scroll { changed += NavigationHost.ownedChanges(sv, of: node, collapsing: node.scrollOrigin > 0) }
            if let field = node.field, field.delegate !== node { changed.append("the text field's delegate") }
            if let text = node.textArea, text.delegate !== node { changed.append("the text view's delegate") }
            for property in changed {
                let line = "element \(node.props["hook"] ?? "") #\(node.id): \(property) changed outside Exact, which owns it"
                if reported.insert(line).inserted { presenter.session?.log(line) }
            }
        }
    }
    #endif

    // MARK: The node's platform object (§3.2)

    /// The platform object of the node's kind, or nil: a text field or view,
    /// a control (a switch, slider, date picker…), a web view, a scroll view.
    static func platform(of node: NodeView, _ presenter: Presenter) -> AnyObject? {
        #if os(iOS)
        node.field ?? node.textArea ?? presenter.controls.controls[node.id] ?? presenter.segments.control(of: node.id) ?? node.web ?? node.scroll
        #else
        node.field ?? node.textArea ?? presenter.controls.controls[node.id] ?? presenter.segments.control(of: node.id) ?? node.scroll
        #endif
    }

    // MARK: Acting on an element (LLP 1075.003 §3.4)

    /// What a hook asks of an element runs on the main queue's next turn:
    /// after the batch being applied and after the session has finished with
    /// it (its timers and frame requests), never inside either, so a hook
    /// called from a batch or a reset never applies a batch of its own there.
    static func later(_ work: @escaping () -> Void) { DispatchQueue.main.async(execute: work) }

    /// A hook's `click()`, `focus()` or `blur()` on a node, as the DOM's, on
    /// the next turn (`later`). False when refused.
    func act(_ id: UInt32, _ action: UInt32) -> Bool {
        #if os(iOS)
        return presenter.navigation.act(id, action)
        #else
        guard let node = presenter.views[id] else { return false }
        switch action {
        case 0:
            guard node.handlers.contains("press"), !node.disabled else { return false }
            Self.later { [weak presenter = self.presenter, weak node] in
                if let presenter, let node, presenter.views[id] === node { presenter.press(id) }
            }
        case 1, 2:
            Self.later { [weak presenter = self.presenter, weak node] in
                guard let presenter, let node, presenter.views[id] === node, let window = node.window else { return }
                let responder: NSView = node.textArea ?? node.field ?? node
                if action == 1 {
                    if responder.acceptsFirstResponder { window.makeFirstResponder(responder) }
                } else if window.firstResponder === responder || window.firstResponder === node.field?.currentEditor() {
                    window.makeFirstResponder(nil)
                }
            }
        default: return false
        }
        return true
        #endif
    }
}
