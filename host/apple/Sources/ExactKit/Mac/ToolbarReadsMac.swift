// @ref LLP 1115.002 D2 — the window's bar is presented again only by a batch
// that touches what it read. Each present records its reads (`BarReads`):
// the routes it consulted and each one's direct children, the header it
// read (shown, kept painted, or the declared toolbar), every node carrying
// `toolbarPlacement` wherever it sits, each read button's invocation target
// (its menu, its popover and that popover's `position_area`, wherever it
// lives) and every view its names find, taken or refused, what an
// `aria-labelledby` names (a readout's, a tablist's, a search field's name),
// a route's first child that is a `header` whether the rule takes it or
// not, the ancestors of all of these (their `display`, `hidden` and
// `inert` decide what the bar shows), the names its buttons resolved, and
// whether it read a native control, whose contents are viewless and change
// only in a batch that says so (`Batch.controls`). A batch is then tested
// against that record before its ops (`concerns`), and what NavigationHost
// decided from it after them (`BarInputs`): the route the bar shows comes
// from routes the bar never read.
//
// A scrolling list's slice of rows touches none of it, so it no longer
// presents the bar: a fifth of `Presenter.apply` while Excord's transcript
// filled rows (QUEUE, 2026-10-25; x2-perf4's plan, theme2 2026-10-29 05:08).
// An exclusion (skip a collection's rows) could not be made sound: a toolbar
// can sit in a row, and a header's popover can live in one.
//
// In a debug build under XCTest, or with EXACT_BAR_SHADOW=1, every batch the
// record skips presents the bar anyway and compares what it shows before
// and after (`ShadowCheck`): a difference is a read the record missed. It
// is said in the session's log and counted, and under XCTest it fails the
// run. A release build compiles none of it.
#if os(macOS)
import AppKit

struct BarReads {
    var nodes = Set<UInt32>()
    var names = Set<String>()
    var controls = false
}

/// What NavigationHost, the split, the dialogs and the sidebar decided that
/// the bar's present reads, compared after each batch's navigation pass.
struct BarInputs: Equatable {
    var selection: [UInt32] = []
    var barRoute = false, hasStack = false, backInToolbar = false
    var split: [UInt32] = []
    var listOwnsWindow = true
    var modal: UInt32?
    var sheet = false, sidebar = false, sidebarItems = false
    /// The page is in the bar's window, and the window's toolbar is still the bar's.
    var placed = false, installed = false
    /// Whether each route the bar shows a header of is shown and live: the
    /// navigation's pass gates routes (`hidden`, `inert`) with no op on them.
    var routes: [UInt32] = []
    /// Each declared toolbar's, shown and live or not: a navigation root anywhere gates its routes, a
    /// declaration's among them, with no op on them.
    var carriers: [UInt32] = []
    /// What Back's enabled state is decided from, with no call to the runner (`NavigationHost.backFacts`).
    var back: [UInt32] = []
    /// The page's appearance (dark, Increase Contrast): what a light-dark ink, a tint and a painted face resolve by.
    var appearance = ""
    /// What the host hides or makes inert, with no op, of everything the bar read and its ancestors: a
    /// navigation root anywhere gates its routes (`NavigationHost.gate`), an alert or a popover the bar
    /// opens among them. Only the nodes so gated, by their bits (1 hidden, 2 inert); the bar's own
    /// projection hiding is not the host's.
    var gates: [UInt32: UInt8] = [:]
    /// The nodes the bar projects (its header or declaration, the parts it takes from the header beside
    /// it), each hidden as it left them: the navigation's pass can show a route again on any batch.
    var projected: [UInt32: Bool] = [:]
}

