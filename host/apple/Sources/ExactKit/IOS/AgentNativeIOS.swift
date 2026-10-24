// @ref LLP 1080.001 — the UIKit walks behind `layout <target> native` and
// `layout agree` (the shared half is `AgentNative.swift`). Ownership is by
// identity, never by class: a subview is accounted for when it is a live
// node's view, a parked or leaving tree's, or one of the views the presenter
// and its hosts are known to add (D3's closed list, `inspectionAccount`).
// Strays are judged only in a subview list Exact owns (`inspectionJudges`);
// inside a platform view — a text view, a web view, a control, a
// controller's view — what is not a node is counted `opaque`, and any node
// found there is still matched. The walks are the presenter's, so a test
// drives them with frames of its own; the agent gathers their inputs.
#if os(iOS) || os(tvOS)
import UIKit

extension Agent {
    func layout() -> [String: Any] {
        let vp = presenter.viewport
        var nodes: [[String: Any]] = []
        for (id, v) in presenter.views.sorted(by: { $0.key < $1.key }) where v.window != nil {
            if presenter.menus.ownsConfirmationNode(v) {
                nodes.append(["id": Int(id), "presentation": "native-confirmation", "geometry": "unavailable"])
                continue
            }
            let nativeAction = presenter.swipeActions.ownsAction(id)
            let actionView = presenter.swipeActions.actionView(id)
            // A bar's segment is where UIKit draws it.
            let scope = (v as? NodeView).flatMap { presenter.navigation.barSegment($0) }.map { s in
                s.control.convert(s.segment, to: vp).offsetBy(dx: -vp.pageOrigin.x, dy: -vp.pageOrigin.y)
            }
            let r = nativeAction ? actionView.map { box($0) } ?? .zero : scope ?? box(v)
            var n: [String: Any] = ["id": Int(id), "x": Agent.r2(r.origin.x), "y": Agent.r2(r.origin.y), "w": Agent.r2(r.width), "h": Agent.r2(r.height)]
            if !nativeAction, scope == nil {
                n["radii"] = v.effectiveCornerSizes(in: v.bounds).map { [Double($0.width), Double($0.height)] }
            }
            if nativeAction { n["presentation"] = "native-swipe-action"; n["visible"] = actionView != nil }
            // A waiting scroll (a closed swipe row's) is at its start, with no range until it scrolls.
            if v.scrollDormant { n["sx"] = 0.0; n["sy"] = 0.0; n["rx"] = 0.0; n["ry"] = 0.0 }
            if let sv = v.scrollView {
                n["sx"] = Agent.r2(sv.contentOffset.x)
                n["sy"] = Agent.r2(sv.contentOffset.y + v.scrollTopInset(sv))
                n.merge(Agent.range(of: sv)) { _, new in new }
                n["port"] = port(of: v, sv)
                // How far past its own ends it sits: a stretched bounce is a
                // state a driver cannot read from the offset alone.
                let past = { (value: CGFloat, start: CGFloat, end: CGFloat) -> CGFloat in
                    if value < start - 0.5 { return value - start }
                    if value > end + 0.5 { return value - end }
                    return 0
                }
                let i = sv.adjustedContentInset
                let ox = past(sv.contentOffset.x, -i.left, max(-i.left, sv.contentSize.width + i.right - sv.bounds.width))
                let oy = past(sv.contentOffset.y, -i.top, max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height))
                if ox != 0 { n["ox"] = Agent.r2(ox) }
                if oy != 0 { n["oy"] = Agent.r2(oy) }
            }
            nodes.append(n)
        }
        for (id, leaf) in presenter.flats.leaves.sorted(by: { $0.key < $1.key }) {
            guard let host = presenter.flats.host(of: id), host.window != nil,
                  let frame = presenter.flats.frameInHost(id) else { continue }
            let r = box(host, region: frame)
            let local = CGRect(origin: .zero, size: leaf.frame.size)
            let corners = BorderPaint.reduced(BorderPaint.radii(leaf.style, in: local), in: local)
            nodes.append(["id": Int(id), "x": Agent.r2(r.minX), "y": Agent.r2(r.minY),
                          "w": Agent.r2(r.width), "h": Agent.r2(r.height),
                          "radii": corners.map { [Double($0.width), Double($0.height)] }])
        }
        nodes.append(contentsOf: inlineLayout())
        nodes.sort { ($0["id"] as? Int ?? 0) < ($1["id"] as? Int ?? 0) }
        // The page's environment (LLP 1012 §1): the insets the kernel was
        // given, and the keyboard's inset on the viewport, by the web's
        // `env()` names.
        let i = presenter.insets
        let env: [String: Any] = ["safe-area-inset-top": Agent.r2(i.top), "safe-area-inset-right": Agent.r2(i.right), "safe-area-inset-bottom": Agent.r2(i.bottom), "safe-area-inset-left": Agent.r2(i.left), "keyboard-inset-height": Agent.r2(presenter.keyboardInset)].merging(presenter.fold.env) { a, _ in a }
            .merging(LayoutRegions.env(presenter.regions, safe: (i.top, i.right, i.bottom, i.left), round: Agent.r2)) { a, _ in a }
        var reply: [String: Any] = ["clock": session.now(), "viewport": ["w": Agent.r2(vp.pageSize.width), "h": Agent.r2(vp.pageSize.height)], "env": env, "nodes": nodes]
        // A presentation the platform draws scaled (a floating sheet): its
        // boxes above are the authored frames times `scale`, not the app's doing.
        let scaled = presenter.modals.drawnScales.map { ["route": $0.route.props["navigationKey"] ?? "", "presentation": $0.kind, "scale": (Double($0.scale) * 1000).rounded() / 1000] as [String: Any] }
        if !scaled.isEmpty { reply["scaled"] = scaled }
        #if os(iOS)
        // The status bar's style Exact asks for (LLP 1105 D7), and the node that decided it.
        let bar = presenter.statusBar
        reply["statusBar"] = ["style": bar.style == .lightContent ? "light-content" : bar.style == .darkContent ? "dark-content" : "default",
                              "source": bar.source.map { Int($0) } ?? NSNull()] as [String: Any]
        #endif
        // The device's screen, where the viewport sits on it (LLP 1035.002
        // D4's `screen` space) and the scene's interface orientation: what a
        // real touch's aim is checked against (LLP 1080.000 D4).
        if let w = vp.window, let container = vp.superview {
            let screen = w.screen
            let origin = container.convert(presenter.pageFrame.origin, to: screen.coordinateSpace) // the page's area (LLP 1130)
            reply["screen"] = ["w": Agent.r2(screen.bounds.width), "h": Agent.r2(screen.bounds.height), "scale": Agent.r2(screen.scale),
                               "x": Agent.r2(origin.x), "y": Agent.r2(origin.y), "orientation": Agent.orientation(w.windowScene)]
        }
        return reply
    }


    // MARK: D1 — `layout <target> native`

    func nativeSubviews(_ id: UInt32, depth: Int, limit: Int, plan: Bool) -> [String: Any] {
        let runner = runnerNode(id, plan: plan)
        if let e = runner["error"] { return ["error": e] }
        var reply: [String: Any]
        var root: UIView?, rootKind = "view", flatOnly: UInt32?
        if presenter.textHost(id) != nil {
            reply = layout(["id": Int(id), "plan": plan])
            if reply["error"] != nil { return reply }
            reply["nodes"] = nil
            root = presenter.textHost(id)
            if presenter.views[id] == nil { rootKind = "inline-owner" }
        } else {
            let vp = presenter.viewport
            reply = ["clock": session.now(), "viewport": ["w": Agent.r2(vp.pageSize.width), "h": Agent.r2(vp.pageSize.height)], "node": runner]
            if let parent = presenter.flats.inspectionParent(of: id), let pv = presenter.views[parent] {
                root = pv; rootKind = "flat-run"; flatOnly = id
            } else {
                // An SVG element: its `svg` node's view.
                var at = Agent.integer(runner["parent"])
                while let p = at, root == nil {
                    if let v = presenter.views[UInt32(p)] { if v.kind == "svg" { root = v; rootKind = "svg-owner" }; break }
                    at = Agent.integer(runnerNode(UInt32(p))["parent"])
                }
            }
        }
        var node = reply["node"] as? [String: Any] ?? runner
        var native = node["native"] as? [String: Any] ?? [:]
        native["subviews"] = root.map { presenter.inspectionDump($0, kind: rootKind, depth: depth, limit: limit, flatOnly: flatOnly) { self.box($0, region: $1) } }
            ?? ["unavailable": "no platform view for #\(id) (a live node no view, flat run or owner paints)"]
        node["native"] = native
        reply["node"] = node
        return reply
    }

    // MARK: D2 — `layout agree`

    func agreement(limit: Int) -> [String: Any] {
        let vp = presenter.viewport
        var reply: [String: Any] = ["clock": session.now(), "viewport": ["w": Agent.r2(vp.pageSize.width), "h": Agent.r2(vp.pageSize.height)]]
        let scale = vp.window?.screen.scale ?? vp.traitCollection.displayScale
        // A pixel, and the layout unit an edge is taken to before it is
        // rounded to one (`PixelGridIOS.swift`).
        let report = AgreementReport(tolerance: 1 / max(scale, 1) + 1.0 / 64)
        guard let kernel = kernelFrames() else { return ["error": "layout agree: the runner's frames are unreadable"] }
        guard let exactView = presenter.session?.view else { return ["error": "layout agree: the session has no view"] }
        if exactView.window == nil { report.incomplete("no-window") }
        let roots = presenter.surfaces(includingRetiring: true).map { ($0.view, $0.label) }
        presenter.inspectAgreement(roots: roots, kernel: kernel, inFlight: nativeInFlight() || settle() != nil, report: report) { self.box($0) }
        if currentEpoch() != kernel.epoch { report.incomplete("spanned") }
        reply["agreement"] = report.json(limit: limit)
        return reply
    }
}

