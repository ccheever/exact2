// A virtualized list's retired rows lend their views to the rows it builds
// next (LLP 1050.000's reuse, host side). The runner never reuses a view id
// (`runner/src/instance/collection/api.rs`): a row it retires is destroyed
// and the next is created under new ids. Building a row's views costs most
// of a row in UIKit — making each view and moving it into the window, where
// every view is visited again (traits, tint, registration) — and destroying
// one costs the same walk out. So a collection row whose whole subtree the
// batch destroys is parked instead: its views stay where they were, the root
// hidden, each one forgotten by the presenter and reset (`recycle`). A later
// row created in a batch, under a collection list, with the same shape (the
// kinds and child counts, in order) takes the parked views under its new ids
// (`rebind`) and applies its create ops to them. A parked tree never leaves
// its list, so nothing moves in or out of the window.
//
// The reset contract, what a reused view must not carry from its last row:
// - identity: the presenter's maps and indexes for the old id (`release`,
//   the destroy path's own), autofocus, the ChromeIndex's names;
// - props (cleared, so a prop the new row lacks is absent), style (replaced
//   whole by the create op), handlers (assigned; their recognizers follow);
// - geometry and presentation: frame, transform, translate/scale/rotate,
//   opacity, hidden; `content`; the create's frame and present ops set them;
// - paint: the text raster, paragraph and inline runs (the new row's
//   `paragraph` op sets its own), layout caches, the live-region text, the
//   layer's bitmap; a raster image, its sublayer and its load (the
//   generation moves, so no completion lands); the symbol's key (its size is
//   reported again for the new id), keeping the glyph view unless the new
//   node shows no symbol;
// - input and accessibility: interaction enabled, the element, traits,
//   label, value, hint, identifier, hidden-elements, all set again by props;
// - motion: an `svg`'s scene layers and every Core Animation spec the view
//   ran (`SvgHost.forget`, on release), so no animation of the old row
//   plays on the new one (LLP 1055 D8; the new row's scene op starts its
//   own). SVG elements are not views: an `svg` parks as one leaf.
// - identity (LLP 1068 §4.9): the view's `incarnation`, zeroed when it
//   parks, before the reset, and a never-used one issued at the take, in
//   the batch that installs its props and handlers; an asynchronous
//   callback captures it and is dropped when it changed.
// Heavy leaves (LLP 1068 §4.0): a video, web view, native-module view,
// canvas, input or text area with no node children is a hole in its row's
// shape. The row's other views park; the leaf is destroyed as any destroy
// goes, before they park, and the next row's leaf is built fresh by its
// create op. A material's effect view is dropped at park and made again
// from the next row's props (§4.1). Nothing heavy is reused.
// What makes a view ineligible instead of reset: a kind other than plain
// boxes, text, images, buttons, `svg`s, waiting scrolls and heavy leaves; a
// live UIScrollView; a platform subview other than the node's glyph, its
// material or its clip box; a placement; gesture recognizers or
// interactions (menus, reorder handles, drags) other than a context menu's
// at rest (LLP 1021 §5.1, `contextual`); a press, drag or swipe in
// progress, or a projected swipe row; focus, a focus ring, editing, a
// pending focus — on a heavy leaf too; flow shapes, a context transform, a
// pending scroll; a view kept by a modal's retiring root; a subtree node
// the batch does not destroy (it may be moving elsewhere).
// While VoiceOver or Switch Control runs nothing parks: its cursor stays on
// the element it was on, never on a view that is now another row.
#if os(iOS) || os(tvOS)
import UIKit

final class NodePool {
    unowned let presenter: Presenter
    init(_ presenter: Presenter) {
        self.presenter = presenter
        // Memory pressure takes every parked tree (LLP 1068 §6).
        memory = NotificationCenter.default.addObserver(forName: UIApplication.didReceiveMemoryWarningNotification, object: nil, queue: .main) { [weak self] _ in
            self?.reset()
        }
    }
    deinit { if let memory { NotificationCenter.default.removeObserver(memory) } }
    private var memory: NSObjectProtocol?