extension WindowToolbarHost {
    /// Whether a batch can change what the bar shows: one that touches what
    /// it read at its last present, makes or names a toolbar anywhere, says a
    /// control's viewless contents changed while the bar shows a control,
    /// moves the tree's roots, carries a command, or names a node with no
    /// view it could place. Every other batch (a list's rows, a scroll's
    /// frames, an animation's values) leaves the bar as it stands; what the
    /// navigation decides from it is `inputsChanged`, after its pass.
    func concerns(_ batch: Batch) -> Bool {
        if batch.controls && reads.controls { return true }
        guard !batch.ops.isEmpty else { return false }
        let nodes = reads.nodes, names = reads.names
        var created: Set<UInt32>?
        // A node with no view, made by none of this batch's ops: nothing to place, so it may be the bar's.
        // Conservative: an op on a viewless node anywhere in the page presents the bar (rare: a viewless
        // node changes through `Batch.controls`, not ops; Excord's fill hits none).
        func unplaced(_ id: UInt32) -> Bool {
            guard presenter.views[id] == nil, presenter.leaving[id] == nil else { return false }
            if created == nil { created = Set(batch.ops.lazy.filter { $0.op == .create }.map(\.id)) }
            return !created!.contains(id)
        }
        for op in batch.ops {
            let id = op.id
            // An op that names no node (a theme's roles, the document's language, the router's place, a scheme
            // or a kind this test does not know) may change what the bar resolves: it presents, unless it is one
            // of these, which no part of the bar reads: a list window's bookkeeping, sound, MIDI, a Live
            // Activity, an authentication or presence session, a canvas surface's work, a drag's bindings and
            // a reorder's state, and the head's title, which `headTitle` puts on the window itself.
            if op.nodeID == nil {
                switch op.op {
                case .collections, .sound, .midi, .activity, .auth, .presence, .surfaceWork, .reorder, .heightDrag, .transformDrag, .retireMotion, .title:
                    continue
                default: return true
                }
            }
            switch op.op {
            case .roots, .command: return true
            case .create:
                // A toolbar arriving anywhere, a row included: a carrier the bar has not read is by definition missing from its reads.
                if op.props["toolbarPlacement"] != nil { return true }
                if let name = op.props["id"], names.contains(name) { return true }
            case .props:
                if nodes.contains(id) { return true }
                if op.props["toolbarPlacement"] != nil || op.clear.contains("toolbarPlacement") { return true }
                if !names.isEmpty, op.props["id"].map(names.contains) == true || op.clear.contains("id") { return true }
                if unplaced(id) { return true }
            case .style:
                if nodes.contains(id) || unplaced(id) { return true }
            case .paragraph:
                if nodes.contains(id) || unplaced(id) { return true }
                // A run taking a name a read button or label refers to: an invocation finds paragraphs' runs too.
                if !names.isEmpty, op.runs.contains(where: { $0.props["id"].map(names.contains) == true }) { return true }
            case .destroy:
                if nodes.contains(id) { return true }
            case .children:
                if nodes.contains(id) || op.ids.contains(where: nodes.contains) || unplaced(id) { return true }
            case .present:
                // A painted face folds its nodes' presented opacity into the item's image (`MacBarFace`).
                if op.property == "opacity", nodes.contains(id) { return true }
            case .frame, .content, .rank, .sticky, .fragments, .flow, .fieldContent, .svg, .animations, .canvas2d, .surface, .surfaceWork:
                break
            default:
                // Anything else on a node the bar read (a block's answer that shows a pane's route, an exit, a flight): it may show or hide it.
                if nodes.contains(id) { return true }
            }
        }
        return false
    }

    /// The broad test NavigationHost's sheet headers still use: any op that
    /// makes, removes or moves a node, writes props, style or words, or
    /// carries a command (RouteSheetMac.swift reads only with a sheet up).
    func mayChangeAHeader(_ batch: Batch) -> Bool { batch.ops.contains { Self.broad.contains($0.op) } }
    private static let broad: Set<BatchOp.Kind> = [.create, .destroy, .children, .roots, .props, .style, .paragraph, .command]