extension Presenter {
    /// Kinds whose view holds a platform object: what they add is UIKit's
    /// (NodePool's heavy leaves, LLP 1068 §4.0).
    private static let platformKinds: Set<String> = NodePool.leaves

    /// The trees the presenter keeps on purpose: parked, leaving.
    private func keptTrees() -> (parked: (roots: [NodeView], members: Set<ObjectIdentifier>), leaving: Set<ObjectIdentifier>) {
        var leaving = Set<ObjectIdentifier>()
        for l in self.leaving.values {
            leaving.insert(ObjectIdentifier(l.view))
            for m in l.members { leaving.insert(ObjectIdentifier(m)) }
        }
        return (pool.inspection, leaving)
    }

    private func liveView(_ v: UIView) -> NodeView? {
        guard let n = v as? NodeView, views[n.id] === n else { return nil }
        return n
    }

    /// Whether Exact owns `v`'s subview list, so a subview nothing accounts
    /// for there is a stray. `owner` is the nearest node view at or above `v`.
    private func inspectionJudges(_ v: UIView, owner: NodeView?) -> Bool {
        if let n = liveView(v) { return !Self.platformKinds.contains(n.kind) && n.props["hatch"] == nil }
        if v === viewport || v === root || v === session?.view { return true }
        if let o = owner {
            let containers: [UIView?] = [o.clipBox, o.scroll, o.overlay, o.materialView?.contentView,
                                         o.glassGroupView?.contentView, o.glassSlot?.contentView]
            if containers.contains(where: { $0 === v }) { return true }
        }
        return surfaces(includingRetiring: true).contains { $0.view === v } || menus.inspectionOwns(v)
    }