    /// Parked trees by shape, each in preorder with its root first and a
    /// hole (nil) for each heavy leaf, the sources its images showed (a tree
    /// showing the new row's symbols again sets no image: UIKit resolves
    /// each one it is given), and when it parked.
    /// `list`: an inner list's card, parked under that list's view (LLP 1068
    /// §4.2.1), counted apart from the rows.
    private struct Tree { let views: [NodeView?]; let images: String; let parked: UInt64; var list: ObjectIdentifier? = nil }
    private var parked: [String: [Tree]] = [:]
    private var roots = Set<ObjectIdentifier>()
    private(set) var count = 0
    private var generation: UInt64 = 0
    /// Enough for the rows one fill slice retires before the next builds.
    static let perShape = 8, capacity = 32
    /// Cards parked under inner lists (LLP 1068 §4.2.1): per list and in all.
    static let perList = 12, innerCapacity = 96
    private(set) var innerCount = 0
    static let kinds: Set<String> = ["view", "text", "image", "button", "scroll", "svg"]
    /// Kinds whose platform view is heavy: a row pools around them (LLP 1068 §4.0).
    /// A 2D canvas (`canvas2d`, LLP 1056 D10) is one until stage 3 pools its
    /// bitmap: its view and replayer go with the row's node, and the next
    /// row's canvas is a new view drawn by its own lifetime's lists. A
    /// control (a switch, a checkbox, a select, a slider, a date, a native
    /// button, LLP 1069.011.000 D8) is one too: its node and its `UIControl`
    /// go as an unpooled control goes, and the next row's is built fresh,
    /// so no value, touch or callback crosses rows.
    static let leaves: Set<String> = ["video", "iframe", "native", "canvas", "canvas2d", "input", "textarea", "control"]
    /// What `state` reports (LLP 1068 §6): parks, takes, evictions, and the
    /// heavy leaves destroyed at a park and built at a take, by kind.
    private(set) var parks = 0, takes = 0, evictions = 0
    private(set) var leavesDropped: [String: Int] = [:], leavesBuilt: [String: Int] = [:]

    /// The last incarnation issued (LLP 1068 §4.9); zero is never issued.
    private static var issued: UInt64 = 0
    static func issue() -> UInt64 { issued += 1; return issued }

    private var batch: Batch?
    private var destroyed: Set<UInt32>?
    private var created: (kind: [UInt32: String], children: [UInt32: [UInt32]], parent: [UInt32: UInt32], image: [UInt32: String])?
    private var claims: [UInt32: NodeView] = [:]
    private var tried = Set<UInt32>()

    /// Batches applied inside a batch (`Presenter.applying`) neither park
    /// nor take: the outer batch's structure is not theirs.
    private var depth = 0

    func begin(_ batch: Batch) {
        depth += 1
        guard depth == 1 else { return }
        self.batch = batch; destroyed = nil; created = nil; claims.removeAll(); tried.removeAll()
    }
    func end() {
        depth -= 1
        guard depth == 0 else { return }
        // Every node of a claimed tree has a create op, so none is left; if
        // one were, it would be a hidden stray: it goes.
        for view in claims.values { view.forget(); view.removeFromSuperview() }
        batch = nil; destroyed = nil; created = nil; claims.removeAll(); tried.removeAll()
        // A tree whose list left the window goes with it.
        for (shape, trees) in parked {
            let kept = trees.filter { $0.views[0]!.window != nil }
            if kept.count == trees.count { continue }
            for tree in trees where tree.views[0]!.window == nil { drop(tree) }
            parked[shape] = kept.isEmpty ? nil : kept
        }
        // A reused view the batch gave no frame has a fresh view's.
        for view in unframed.values where view.frame != .zero { view.frame = .zero }
        unframed.removeAll()
    }
    /// Reused views, until the batch frames them.
    private var unframed: [UInt32: NodeView] = [:]
    func framed(_ id: UInt32) { if !unframed.isEmpty { unframed.removeValue(forKey: id) } }
    func isParked(_ view: UIView) -> Bool { !roots.isEmpty && roots.contains(ObjectIdentifier(view)) }
    func reset() {
        for tree in parked.values.joined() { drop(tree) }
        unframed.removeAll()
        parked.removeAll()
    }
    /// A tree leaves the pool: its views are forgotten and its root leaves the list.
    private func drop(_ tree: Tree) {
        let root = tree.views[0]!
        roots.remove(ObjectIdentifier(root))
        if tree.list != nil { innerCount -= 1 } else { count -= 1 }
        #if os(iOS)
        for case let view? in tree.views { presenter.menus.context.dropped(view) }
        #endif
        for view in tree.views { view?.forget() }
        root.removeFromSuperview()
    }
    /// Room for one more tree of `shape`: the least recently parked tree
    /// goes, of that shape when it is full, else of any shape.
    private func makeRoom(for shape: String) {
        var victim: (shape: String, index: Int)?
        if let trees = parked[shape], trees.filter({ $0.list == nil }).count >= Self.perShape {
            victim = trees.firstIndex { $0.list == nil }.map { (shape, $0) }
        } else if count >= Self.capacity {
            let oldest = parked.compactMap { key, trees in trees.first { $0.list == nil }.map { (key, $0) } }
                .min { $0.1.parked < $1.1.parked }
            victim = oldest.flatMap { key, tree in parked[key]?.firstIndex { $0.parked == tree.parked }.map { (key, $0) } }
        }
        guard let victim, var trees = parked[victim.shape] else { return }
        let tree = trees.remove(at: victim.index)
        parked[victim.shape] = trees.isEmpty ? nil : trees
        drop(tree)
        evictions += 1
    }