    /// What the navigation and its neighbours decide now, of what the bar reads.
    func currentInputs() -> BarInputs {
        let navigation = presenter.navigation
        var inputs = BarInputs()
        if let s = navigation.selection { inputs.selection = [s.root, UInt32(clamping: s.at)] + s.stack }
        inputs.barRoute = navigation.barRoute
        inputs.hasStack = navigation.hasStack
        inputs.backInToolbar = navigation.backInToolbar
        inputs.back = navigation.backFacts
        let split = navigation.split
        if split.active, let shown = split.shown { inputs.split = [shown.root.id, shown.list.id, shown.detail?.id ?? 0] }
        inputs.listOwnsWindow = split.listOwnsWindow
        inputs.modal = presenter.dialogs.active?.id
        let routes = [navigation.headerRoute?.route, navigation.splitHeaderRoutes?.owner, navigation.splitHeaderRoutes?.beside].compactMap { $0 }
        inputs.routes = routes.map { route in route.id << 2 | (visible(route) ? 1 : 0) | (route.inert ? 2 : 0) }
        inputs.carriers = presenter.carrying("toolbarPlacement").map { node in node.id << 2 | (visible(node) ? 1 : 0) | (node.inert ? 2 : 0) }
        inputs.appearance = presenter.viewport.effectiveAppearance.name.rawValue
        var mine = besideParts.map(\.node)
        if projected, let owner { mine.append(owner) }
        for node in mine { inputs.projected[node.id] = node.isHidden }
        // By identity, once: the pass over the record is then linear, not one search of `mine` a node.
        let projecting = Set(mine.map(ObjectIdentifier.init))
        for id in reads.nodes {
            guard let node = presenter.views[id] else { continue }
            let hidden = node.isHidden && !projecting.contains(ObjectIdentifier(node))
            let bits: UInt8 = (hidden ? 1 : 0) | (node.routeInert ? 2 : 0)
            if bits != 0 { inputs.gates[id] = bits }
        }
        if let window {
            inputs.placed = presenter.root.window === window
            inputs.installed = toolbar != nil && window.toolbar === toolbar
            inputs.sheet = window.attachedSheet != nil
            inputs.sidebar = presenter.sidebar.installed(in: window)
            inputs.sidebarItems = inputs.sidebar && presenter.sidebar.inToolbar(of: window)
        }
        return inputs
    }

    /// The bar presented before the document's language moved (`applyLanguage` runs after the batch):
    /// once more, after it.
    func languageApplied(_ batch: Batch) {
        if batch.ops.contains(where: { $0.op == .language }) { sync() }
    }

    /// Whether what the navigation decided moved since the bar's last present.
    func inputsChanged() -> Bool { window != nil && inputs != currentInputs() }

