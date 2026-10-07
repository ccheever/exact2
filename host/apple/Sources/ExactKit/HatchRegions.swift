// Hatch-owned regions and parts (@ref LLP 1075.003.000.001 §3.4, §3.5): what
// a hatch tells the agent it cannot otherwise see. A region is a sentence
// bound, weakly, to an object the hatch made (a view, a gesture recognizer),
// or to nothing for an appearance it set; the host then observes the object
// and `tree` and `state.hatches` show both halves, the declaration and what
// was observed. A part is a control a hatch drew, bound to its view, named
// under its node. Neither does anything when the agent names it: what a
// bound control does when touched is the hatch's own code.
//
// The host's entries (NativeHatches.swift):
//
//   80  owns(host, scope, scopeLen, node, kind, object, what, whatLen, flags) → 0 taken
//         kind 0 view, 1 recognizer, 2 appearance; flags bit 0 a hatch-only surface
//   88  parts(host, node, json, len, views, count) → 0 taken
//         json [{"id", "role", "label"}, …], one view each, replacing the node's parts
//
// Bounds (§3.2): 32 regions a scope instance and 1,024 a session, a sentence
// of 120 bytes, 16 appearance regions a session (the same sentence's key
// replaces), 32 parts a node with ids of 64 bytes, unique in the node, and
// labels of 120. Past one, the registration is refused by name, whole.
import Foundation
#if os(macOS)
import AppKit
typealias HatchRecognizer = NSGestureRecognizer
#else
import UIKit
typealias HatchRecognizer = UIGestureRecognizer
#endif

final class HatchRegions {
    weak var presenter: Presenter?
    static let perScope = 32, perSession = 1024, sentence = 120, appearances = 16
    static let partsPerNode = 32, partId = 64, partLabel = 120, tombstonesKept = 16, tombstoneMs = 5000.0

    final class Region {
        let scope: String, node: UInt32, kind: String, what: String, surface: Bool
        weak var object: AnyObject?
        init(scope: String, node: UInt32, kind: String, what: String, surface: Bool, object: AnyObject?) {
            self.scope = scope; self.node = node; self.kind = kind; self.what = what; self.surface = surface; self.object = object
        }
    }
    struct Part { let id: String, role: String, label: String; weak var view: PlatformView? }
    private struct Tombstone { let scope: String, node: UInt32, kind: String, what: String, ended: Double }

    private(set) var regions: [Region] = []
    private(set) var parts: [UInt32: [Part]] = [:]
    private var tombstones: [Tombstone] = []
    private(set) var rejected = 0

    init(_ presenter: Presenter) { self.presenter = presenter }

    private func refuse(_ scope: String, _ why: String) -> Bool {
        rejected += 1
        presenter?.session?.log("hatch \(scope): \(why)")
        return false
    }

    /// A hatch's `owns(…)`. False when refused.
    func owns(scope: String, node: UInt32, kind: UInt32, object: AnyObject?, what: String, surface: Bool) -> Bool {
        guard what.utf8.count <= Self.sentence, !what.isEmpty else { return refuse(scope, "owns refused: its sentence is \(what.utf8.count) bytes, over \(Self.sentence)") }
        let name = ["view", "recognizer", "appearance"][Int(min(kind, 2))]
        if kind == 2 {
            // Keyed by its sentence: saying it again replaces it.
            regions.removeAll { $0.kind == "appearance" && $0.what == what }
            guard regions.filter({ $0.kind == "appearance" }).count < Self.appearances else { return refuse(scope, "owns refused: \(Self.appearances) appearance regions a session") }
        } else {
            guard object != nil else { return refuse(scope, "owns refused: no \(name) was given") }
            regions.removeAll { $0.object === object }
        }
        guard regions.filter({ $0.scope == scope && $0.node == node }).count < Self.perScope else { return refuse(scope, "owns refused: \(Self.perScope) regions a scope") }
        guard regions.count < Self.perSession else { return refuse(scope, "owns refused: \(Self.perSession) regions a session") }
        regions.append(Region(scope: scope, node: node, kind: name, what: what, surface: surface, object: object))
        return true
    }