    /// The list `root` is a row of, when a collection owns it.
    private func list(holding root: UIView) -> NodeView? {
        guard let scroll = root.superview as? ScrollView, let list = scroll.superview as? NodeView,
              list.scroll === scroll, presenter.collections.owns(list.id) else { return nil }
        return list
    }
    /// `view`'s node children, through its logical container (its clip box,
    /// a glass's content view); nil when the view holds a platform subview
    /// other than its glyph, material or clip box, or a live scroll view.
    private func children(_ view: NodeView) -> [NodeView]? {
        guard view.scroll == nil, view.overlay == nil else { return nil }
        let container = view.container
        func own(_ sub: UIView) -> Bool { sub === view.symbolView || sub === view.materialView || sub === view.clipBox || sub === view.glassGroupView || sub === view.glassSlot }
        if container !== view {
            guard view.subviews.allSatisfy({ own($0) || ($0 === container) }) else { return nil }
        }
        var out: [NodeView] = []
        for sub in container.subviews where !(container === view && own(sub)) {
            guard let child = sub as? NodeView else { return nil }
            out.append(child)
        }
        return out
    }
    /// `view`'s shape, its views appended in preorder with a hole (nil) for
    /// each heavy leaf, which goes to `leaves`; nil if it cannot park.
    private func shape(_ view: NodeView, _ views: inout [NodeView?], _ leaves: inout [NodeView],
                       _ lists: inout [NodeView]) -> String? {
        // An inner list at rest parks with its row; its content is not part
        // of the row's shape (LLP 1068 §4.2.1).
        if view.kind == "list", view.scroll != nil {
            guard !lists.isEmpty || !views.isEmpty, atRest(view) else { return nil }
            views.append(view); lists.append(view)
            return "list[]"
        }
        if Self.leaves.contains(view.kind) {
            let inside: UIView = view.overlay ?? view
            guard !inside.subviews.contains(where: { $0 is NodeView }) else { return nil }
            views.append(nil); leaves.append(view)
            return view.kind + "*"
        }
        guard Self.kinds.contains(view.kind), let children = children(view) else { return nil }
        views.append(view)
        var shape = view.kind + "("
        for child in children {
            guard let inner = self.shape(child, &views, &leaves, &lists) else { return nil }
            shape += inner
        }
        return shape + ")"
    }
    /// An inner list that nothing moves: not under the reader's hand,
    /// decelerating or animating, not being corrected, no refresh running,
    /// unzoomed, no authored scroll pending (LLP 1068 §4.2.1).
    private func atRest(_ list: NodeView) -> Bool {
        guard let sv = list.scroll else { return false }
        #if os(tvOS)
        // tvOS has no refresh control.
        return !sv.isTracking && !sv.isDragging && !sv.isDecelerating && !presenter.collections.correcting
            && sv.zoomScale == 1
            && list.pendingScrollTop == nil && list.pendingScrollLeft == nil
            && sv.layer.animationKeys()?.isEmpty ?? true
        #else
        return !sv.isTracking && !sv.isDragging && !sv.isDecelerating && !presenter.collections.correcting
            && sv.refreshControl?.isRefreshing != true && sv.zoomScale == 1
            && list.pendingScrollTop == nil && list.pendingScrollLeft == nil
            && sv.layer.animationKeys()?.isEmpty ?? true
        #endif
    }
    private func destroyedIDs() -> Set<UInt32> {
        if let destroyed { return destroyed }
        let ids = Set(batch?.ops.lazy.filter { $0.op == .destroy }.map(\.id) ?? [])
        destroyed = ids
        return ids
    }

