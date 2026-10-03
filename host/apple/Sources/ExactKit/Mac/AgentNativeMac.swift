// @ref LLP 1080.001 — the AppKit walks behind `layout <target> native` and
// `layout agree`, as UIKit's (`AgentNativeIOS.swift`): ownership by
// identity, strays judged only in a subview list Exact owns. AppKit has no
// node pool and no flat leaves; its window is shared (two sessions of the
// sample host, sheets of unproven ownership), so the walk is the session's
// `ExactView` alone. `interactive` is absent: AppKit has no flag of its own,
// and `NodeView.inert` folds route and dialog policy (LLP 1080.001 §0 4).
#if os(macOS)
import AppKit

extension Agent {
    func nativeSubviews(_ id: UInt32, depth: Int, limit: Int) -> [String: Any] {
        let runner = runnerNode(id)
        if let e = runner["error"] { return ["error": e] }
        var reply: [String: Any]
        var root: NSView?, rootKind = "view"
        if presenter.textHost(id) != nil {
            reply = layout(["id": Int(id)])
            if reply["error"] != nil { return reply }
            reply["nodes"] = nil
            root = presenter.textHost(id)
            if presenter.views[id] == nil { rootKind = "inline-owner" }
        } else {
            let size = presenter.viewport.contentView.bounds.size
            reply = ["clock": session.now(), "viewport": ["w": Agent.r2(size.width), "h": Agent.r2(size.height)], "node": runner]
            var at = runner["parent"].flatMap { Agent.integer($0) }
            while let p = at, root == nil {
                if let v = presenter.views[UInt32(p)] { if v.kind == "svg" { root = v; rootKind = "svg-owner" }; break }
                at = runnerNode(UInt32(p))["parent"].flatMap { Agent.integer($0) }
            }
        }
        var node = reply["node"] as? [String: Any] ?? runner
        var native = node["native"] as? [String: Any] ?? [:]
        native["subviews"] = root.map { presenter.inspectionDump($0, kind: rootKind, depth: depth, limit: limit) { self.box($0) } }
            ?? ["unavailable": "no platform view for #\(id) (a live node no view or owner paints)"]
        node["native"] = native
        reply["node"] = node
        return reply
    }

    // MARK: D2 — `layout agree`

    func agreement(limit: Int) -> [String: Any] {
        let size = presenter.viewport.contentView.bounds.size
        var reply: [String: Any] = ["clock": session.now(), "viewport": ["w": Agent.r2(size.width), "h": Agent.r2(size.height)]]
        let scale = presenter.viewport.window?.backingScaleFactor ?? 1
        let report = AgreementReport(tolerance: 1 / max(scale, 1))
        guard let kernel = kernelFrames() else { return ["error": "layout agree: the runner's frames are unreadable"] }
        guard let exactView = presenter.session?.view else { return ["error": "layout agree: the session has no view"] }
        if exactView.window == nil { report.incomplete("no-window") }
        if let window = exactView.window {
            report.excluded = window.sheets.map { "sheet \(String(describing: Swift.type(of: $0)))" }
                + (window.childWindows ?? []).map { "child window \(String(describing: Swift.type(of: $0)))" }
        }
        presenter.inspectAgreement(roots: [(exactView, "ExactView")], kernel: kernel, inFlight: nativeInFlight() || settle() != nil, report: report) { self.box($0) }
        if currentEpoch() != kernel.epoch { report.incomplete("spanned") }
        reply["agreement"] = report.json(limit: limit)
        return reply
    }
}

extension Presenter {
    private static let platformKinds: Set<String> = ["video", "iframe", "native", "canvas", "canvas2d", "input", "textarea", "control"]

    private func liveView(_ v: NSView) -> NodeView? {
        guard let n = v as? NodeView, views[n.id] === n else { return nil }
        return n
    }

    private func inspectionJudges(_ v: NSView, owner: NodeView?) -> Bool {
        if let n = liveView(v) { return !Self.platformKinds.contains(n.kind) && n.props["hook"] == nil }
        if v === root || v === session?.view { return true }
        if let o = owner {
            let containers: [NSView?] = [o.clipBox, o.scroll?.documentView, o.overlay, o.materialContent, o.glassGroupContent]
            if containers.contains(where: { $0 === v }) { return true }
        }
        return menus.inspectionOwns(v) || dialogs.inspectionOwns(v)
    }