    /// A node's parts, replacing those it had. False, and they are left as
    /// they were, when the list breaks a bound.
    func setParts(node: UInt32, scope: String, _ list: [Part]) -> Bool {
        guard list.count <= Self.partsPerNode else { return refuse(scope, "parts refused: \(list.count) parts, over \(Self.partsPerNode) a node") }
        var ids = Set<String>()
        for part in list {
            guard !part.id.isEmpty, part.id.utf8.count <= Self.partId else { return refuse(scope, "parts refused: an id of \(part.id.utf8.count) bytes (1 to \(Self.partId))") }
            guard ids.insert(part.id).inserted else { return refuse(scope, "parts refused: `\(part.id)` is listed twice") }
            guard part.label.utf8.count <= Self.partLabel else { return refuse(scope, "parts refused: `\(part.id)`'s label is over \(Self.partLabel) bytes") }
        }
        parts[node] = list.isEmpty ? nil : list
        return true
    }

    /// A node's hatch ended, or its view is going to another node: what it
    /// registered goes with it.
    func ended(node: UInt32) {
        guard node != 0 else { return }
        regions.removeAll { $0.node == node }
        parts[node] = nil
    }

    /// A scope that is not a node ended (a route, the window, the app).
    func ended(scope: String) { regions.removeAll { $0.scope == scope && $0.node == 0 } }

    /// A reload or the session's end: every registration goes.
    func reset() { regions = []; parts = [:]; tombstones = [] }

    private func live(_ region: Region) -> Bool {
        guard region.kind != "appearance" else { return true }
        guard let object = region.object else { return false }
        if let view = object as? PlatformView { return view.window != nil }
        if let recognizer = object as? HatchRecognizer { return recognizer.view?.window != nil }
        return true
    }

    /// The host retires a region whose object is gone or out of the window:
    /// it stays as a tombstone for 5 s of session clock, at most 16 kept.
    /// Run as a batch begins and before a read; with nothing changed since,
    /// a second run changes nothing, so two reads agree.
    func sweep() {
        guard !regions.isEmpty || !tombstones.isEmpty, let now = presenter?.session?.now() else { return }
        for region in regions where !live(region) {
            tombstones.append(Tombstone(scope: region.scope, node: region.node, kind: region.kind, what: region.what, ended: now))
        }
        regions.removeAll { !live($0) }
        tombstones.removeAll { now - $0.ended >= Self.tombstoneMs }
        if tombstones.count > Self.tombstonesKept { tombstones.removeFirst(tombstones.count - Self.tombstonesKept) }
    }

    private func frame(_ view: PlatformView) -> [String: Any] { AgreementReport.rect(view.convert(view.bounds, to: nil)) }

    private func json(_ region: Region) -> [String: Any] {
        var observed: [String: Any] = ["live": true]
        if let view = region.object as? PlatformView {
            observed["frame"] = frame(view)
            observed["class"] = String(describing: type(of: view))
        } else if let recognizer = region.object as? HatchRecognizer {
            if let view = recognizer.view { observed["frame"] = frame(view) }
            observed["class"] = String(describing: type(of: recognizer))
        }
        var out: [String: Any] = ["by": region.scope, "kind": region.kind, "what": region.what, "observed": observed]
        if region.surface { out["surface"] = true }
        if region.node != 0 { out["node"] = Int(region.node) }
        return out
    }

    private func json(_ t: Tombstone) -> [String: Any] {
        var out: [String: Any] = ["by": t.scope, "kind": t.kind, "what": t.what, "observed": ["live": false, "ended": t.ended]]
        if t.node != 0 { out["node"] = Int(t.node) }
        return out
    }

    private func json(_ part: Part) -> [String: Any] {
        var out: [String: Any] = ["id": part.id, "role": part.role, "label": part.label, "live": part.view?.window != nil]
        if let view = part.view, view.window != nil { out["frame"] = frame(view) }
        return out
    }

    /// `tree`: each node's regions (with their tombstones) and parts, under it.
    func decorate(_ tree: [String: Any]) -> [String: Any] {
        guard !regions.isEmpty || !parts.isEmpty || !tombstones.isEmpty, var nodes = tree["nodes"] as? [[String: Any]] else { return tree }
        sweep()
        for index in nodes.indices {
            guard let id = (nodes[index]["id"] as? NSNumber)?.uint32Value else { continue }
            let owned = regions.filter { $0.node == id }.map(json) + tombstones.filter { $0.node == id }.map(json)
            if !owned.isEmpty { nodes[index]["owns"] = owned }
            if let list = parts[id] { nodes[index]["parts"] = list.map(json) }
        }
        var out = tree
        out["nodes"] = nodes
        return out
    }