    /// `root` is being destroyed: park its views and answer true, or answer
    /// false and leave the destroy to the caller. Nothing is destroyed or
    /// reset until the whole subtree has been found eligible.
    func retire(_ root: NodeView) -> Bool {
        guard depth == 1, presenter.reorder == nil, !presenter.swipeActions.assistive,
              !Self.leaves.contains(root.kind), list(holding: root) != nil,
              root.canvasAbove == nil, !presenter.modals.retainsRemovedView(root) else { return false }
        return park(root, under: nil)
    }
    /// Park `root`'s tree: a collection's row (`under` nil), or an inner
    /// list's card (LLP 1068 §4.2.1). False, and nothing touched, when any of
    /// it is ineligible.
    private func park(_ root: NodeView, under list: NodeView?) -> Bool {
        var views: [NodeView?] = [], leaves: [NodeView] = [], lists: [NodeView] = []
        guard let shape = shape(root, &views, &leaves, &lists), list == nil || lists.isEmpty else { return false }
        let destroyed = destroyedIDs()
        guard views.allSatisfy({ $0.map { destroyed.contains($0.id) && recyclable($0) } ?? true }),
              leaves.allSatisfy({ destroyed.contains($0.id) && idle($0) && unhooked($0) }) else { return false }
        // The cards an inner list shows park under it before the list itself
        // is reset; the rest of its content goes by its own destroy ops.
        var cards: [(NodeView, NodeView)] = []
        for inner in lists {
            // A spacer (a box with no node children) is not a card.
            for case let card as NodeView in inner.scroll?.subviews ?? []
                where destroyed.contains(card.id) && card.container.subviews.contains(where: { $0 is NodeView }) {
                cards.append((card, inner))
            }
        }
        // The heavy leaves go as any destroyed view goes (their arms run
        // unchanged), then the rest park.
        for leaf in leaves {
            leavesDropped[leaf.kind, default: 0] += 1
            let gone = presenter.release(leaf.id) { $0.forget() }
            gone?.removeFromSuperview()
        }
        if let list { makeRoom(under: list) } else { makeRoom(for: shape) }
        let images = views.lazy.compactMap { $0 }.filter { $0.kind == "image" }.map { $0.imageSource ?? "" }.joined(separator: "|")
        for case let view? in views {
            view.incarnation = 0
            #if os(iOS)
            presenter.menus.context.parked(view)
            #endif
            presenter.release(view.id) { $0.recycle() }
        }
        root.isHidden = true
        // Past the rows, so the list's next children op moves none of them.
        root.superview?.bringSubviewToFront(root)
        generation += 1
        parked[shape, default: []].append(Tree(views: views, images: images, parked: generation, list: list.map(ObjectIdentifier.init)))
        roots.insert(ObjectIdentifier(root)); parks += 1
        if list != nil { innerCount += 1 } else { count += 1 }
        for (card, inner) in cards where parkedCards(under: inner) < Self.perList { _ = park(card, under: inner) }
        for inner in lists { inner.recycleScroll() }
        return true
    }
    private func parkedCards(under list: NodeView) -> Int {
        let key = ObjectIdentifier(list)
        return parked.values.joined().filter { $0.list == key }.count
    }
    /// Room for one more card under `list`: its least recently parked card
    /// goes when the list holds 12, the oldest card of any list at 96.
    private func makeRoom(under list: NodeView) {
        let key = ObjectIdentifier(list)
        let cards = parked.flatMap { shape, trees in trees.filter { $0.list != nil }.map { (shape, $0) } }
        let mine = cards.filter { $0.1.list == key }
        let victim = mine.count >= Self.perList ? mine.min { $0.1.parked < $1.1.parked }
            : innerCount >= Self.innerCapacity ? cards.min { $0.1.parked < $1.1.parked } : nil
        guard let (shape, tree) = victim, var trees = parked[shape],
              let index = trees.firstIndex(where: { $0.parked == tree.parked }) else { return }
        trees.remove(at: index)
        parked[shape] = trees.isEmpty ? nil : trees
        drop(tree)
        evictions += 1
    }
    /// Nothing about the view outlives its row's reset.
    private func recyclable(_ v: NodeView) -> Bool {
        presenter.views[v.id] === v && (v.scroll == nil || v.kind == "list") && !v.scrollNeeded
            && v.field == nil && v.textArea == nil && v.video == nil && v.web == nil && v.metal == nil
            && v.overlay == nil && v.canvasInput == nil
            && idle(v)
            && v.swipeHold == nil && v.heightHold == nil && v.reorderHold == nil && v.transformHold == nil
            && !v.pressed && (v.gestureRecognizers ?? []).allSatisfy({ ($0 as? PointerRecognizer)?.idle == true || contextual($0, of: v) })
            && v.interactions.allSatisfy({ contextual($0, of: v) })
            && v.flowShapes.isEmpty && v.contextTransform.isIdentity
            && v.pendingScrollLeft == nil && v.pendingScrollTop == nil
            // A hooked node's view is the app's to keep (LLP 1075.003.000),
            // unless its hook undoes what it adds (`reusable`, LLP
            // 1075.003.000.000 §8); the checks above still refuse a view
            // with interactions or gestures left on it. Its props and the mark
            // stay until it is forgotten, after its own destroy op.
            && unhooked(v)
    }
    /// Not a hooked node, or one whose hook undoes what it adds: a hooked
    /// heavy leaf is destroyed as any is, but its row stays out of the pool
    /// as the journal says (LLP 1075.003.000 §3.3).
    private func unhooked(_ v: NodeView) -> Bool { v.props["hook"] == nil || v.hookReusable }
    /// Not placed, focused, editing or about to be: a leaf so held keeps
    /// its row out of the pool, destroyed as before.
    /// A context menu's own (LLP 1021 §5.1): the node's long-press
    /// recognizer at rest, and its menu's interaction while no menu shows
    /// from it. They follow the node's handlers and props, so the next row's
    /// create ops and the menu host's sync set them again (`park` takes the
    /// interaction off first). A row whose menu is up is destroyed as before,
    /// and the menu ends as any unmounted source's does.
    private func contextual(_ recognizer: UIGestureRecognizer, of v: NodeView) -> Bool {
        // `.possible` holds while a touch is still being judged (a long press
        // waiting out its duration): at rest means no touch as well.
        guard recognizer.state == .possible, recognizer.numberOfTouches == 0 else { return false }
        #if os(iOS)
        if presenter.menus.context.owns(recognizer, on: v) { return true }
        #endif
        return recognizer === v.contextRecognizer
    }
    private func contextual(_ interaction: UIInteraction, of v: NodeView) -> Bool {
        #if os(iOS)
        return presenter.menus.context.owns(interaction, on: v) && presenter.menus.context.open?.source !== v
        #else
        return false
        #endif
    }
    private func idle(_ v: NodeView) -> Bool {
        presenter.views[v.id] === v && v.placement == nil && !v.placementHidden && v.focusRing == nil
            && !v.isFirstResponder && v.field?.isFirstResponder != true && v.textArea?.isFirstResponder != true
            && presenter.editing !== v && presenter.pendingFocusNode !== v
    }