    /// What accounts for `sub`, a subview of `parent` under node `owner`, by
    /// identity: its role and the node it belongs to; nil when nothing does.
    private func inspectionAccount(_ sub: UIView, in parent: UIView, owner: NodeView?) -> (role: String, owner: UInt32?)? {
        if let o = owner {
            let id = o.id
            if sub === o.symbolView { return ("glyph", id) }
            if sub === o.textRasterTint { return ("text", id) } // its one-colour text, tinted as it composites (TextRasterIOS.swift)
            if sub === o.materialView || sub === o.glassSlot || sub === o.glassGroupView || sub === o.vibrancyView { return ("material", id) }
            if sub === o.clipBox { return ("clip", id) }
            if sub === o.metal || sub === o.overlay { return ("canvas", id) }
            if sub === o.field || sub === o.textArea { return ("editor", id) }
            if sub === o.scroll { return ("scroll", id) }
            if sub === o.video || sub === o.web { return ("heavy", id) }
            if sub === controls.controls[id] { return ("control", id) }
            if segments.inspectionOwns(sub) { return ("segment", id) }
            if swipeActions.inspectionOwns(sub) { return ("swipe", id) }
            if groupedLists?.inspectionOwns(sub) == true { return ("grouped-list", id) }
            if Self.platformKinds.contains(o.kind) || o.props["hatch"] != nil { return ("platform", id) }
        }
        if menus.inspectionOwns(sub) { return ("menu", owner?.id) }
        // A live flight's layer on the presentation it flies in, and the
        // empty slot it holds open where its view lays out (FlightsIOS.swift).
        if flights.values.contains(where: { $0.container === sub || $0.slot === sub }) { return ("flight", nil) }
        // A grouped list's background past the viewport's edge, beside it (LLP 1116.004 D2).
        if owner == nil, groupedLists?.inspectionOwns(sub) == true { return ("grouped-list", nil) }
        if sub === viewport { return ("viewport", nil) }
        if sub === root { return ("document", nil) }
        if session?.view?.inspectionOwned.contains(where: { $0 === sub }) == true { return ("probe", nil) }
        if surfaces(includingRetiring: true).contains(where: { $0.owned.contains { $0 === sub } }) { return ("modal", nil) }
        // A controller's root view: navigation stacks, tabs, a modal's
        // covered owner. Its interior is UIKit's; nodes in it are matched.
        if sub.next is UIViewController { return ("controller", nil) }
        if let scroll = parent as? UIScrollView {
            #if !os(tvOS)
            if sub === scroll.refreshControl { return ("refresh", owner?.id) }
            #endif
            if String(describing: Swift.type(of: sub)) == "_UIScrollViewScrollIndicator" { return ("indicator", owner?.id) }
            // iOS 26's scroll edge effect (LLP 1077 D16): UIKit hangs each
            // edge's effect in a passthrough container on the scroll view,
            // the root viewport's included under `viewport-fit="cover"`.
            let kinds = sub.subviews.map { String(describing: Swift.type(of: $0)) }
            if String(describing: Swift.type(of: sub)) == "_UITouchPassthroughView",
               kinds.contains(where: { $0 == "ScrollEdgeEffectView" || $0 == "BackdropView" }) { return ("edge-effect", owner?.id) }
        }
        return nil
    }