    private func inspectionAccount(_ sub: NSView, in parent: NSView, owner: NodeView?) -> (role: String, owner: UInt32?)? {
        if let o = owner {
            let id = o.id
            if sub === o.symbolClip || sub === o.symbolView { return ("glyph", id) }
            if sub === o.materialView || sub === o.glassGroupView || sub === o.glassIsolation || sub === o.materialContent { return ("material", id) }
            if sub === o.clipBox { return ("clip", id) }
            if sub === o.metal || sub === o.overlay { return ("canvas", id) }
            if sub === o.field || sub === o.textArea || sub === o.textAreaScroll { return ("editor", id) }
            if sub === o.scroll { return ("scroll", id) }
            if sub === o.video || sub === o.web { return ("heavy", id) }
            if sub === controls.controls[id] { return ("control", id) }
            if segments.inspectionOwns(sub) { return ("segment", id) }
            if Self.platformKinds.contains(o.kind) || o.props["hook"] != nil { return ("platform", id) }
        }
        if menus.inspectionOwns(sub) { return ("menu", owner?.id) }
        if dialogs.inspectionOwns(sub) { return ("dialog", owner?.id) }
        if sub === viewport { return ("viewport", nil) }
        if sub === root { return ("document", nil) }
        if sub.nextResponder is NSViewController { return ("controller", nil) }
        return nil
    }

    private func inspectionHider(_ n: NodeView) -> String? {
        if n.placementHidden { return "placement" }
        if segments.hides(n) { return "segments" }
        if menus.hides(n) { return "menus" }
        if navigation.hides(n) { return "navigation" }
        if toolbar.hides(n) { return "toolbar" }
        return nil
    }

    /// D1: the views under `root`, preorder, bounded; `box` puts a view in
    /// the viewport's space.
    func inspectionDump(_ root: NSView, kind: String, depth: Int, limit: Int, box: (NSView) -> CGRect) -> [String: Any] {
        var leaving = Set<ObjectIdentifier>()
        for l in self.leaving.values { leaving.insert(ObjectIdentifier(l.view)); for m in l.members { leaving.insert(ObjectIdentifier(m)) } }
        var entries: [[String: Any]] = [], truncated: [String] = []
        func cut(_ reason: String) { if !truncated.contains(reason) { truncated.append(reason) } }
        func visit(_ v: NSView, _ d: Int, parent: NSView?, owner: NodeView?) {
            guard entries.count < limit else { cut("limit"); return }
            var e: [String: Any] = ["kind": "view", "depth": d, "class": String(describing: Swift.type(of: v)), "frame": AgreementReport.rect(box(v)),
                                    "hidden": v.isHidden, "alpha": Agent.r2(v.alphaValue)]
            if let layer = v.layer {
                var l: [String: Any] = ["masksToBounds": layer.masksToBounds, "zPosition": Agent.r2(layer.zPosition)]
                let t = layer.transform
                if !CATransform3DIsIdentity(t) {
                    if CATransform3DIsAffine(t) {
                        let a = CATransform3DGetAffineTransform(t)
                        l["transform"] = [a.a, a.b, a.c, a.d, a.tx, a.ty].map { Agent.r2($0) }
                    } else { l["transform"] = "3d" }
                }
                e["layer"] = l
            }
            let n = liveView(v)
            if let n { e["node"] = Int(n.id) }
            else if leaving.contains(ObjectIdentifier(v)) { e["role"] = "leaving" }
            else if let parent, let a = inspectionAccount(v, in: parent, owner: owner) { e["role"] = a.role; if let o = a.owner { e["owner"] = Int(o) } }
            else { e["role"] = "unaccounted" }
            let nextOwner = n ?? owner
            let entered = n != nil || inspectionJudges(v, owner: nextOwner)
            if !entered, !v.subviews.isEmpty { e["opaque"] = "platform"; e["children"] = v.subviews.count }
            entries.append(e)
            guard entered else { return }
            if d + 1 > depth { if !v.subviews.isEmpty { cut("depth") }; return }
            for sub in v.subviews { visit(sub, d + 1, parent: v, owner: nextOwner) }
        }
        visit(root, 0, parent: nil, owner: root as? NodeView)
        var out: [String: Any] = ["root": kind, "depth": depth, "limit": limit, "count": entries.count, "complete": truncated.isEmpty, "entries": entries]
        if !truncated.isEmpty { out["truncated"] = truncated }
        return out
    }