    /// This batch's created nodes: kinds, children, parents, image sources.
    private func createdNodes() -> (kind: [UInt32: String], children: [UInt32: [UInt32]], parent: [UInt32: UInt32], image: [UInt32: String])? {
        if let created { return created }
        guard let batch else { return nil }
        var kind: [UInt32: String] = [:], children: [UInt32: [UInt32]] = [:], parent: [UInt32: UInt32] = [:]
        var image: [UInt32: String] = [:]
        var flat = Set<UInt32>()
        for op in batch.ops where op.op == .create {
            // A flat leaf is a layer, not a view: no part of a row's shape
            // (LLP 1068 §6.1), as a parked row's subviews do not hold it.
            if presenter.flats.batchFlat.contains(op.id) { flat.insert(op.id); continue }
            kind[op.id] = op.kind
            if op.kind == "image" { image[op.id] = op.props["imageSource"] ?? "" }
        }
        for op in batch.ops where op.op == .children {
            let ids = op.ids.map { UInt32($0) }.filter { !flat.contains($0) }
            if kind[op.id] != nil { children[op.id] = ids }
            for child in ids where kind[child] != nil { parent[child] = op.id }
        }
        created = (kind, children, parent, image)
        return created
    }
    /// The collection list a node this batch creates will be a row of, and
    /// its row's root (LLP 1068 §5.1 asks which list a new leaf is in).
    func list(creating id: UInt32) -> NodeView? {
        guard depth >= 1, let c = createdNodes() else { return nil }
        var root = id
        while let up = c.parent[root], c.kind[up] != nil { root = up }
        guard let list = c.parent[root].flatMap({ presenter.views[$0] }), list.kind == "list",
              presenter.collections.owns(list.id) else { return nil }
        return list
    }