    /// `state.hatches`: every region, whatever its scope, the hatch-only
    /// surfaces among them, and the parts by node.
    func state(into reply: inout [String: Any]) {
        sweep()
        guard !regions.isEmpty || !parts.isEmpty || !tombstones.isEmpty || rejected > 0 else { return }
        reply["owns"] = regions.map(json) + tombstones.map(json)
        reply["surfaces"] = regions.filter(\.surface).map { ["by": $0.scope, "what": $0.what] }
        reply["parts"] = Dictionary(uniqueKeysWithValues: parts.map { (String($0.key), $0.value.map(json)) })
        reply["regionsRefused"] = rejected
    }

    /// The region a view's interior belongs to (LLP 1080.001 D3 as amended):
    /// its innermost ancestor that a region binds, by identity.
    func owner(of view: PlatformView) -> String? {
        guard regions.contains(where: { $0.kind == "view" }) else { return nil }
        var at: PlatformView? = view
        while let v = at {
            if let region = regions.first(where: { $0.kind == "view" && $0.object === v }) { return region.what }
            at = v.superview
        }
        return nil
    }

    /// The part whose bound view `object` is, or lies inside: `<node>/<id>`.
    /// `tree --ax` joins a part by this ownership, never by a string (§3.5).
    func part(owning object: AnyObject) -> String? {
        guard !parts.isEmpty else { return nil }
        #if os(macOS)
        var at: PlatformView? = (object as? NSCell)?.controlView ?? (object as? PlatformView)
        #else
        var at: PlatformView? = object as? PlatformView
        #endif
        while let view = at {
            for (node, list) in parts { if let part = list.first(where: { $0.view === view }) { return "\(node)/\(part.id)" } }
            if view is NodeView { return nil }
            at = view.superview
        }
        return nil
    }

    /// Every part as `tree --ax`'s coverage lists it: exposed when one of the
    /// walk's elements joined it, `not exposed` otherwise.
    func coverage(joined: Set<String>) -> [[String: Any]] {
        parts.flatMap { node, list in list.map { ["node": Int(node), "id": $0.id, "ax": joined.contains("\(node)/\($0.id)") ? "exposed" : "not exposed"] as [String: Any] } }
            .sorted { ($0["node"] as? Int ?? 0, $0["id"] as? String ?? "") < ($1["node"] as? Int ?? 0, $1["id"] as? String ?? "") }
    }

    /// The live part `id` of `node`, and its view.
    func part(_ id: String, of node: UInt32) -> (Part, PlatformView)? {
        guard let part = parts[node]?.first(where: { $0.id == id }), let view = part.view else { return nil }
        return (part, view)
    }

    // MARK: Undeclared gestures (§3.4), a development check

    private var reported: Set<String> = []

    /// Recognizers on a hatched node's view that are neither Exact's own
    /// (`known`) nor bound by a region, journaled once a hatch and class:
    /// 8 levels below the view, 2,000 views a check.
    func undeclared(under root: PlatformView, by scope: String, known: (HatchRecognizer) -> Bool) {
        var stack: [(PlatformView, Int)] = [(root, 0)], walked = 0
        while let (view, depth) = stack.popLast() {
            walked += 1
            if walked > 2000 {
                if reported.insert("\(scope)\ntruncated").inserted { presenter?.session?.log("hatch \(scope): the undeclared-recognizer check was truncated at 2,000 views") }
                return
            }
            for recognizer in view.gestureRecognizers ?? [] where !known(recognizer) && !regions.contains(where: { $0.object === recognizer }) {
                let name = String(describing: type(of: recognizer))
                if reported.insert("\(scope)\n\(name)").inserted { presenter?.session?.log("hatch \(scope) added a \(name) it did not declare") }
            }
            if depth < 8 { for sub in view.subviews where !(sub is NodeView) { stack.append((sub, depth + 1)) } }
        }
    }
}