    /// D2: walk `roots`, then compare frames; `box` puts a view in the
    /// viewport's space for a disagreement's report.
    func inspectAgreement(roots: [(NSView, String)], kernel: KernelFrames, inFlight: Bool, report: AgreementReport,
                          box: (NSView) -> CGRect) {
        if !kernel.complete { report.incomplete("frames-cap") }
        if inFlight { report.incomplete("in-flight") }
        report.walked = roots.map(\.1)
        var leaving = Set<ObjectIdentifier>()
        for l in self.leaving.values { leaving.insert(ObjectIdentifier(l.view)); for m in l.members { leaving.insert(ObjectIdentifier(m)) } }
        var seen: [UInt32: NodeView] = [:]
        var stack: [(NSView, NodeView?)] = roots.reversed().map { ($0.0, nil) }
        walk: while let (v, owner) = stack.popLast() {
            let judged = inspectionJudges(v, owner: owner)
            for sub in v.subviews.reversed() {
                report.views += 1
                if report.views > AgreementReport.walkCap { report.incomplete("walk-cap"); break walk }
                if judged { report.judged += 1 } else { report.opaque += 1 }
                if let n = sub as? NodeView {
                    if views[n.id] === n {
                        seen[n.id] = n
                        report.hiddenCompared += 1
                        let hider = inspectionHider(n)
                        if n.isHidden {
                            if let hider { report.claim(hider) } else { report.add("hidden", ["id": Int(n.id), "native": ["hidden": true], "expected": ["hidden": false]]) }
                        } else if n.placementHidden {
                            report.add("hidden", ["id": Int(n.id), "native": ["hidden": false], "expected": ["hidden": true], "hider": "placement"])
                        }
                        stack.append((n, n))
                        continue
                    }
                    if leaving.contains(ObjectIdentifier(n)) { continue }
                    if judged {
                        var fields: [String: Any] = ["class": "NodeView", "retired": Int(n.id), "frame": AgreementReport.rect(box(n))]
                        if let o = owner { fields["under"] = Int(o.id) }
                        report.add("stray", fields)
                    }
                    continue
                }
                if judged, inspectionAccount(sub, in: v, owner: owner) == nil {
                    var fields: [String: Any] = ["class": String(describing: Swift.type(of: sub)), "frame": AgreementReport.rect(box(sub))]
                    if let o = owner { fields["under"] = Int(o.id) }
                    report.add("stray", fields)
                    continue
                }
                stack.append((sub, owner))
            }
        }
        for (id, l) in self.leaving {
            report.leaving += 1
            if !l.view.routeInert || !l.view.isAccessibilityHidden() {
                report.add("leaving-interactive", ["id": Int(id), "inert": l.view.routeInert, "accessibilityHidden": l.view.isAccessibilityHidden()])
            }
        }
        if !inFlight { compareFrames(kernel, seen: seen, report: report) }
    }

    private func compareFrames(_ kernel: KernelFrames, seen: [UInt32: NodeView], report: AgreementReport) {
        for id in kernel.order {
            guard let f = kernel.frames[id] else { continue }
            if f.inlineRun { report.skip("inline"); continue }
            if f.hostOwned { report.skip("region"); continue }
            guard let v = views[id] else {
                if inlineOwners[id] != nil { report.skip("inline"); continue }
                var at = f.parent, svg = false
                while let p = at { if let pv = views[p] { svg = pv.kind == "svg"; break }; at = kernel.frames[p]?.parent }
                report.skip(svg ? "svg" : "unviewed")
                continue
            }
            guard seen[id] === v, v.window != nil else { report.skip("offWindow"); continue }
            if v.placedAncestor != nil { report.skip("placed"); continue }
            if menus.projects(v) || dialogs.projects(v) { report.skip("projected"); continue }
            if toolbar.hides(v) { report.skip("projected"); continue }
            if f.transformed || v.frameRotation != 0 || v.layer.map({ !CATransform3DIsIdentity($0.transform) }) == true { report.skip("transformed"); continue }
            if v.layer?.animationKeys()?.isEmpty == false { report.skip("animating"); continue }
            let sv = v.superview
            var sizeOnly = false
            if let parent = f.parent {
                guard let pv = views[parent] else { report.skip("reparented"); continue }
                if sv === pv {
                } else if sv === pv.scroll?.documentView || sv === pv.overlay {
                    sizeOnly = true
                } else if let sv, [pv.clipBox, pv.materialContent, pv.glassGroupContent].contains(where: { $0 === sv }) {
                    sizeOnly = !(sv.frame == pv.bounds && sv.bounds.origin == .zero)
                } else { report.skip("reparented"); continue }
                if collections.owns(parent) { sizeOnly = true }
            } else if sv !== root { report.skip("reparented"); continue }
            report.compare(id, kernel: f.rect, native: v.frame, sizeOnly: sizeOnly)
        }
    }
}
#endif