    /// A parked view for `id`, which this batch creates: the new subtree `id`
    /// belongs to claims a tree of its shape at its first create, if its
    /// parent is a collection's list and every node in it is new. A heavy
    /// leaf's id is a hole: nil, and the caller builds it.
    func take(_ id: UInt32) -> NodeView? {
        if let view = claims.removeValue(forKey: id) { return rebound(view, id) }
        guard depth == 1, count + innerCount > 0, batch != nil, let c = createdNodes() else { return nil }
        var root = id
        // An inner list's cards claim their own trees (LLP 1068 §4.2.1).
        while let up = c.parent[root], c.kind[up] != nil, !presenter.collections.owns(up) { root = up }
        guard tried.insert(root).inserted, let list = c.parent[root].flatMap({ presenter.views[$0] }),
              list.kind == "list", presenter.collections.owns(list.id),
              let kind = c.kind[root], !Self.leaves.contains(kind) else { return nil }
        var ids: [UInt32] = [], holes: [String] = []
        func shape(_ node: UInt32) -> String? {
            guard let kind = c.kind[node] else { return nil }
            ids.append(node)
            if kind == "list", node != root, presenter.collections.owns(node) { return "list[]" }
            if Self.leaves.contains(kind) {
                guard c.children[node]?.isEmpty ?? true else { return nil }
                holes.append(kind)
                return kind + "*"
            }
            guard Self.kinds.contains(kind) else { return nil }
            var s = kind + "("
            for child in c.children[node] ?? [] {
                guard let inner = shape(child) else { return nil }
                s += inner
            }
            return s + ")"
        }
        guard let key = shape(root), var trees = parked[key], !trees.isEmpty else { return nil }
        let images = ids.lazy.compactMap { c.image[$0] }.joined(separator: "|")
        // A card prefers one parked under its own (reused) list view.
        let here = list.scroll.map(ObjectIdentifier.init)
        let mine = trees.indices.filter { trees[$0].list != nil && trees[$0].views[0]!.superview.map(ObjectIdentifier.init) == here }
        // In the order they parked: a list's cards come back where they were.
        let pick = mine.first { trees[$0].images == images } ?? mine.first
            ?? trees.lastIndex { $0.images == images } ?? trees.count - 1
        let tree = trees.remove(at: pick)
        parked[key] = trees.isEmpty ? nil : trees
        roots.remove(ObjectIdentifier(tree.views[0]!)); takes += 1
        if tree.list != nil { innerCount -= 1 } else { count -= 1 }
        for kind in holes { leavesBuilt[kind, default: 0] += 1 }
        for (new, view) in zip(ids, tree.views) {
            guard let view else { continue }
            claims[new] = view; unframed[new] = view
        }
        return claims.removeValue(forKey: id).map { rebound($0, id) }
    }
    private func rebound(_ view: NodeView, _ id: UInt32) -> NodeView { view.rebind(id); return view }

    /// What `layout agree` checks (LLP 1080.001 D2): each parked tree's root,
    /// which must stay hidden, and every view of every parked tree.
    var inspection: (roots: [NodeView], members: Set<ObjectIdentifier>) {
        var roots: [NodeView] = [], members = Set<ObjectIdentifier>()
        for tree in parked.values.joined() {
            if let root = tree.views.first ?? nil { roots.append(root) }
            for case let view? in tree.views { members.insert(ObjectIdentifier(view)) }
        }
        return (roots, members)
    }

    /// `state`'s pool section (LLP 1068 §6): what is parked, by shape
    /// count and in total, and the counters since launch.
    var observation: [String: Any] {
        ["parked": count, "shapes": parked.count, "capacity": Self.capacity, "perShape": Self.perShape,
         "parks": parks, "takes": takes, "evictions": evictions,
         "leavesDropped": leavesDropped, "leavesBuilt": leavesBuilt]
    }
}