    /// The record of what the present that just ran read (`concerns`).
    func recordReads() {
        var r = BarReads()
        defer { reads = r; inputs = currentInputs() }
        guard window != nil else { return }
        var walked = Set<UInt32>()
        func note(_ node: NodeView) {
            r.nodes.insert(node.id)
            if node.kind == "control" { r.controls = true }
        }
        // Every view carrying `name` as its `id`, whether the reader takes it or not (an invocation
        // refuses a view with no `popover`, and resolves to the lowest id): any of them can become the one.
        func named(_ name: String) {
            guard r.names.insert(name).inserted else { return }
            for id in (presenter.chrome.named[name] ?? []).sorted() { if let view = presenter.views[id] { subtree(view) } }
        }
        func subtree(_ node: NodeView) {
            guard walked.insert(node.id).inserted else { return }
            note(node)
            if node.isButton {
                for key in ["popovertarget", "commandfor"] { if let name = node.props[key], !name.isEmpty { named(name) } }
                if let target = presenter.invocation(of: node)?.target { subtree(target) }
            }
            // `aria-labelledby`: the names and words of what it names are the node's label (`authoredLabel`),
            // a readout's, a tablist's or a search field's item's name.
            if let refs = node.props["accessibilityLabelledBy"] { refs.split(separator: " ").forEach { named(String($0)) } }
            for case let child as NodeView in node.container.subviews { subtree(child) }
        }
        let navigation = presenter.navigation
        var routes: [(root: NodeView, route: NodeView, pushed: Bool)] = []
        if let h = navigation.headerRoute { routes.append(h) }
        if let s = navigation.splitHeaderRoutes {
            routes.append((s.root, s.owner, true))
            if let beside = s.beside { routes.append((s.root, beside, true)) }
        }
        // Every route in the split's pane: a block's answer for the one under the detail shows the detail.
        if navigation.split.active, let shown = navigation.split.shown { shown.routes.forEach(note) }
        for (root, route, _) in routes {
            note(root); note(route)
            // The route's own children: which one is its header (`HeaderRule`). A first child that is a
            // `header` is walked whole by the rule whatever it answers, so it is read whole, refused or not.
            let kids = route.container.subviews.compactMap { $0 as? NodeView }
            kids.forEach(note)
            if let first = kids.first, first.props["semanticTag"] == "header" { subtree(first) }
        }
        for header in [header, beside].compactMap({ $0 }) { subtree(header.header) }
        for carrier in presenter.carrying("toolbarPlacement") { subtree(carrier) }
        // The paragraphs whose runs carry a name read (`invocationElement` finds runs too): a run losing it
        // is a paragraph op there. One pass over the page's runs for all the names, again if one adds a name.
        var looked = 0
        while looked < r.names.count, !presenter.inlineOwners.isEmpty {
            looked = r.names.count
            for location in presenter.inlineOwners.values {
                guard let owner = presenter.views[location.owner], !walked.contains(owner.id), owner.inlineText.indices.contains(location.index),
                      let name = owner.inlineText[location.index].props["id"], r.names.contains(name) else { continue }
                subtree(owner)
            }
        }
        // Their ancestors: what is shown, inert or in this window is decided there.
        // Both chains: the views' own (`visible`) and the authored one (an open popover's parent, a pane's home: `inert`).
        // Each chain stops at a node it has climbed from before: what is above it is recorded.
        var climbed: [Set<ObjectIdentifier>] = [[], []]
        for id in Array(r.nodes) {
            guard let start = presenter.views[id] else { continue }
            for chain in 0..<2 {
                let up = { (view: NSView) in chain == 1 ? self.presenter.authoredSuperview(of: view) ?? view.superview : view.superview }
                var ancestor = up(start)
                while let view = ancestor, climbed[chain].insert(ObjectIdentifier(view)).inserted {
                    if let node = view as? NodeView { r.nodes.insert(node.id) }
                    ancestor = up(view)
                }
            }
        }
    }

    #if DEBUG
    /// The guard over the record (debug builds only): a skipped batch
    /// presents anyway, and the bar must show what it showed.
    func shadowCheck() {
        guard ShadowCheck.on, window != nil else { return }
        // The release path's record stays as it was: the guard tests its decisions, never repairs them.
        let before = shadowState(), presented = presents, kept = (reads, inputs)
        shadowing = true
        sync()
        shadowing = false
        presents = presented
        (reads, inputs) = kept
        let after = shadowState()
        ShadowCheck.checks += 1
        guard before != after else { return }
        let diff = zip(before.split(separator: "\n"), after.split(separator: "\n")).first { $0 != $1 }.map { "\($0.0) → \($0.1)" } ?? "\(before.count) → \(after.count) lines"
        ShadowCheck.mismatches.append(diff)
        presenter.session?.log("window bar: a batch its reads skipped changed it (\(diff))")
        FileHandle.standardError.write(Data("exact: window bar shadow mismatch: \(diff)\n".utf8))
        if ShadowCheck.testing { assertionFailure("window bar shadow mismatch: \(diff)") }
    }