    /// The host that hid `n`, when one may (D3's hiders).
    private func inspectionHider(_ n: NodeView) -> String? {
        if n.placementHidden { return "placement" }
        if segments.hides(n) { return "segments" }
        if menus.hides(n) { return "menus" }
        if navigation.controllers.values.contains(where: { $0.lifted === n }) { return "navigation" }
        if swipeActions.hides(n) { return "swipe" }
        if groupedLists?.hides(n) == true { return "grouped-list" }
        return nil
    }

    private func geometryAnimating(_ layer: CALayer) -> Bool {
        func moves(_ animation: CAAnimation) -> Bool {
            if let group = animation as? CAAnimationGroup { return group.animations?.contains(where: moves) == true }
            guard let path = (animation as? CAPropertyAnimation)?.keyPath, let property = path.split(separator: ".").first else { return false }
            return ["bounds", "position", "transform", "anchorPoint"].contains(String(property))
        }
        return layer.animationKeys()?.contains { layer.animation(forKey: $0).map(moves) == true } == true
    }

    /// D1: the views and flat-leaf layers under `root`, preorder, bounded;
    /// `box` puts a view's region in the viewport's space.
    func inspectionDump(_ root: UIView, kind: String, depth: Int, limit: Int, flatOnly: UInt32?,
                        box: (UIView, CGRect?) -> CGRect) -> [String: Any] {
        let kept = keptTrees()
        var entries: [[String: Any]] = [], truncated: [String] = []
        func cut(_ reason: String) { if !truncated.contains(reason) { truncated.append(reason) } }
        func layers(of n: NodeView, at d: Int) {
            for (layer, leaves) in flats.inspectionLayers(under: n.id) {
                if let only = flatOnly, !leaves.contains(only) { continue }
                guard entries.count < limit else { cut("limit"); return }
                // A run of zero-sized leaves has an empty path, whose box is
                // null (infinite origins): reported as empty, never encoded.
                let local = (layer as? CAShapeLayer).map { $0.path?.boundingBoxOfPath ?? .null } ?? layer.frame
                let path: [String: Any] = local.isNull || local.isInfinite || ![local.minX, local.minY, local.width, local.height].allSatisfy(\.isFinite)
                    ? ["x": 0, "y": 0, "w": 0, "h": 0, "empty": true] : AgreementReport.rect(box(n.container, local))
                var e: [String: Any] = ["kind": "layer", "depth": d, "class": String(describing: Swift.type(of: layer)),
                                        "role": leaves.count > 1 ? "flat-run" : "flat-leaf", "leaves": leaves.prefix(16).map(Int.init),
                                        "leafCount": leaves.count, "path": path,
                                        "attached": layer.superlayer != nil, "hidden": layer.isHidden, "opacity": Agent.r2(CGFloat(layer.opacity))]
                if !CATransform3DIsIdentity(layer.transform) { e["transform"] = "3d" }
                entries.append(e)
            }
        }
        func visit(_ v: UIView, _ d: Int, parent: UIView?, owner: NodeView?) {
            guard entries.count < limit else { cut("limit"); return }
            var e: [String: Any] = ["kind": "view", "depth": d, "class": String(describing: Swift.type(of: v)), "frame": AgreementReport.rect(box(v, nil)),
                                    "hidden": v.isHidden, "alpha": Agent.r2(v.alpha), "interactive": v.isUserInteractionEnabled]
            var l: [String: Any] = ["masksToBounds": v.layer.masksToBounds, "zPosition": Agent.r2(v.layer.zPosition)]
            let t = v.layer.transform
            if !CATransform3DIsIdentity(t) {
                if CATransform3DIsAffine(t) {
                    let a = CATransform3DGetAffineTransform(t)
                    l["transform"] = [a.a, a.b, a.c, a.d, a.tx, a.ty].map { Agent.r2($0) }
                } else { l["transform"] = "3d" }
            }
            e["layer"] = l
            let n = liveView(v)
            if let n { e["node"] = Int(n.id) }
            else if kept.parked.members.contains(ObjectIdentifier(v)) { e["role"] = "pooled" }
            else if kept.leaving.contains(ObjectIdentifier(v)) { e["role"] = "leaving" }
            else if let parent, let a = inspectionAccount(v, in: parent, owner: owner) { e["role"] = a.role; if let o = a.owner { e["owner"] = Int(o) } }
            else { e["role"] = "unaccounted" }
            let nextOwner = n ?? owner
            let entered = n != nil || inspectionJudges(v, owner: nextOwner)
            if !entered, !v.subviews.isEmpty { e["opaque"] = "platform"; e["children"] = v.subviews.count }
            entries.append(e)
            // A node's flat layers are one level down, under the same bound as its views.
            if let n {
                if d + 1 > depth { if !flats.inspectionLayers(under: n.id).isEmpty { cut("depth") } } else { layers(of: n, at: d + 1) }
            }
            guard entered else { return }
            if d + 1 > depth { if !v.subviews.isEmpty { cut("depth") }; return }
            for sub in v.subviews { visit(sub, d + 1, parent: v, owner: nextOwner) }
        }
        if flatOnly != nil, let n = root as? NodeView { layers(of: n, at: 0) } else { visit(root, 0, parent: nil, owner: root as? NodeView) }
        var out: [String: Any] = ["root": kind, "depth": depth, "limit": limit, "count": entries.count, "complete": truncated.isEmpty, "entries": entries]
        if !truncated.isEmpty { out["truncated"] = truncated }
        return out
    }