extension NodeView {
    /// Parked (`NodePool`): what `forget` drops, except the view's presenter
    /// and its symbol glyph view, which the next row reuses.
    func recycle() {
        presenter?.forgetParagraph(self)
        cancelSurfaceControls()
        invalidateText()
        cachedTextLayout = nil
        dropTextRaster()
        liveText = nil
        loadGeneration += 1
        presenter?.session?.rasters.cancel(id)
        // A symbol keeps its glyph and key: the same symbol again costs no
        // image, and `finishReuse` reports its size under the new id.
        raster = nil; imageSource = nil
        if symbolView == nil { image = nil }
        symbolRefusal = nil
        inlinePressed = nil
        // The next node's hook decides again.
        hookReusable = false
        content = .zero
        needsCapture = false; paintedThisTurn = false
        // A parked view keeps no bitmap: its create ops paint it again.
        if layer.contents != nil { layer.contents = nil }
        // Its material goes, its children back in the node (LLP 1068 §4.1):
        // the next row's props make a new one.
        if materialView != nil { props["backgroundMaterial"] = nil; updateMaterial() }
        // `glassGroupAuto` with it: a row recycled onto a numeric group must
        // not report the old one's `auto` (LLP 1053.000.000.000).
        if glassGroupView != nil { props["glassGroup"] = nil; props["glassGroupAuto"] = nil; syncGlassGroup() }
    }
    /// An inner list parked with its row (LLP 1068 §4.2.1): back to a new
    /// list's scroll, with its delegate detached while it resets.
    func recycleScroll() {
        guard let sv = scroll else { return }
        let delegate = sv.delegate
        sv.delegate = nil
        sv.setContentOffset(.zero, animated: false)
        sv.contentSize = .zero
        sv.delegate = delegate
        content = .zero
        if let e = extras {
            e.beforeLayoutScroll = nil; e.hiddenScroll = nil; e.followedScroll = nil
            e.anchoredScrollTop = nil; e.retainedScrollTop = nil; e.lastScrollEvent = .zero
            e.readingAnchors.removeAll(); e.activeReadingAnchor = nil
            e.pendingScrollTop = nil; e.pendingScrollLeft = nil
        }
    }
    /// Taken for `newID`: a fresh view's state, before its create ops, and
    /// a never-used incarnation (LLP 1068 §4.9). The create ops that install
    /// its props and handlers run in this batch, before any callback can.
    func rebind(_ newID: UInt32) {
        id = newID
        incarnation = NodePool.issue()
        // UIKit's setters are not free, even to the same value.
        if isHidden { isHidden = false }
        if alpha != 1 { alpha = 1 }
        translatePx = .zero; translatePercent = .zero; layoutOffset = .zero; layoutScale = CGPoint(x: 1, y: 1); endSurface(); scale = 1; rotate = 0; press = PressFeedback()
        if !transform.isIdentity { transform = .identity }
        if !isUserInteractionEnabled { isUserInteractionEnabled = true }
        if isAccessibilityElement { isAccessibilityElement = false }
        if accessibilityTraits != [] { accessibilityTraits = [] }
        if accessibilityLabel != nil { accessibilityLabel = nil }
        if accessibilityValue != nil { accessibilityValue = nil }
        if accessibilityHint != nil { accessibilityHint = nil }
        if accessibilityIdentifier != nil { accessibilityIdentifier = nil }
        if accessibilityElementsHidden { accessibilityElementsHidden = false }
        if accessibilityViewIsModal { accessibilityViewIsModal = false }
        props = [:]
        // The presenter's indexes for the new id (LLP 1068 §4.0): each is
        // filled by a property observer a rebind does not fire.
        if scroll != nil { presenter?.scrollers.insert(newID) }
        if materialView != nil { presenter?.materialNodes.insert(newID) }
        if !contextTransform.isIdentity { presenter?.contextNodes.insert(newID) }
    }
    /// After its create ops: a node that shows no symbol keeps no glyph
    /// view; one that shows the same symbol as before reports its size for
    /// its new id (a changed symbol already did).
    func finishReuse() {
        guard kind == "image" else { return }
        if imageSource?.hasPrefix("symbol:") != true { clearSymbol(); image = nil; return }
        if symbolView != nil { presenter?.queueIntrinsicSize(self, generation: loadGeneration, image?.size) }
    }
}
#endif