    /// What the bar shows, as text: its items and their faces, its order,
    /// the window's title, which header it projects and what it decided of
    /// each, the parts it hides, and where a popover from it is anchored.
    private func shadowState() -> String {
        var lines: [String] = []
        // What is shown where: the projected nodes and the header as the page shows them, hidden or painted.
        lines.append("hidden owner \(owner.map { "\($0.id) \($0.isHidden)" } ?? "-") header \(header.map { "\($0.header.id) \($0.header.isHidden)" } ?? "-") beside \(beside.map { "\($0.header.id) \($0.header.isHidden)" } ?? "-") parts \(besideParts.map { "\($0.node.id) \($0.node.isHidden)" })")
        lines.append("installed \(window?.toolbar === toolbar && toolbar != nil) projected \(projected) title \(window?.title ?? "") subtitle \(window?.subtitle ?? "")")
        lines.append("decided \(headerDecision ?? "-") | \(besideDecision ?? "-") | kept \(headerKept ?? "-")")
        lines.append("header \(header.map { "\($0.header.id) lead \($0.leading.map(\.id)) trail \($0.trailing.map(\.id)) backs \($0.backs.map(\.id))" } ?? "-")")
        lines.append("beside \(beside.map { "\($0.header.id) lead \($0.leading.map(\.id)) trail \($0.trailing.map(\.id))" } ?? "-") parts \(besideParts.map(\.node.id))")
        lines.append("order \(toolbar?.items.map(\.itemIdentifier.rawValue) ?? [])")
        lines.append("back \(backItem.isEnabled) segments \(segments.summary(overflow: false).map { s in "\(s["tablist"] ?? "") \(s["label"] ?? "") \(s["selected"] ?? "") \((s["tabs"] as? [[String: Any]] ?? []).map { "\($0["id"] ?? "")/\($0["title"] ?? "")/\($0["enabled"] ?? "")" })" } ?? "-") \(segments.item?.isEnabled ?? false) \(segments.item?.subitems.map { "\($0.label)/\($0.isEnabled)" } ?? [])")
        lines.append("search \(search.node?.id.description ?? "-") \(search.item.map { "\($0.label) | \($0.searchField.placeholderString ?? "") | \($0.isEnabled) | \($0.searchField.accessibilityLabel() ?? "")" } ?? "-")")
        for id in items.keys.sorted() {
            guard let item = items[id] else { continue }
            let hash = { (image: NSImage?) in image?.tiffRepresentation.map { $0.hashValue } ?? 0 }
            let view: String
            if let button = item.view as? NSButton {
                view = "button \(button.title) \(button.isEnabled) \(button.contentTintColor?.description ?? "-") \(hash(button.image)) \(button.accessibilityLabel() ?? "")"
            } else if let image = item.view as? NSImageView {
                view = "image \(hash(image.image)) \(image.accessibilityLabel() ?? "") \(image.accessibilityIdentifier())"
            } else { view = "-" }
            var style = 0, tint = "-"
            if #available(macOS 26.0, *) { style = item.style.rawValue; tint = item.backgroundTintColor?.description ?? "-" }
            lines.append("item \(id) \(type(of: item)) \(item.label) | \(item.title) | \(item.isEnabled) nav \(item.isNavigational) style \(style) tint \(tint) image \(hash(item.image)) tip \(item.toolTip ?? "") view \(view)")
        }
        // What VoiceOver is told of each item (`syncAccessibility`, from the item and its node), not
        // AppKit's elements, which exist only after a layout the guard's present must not force.
        for id in items.keys.sorted() {
            guard let item = items[id], let node = presenter.views[id] else { continue }
            lines.append("ax \(id) \(item.label) \(item.isEnabled && !node.ariaUnavailable)")
        }
        return lines.joined(separator: "\n")
    }
    #endif
}

extension Presenter {
    /// The accent or another system colour changed (LLP 1095 D5): every view applies its colours again.
    func systemColorsChanged() {
        views.values.forEach { $0.systemColorsChanged() }
        // A prominent item's tint is resolved when the bar presents (`prominence`): no batch says it moved.
        toolbar.sync()
    }
}

#if DEBUG
/// The shadow guard's switch and its record (debug builds only).
enum ShadowCheck {
    static let testing = NSClassFromString("XCTestCase") != nil
    static var on = ProcessInfo.processInfo.environment["EXACT_BAR_SHADOW"].map { $0 == "1" } ?? testing
    static var checks = 0 { didSet { if oldValue == 0 { atexit { ShadowCheck.report() } } } }
    static var mismatches: [String] = []
    static func report() {
        FileHandle.standardError.write(Data("exact: window bar shadow: \(checks) skipped batches presented again, \(mismatches.count) changed the bar\n".utf8))
    }
}
#endif
#endif