    /// D2: walk `roots` and the kept trees, then compare frames; `box` puts
    /// a view in the viewport's space for a disagreement's report.
    func inspectAgreement(roots: [(UIView, String)], kernel: KernelFrames, inFlight: Bool, report: AgreementReport,
                          box: (UIView) -> CGRect) {
        if !kernel.complete { report.incomplete("frames-cap") }
        if inFlight { report.incomplete("in-flight") }
        let kept = keptTrees()
        let retiringRoots = modals.inspectionRetiring
        report.walked = roots.map(\.1)
        var seen: [UInt32: NodeView] = [:]
        var stack: [(UIView, NodeView?)] = roots.reversed().map { ($0.0, nil) }
        walk: while let (v, owner) = stack.popLast() {
            let judged = inspectionJudges(v, owner: owner)
            for sub in v.subviews.reversed() {
                report.views += 1
                if report.views > AgreementReport.walkCap { report.incomplete("walk-cap"); break walk }
                if judged { report.judged += 1 } else {
                    report.opaque += 1
                    if let what = elements.regions.owner(of: sub) { report.opaqueBy[what, default: 0] += 1 }
                }
                if let n = sub as? NodeView {
                    // In the presenter's map is not alive: the kernel must
                    // still have the node (a complete `frames`), or the view
                    // is a retired one the map forgot to drop.
                    if views[n.id] === n, kernel.complete, kernel.frames[n.id] == nil {
                        var fields: [String: Any] = ["class": "NodeView", "retired": Int(n.id), "inMap": true, "frame": AgreementReport.rect(box(n))]
                        if let o = owner { fields["under"] = Int(o.id) }
                        report.add("stray", fields)
                        continue
                    }
                    if views[n.id] === n {
                        seen[n.id] = n
                        if n.window != nil {
                            report.hiddenCompared += 1
                            let hider = inspectionHider(n)
                            if n.isHidden {
                                if let hider { report.claim(hider) } else { report.add("hidden", ["id": Int(n.id), "native": ["hidden": true], "expected": ["hidden": false]]) }
                            } else if n.placementHidden {
                                report.add("hidden", ["id": Int(n.id), "native": ["hidden": false], "expected": ["hidden": true], "hider": "placement"])
                            }
                        }
                        stack.append((n, n))
                        continue
                    }
                    if kept.parked.members.contains(ObjectIdentifier(n)) || kept.leaving.contains(ObjectIdentifier(n)) { continue }
                    // A retiring modal keeps its removed route on screen until
                    // UIKit's dismissal ends (`ModalIOS.swift`): kept on purpose.
                    if retiringRoots.contains(where: { n === $0 || n.isDescendant(of: $0) }) { continue }
                    // An unmapped node view is a leak wherever it hangs, an
                    // opaque controller interior included (D3: nodes there are matched).
                    do {
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
        // The trees kept on purpose must not show or take input (D2).
        for root in kept.parked.roots {
            report.parkedRoots += 1
            if !root.isHidden { report.add("parked-visible", ["class": "NodeView", "frame": AgreementReport.rect(box(root))]) }
        }
        for (id, l) in leaving {
            report.leaving += 1
            if l.view.isUserInteractionEnabled || !l.view.accessibilityElementsHidden {
                report.add("leaving-interactive", ["id": Int(id), "interactive": l.view.isUserInteractionEnabled, "accessibilityHidden": l.view.accessibilityElementsHidden])
            }
        }
        if !inFlight { compareFrames(kernel, seen: seen, report: report) }
        labelDisagreements(report, kernel: kernel)
    }

    /// testIds from the snapshot the walk read, never a later tree: a live
    /// node (mapped and in `frames`) gives its own; a retired one none.
    private func labelDisagreements(_ report: AgreementReport, kernel: KernelFrames) {
        func testId(_ id: Any?) -> String? {
            guard let id = id as? Int, let v = views[UInt32(id)], kernel.frames[UInt32(id)] != nil else { return nil }
            return v.props["testId"]
        }
        report.found = report.found.map { d in
            var d = d
            if let t = testId(d["id"]) { d["testId"] = t }
            if let t = testId(d["under"]) { d["underTestId"] = t }
            return d
        }
    }

    /// `frame` (D2): only an ordinary node — in a window, untransformed in
    /// the view and the model, not animating, not placed, projected, held
    /// or reparented — and its origin only where its superview lines up
    /// with its parent's bounds.
    private func compareFrames(_ kernel: KernelFrames, seen: [UInt32: NodeView], report: AgreementReport) {
        for id in kernel.order {
            guard let f = kernel.frames[id] else { continue }
            if f.inlineRun { report.skip("inline"); continue }
            if f.hostOwned { report.skip("region"); continue }
            guard let v = views[id] else {
                if flats.isFlat(id) { report.skip("flat") }
                else if inlineOwners[id] != nil { report.skip("inline") }
                else {
                    var at = f.parent, svg = false
                    while let p = at { if let pv = views[p] { svg = pv.kind == "svg"; break }; at = kernel.frames[p]?.parent }
                    report.skip(svg ? "svg" : "unviewed")
                }
                continue
            }
            guard seen[id] === v, v.window != nil else { report.skip("offWindow"); continue }
            if v.placedAncestor != nil { report.skip("placed"); continue }
            if menus.projects(v) || swipeActions.projects(v) || groupedLists?.projects(v) == true { report.skip("projected"); continue }
            if navigation.controllers[id]?.node === v { report.skip("route"); continue }
            if f.transformed || !CATransform3DIsIdentity(v.layer.transform) { report.skip("transformed"); continue }
            if geometryAnimating(v.layer) { report.skip("animating"); continue }
            let sv = v.superview
            var sizeOnly = false
            if let parent = f.parent {
                guard let pv = views[parent] else { report.skip("reparented"); continue }
                if sv === pv {
                } else if sv === pv.scroll || sv === pv.overlay {
                    sizeOnly = true
                } else if let sv, [pv.clipBox, pv.materialView?.contentView, pv.glassGroupView?.contentView, pv.glassSlot?.contentView].contains(where: { $0 === sv }) {
                    sizeOnly = !(sv.frame == pv.bounds && sv.bounds.origin == .zero)
                } else { report.skip("reparented"); continue }
                // A collection row's origin is the list's (LLP 1080.001 §5).
                if collections.owns(parent) { sizeOnly = true }
            } else if sv !== root { report.skip("reparented"); continue }
            report.compare(id, kernel: f.rect, native: v.frame, sizeOnly: sizeOnly)
        }
    }
}
#endif
