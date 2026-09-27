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
//   label, value, hint, identifier, hidden-elements, all set again by props.
// What makes a view ineligible instead of reset: a kind other than plain
// boxes, text, images, buttons and waiting scrolls; a live UIScrollView, an
// input, a text area, a canvas, video or web view, a material, a placement;
// gesture recognizers or interactions (menus, reorder handles, drags); a
// press, drag or swipe in progress, or a projected swipe row; focus, a focus
// ring, editing, a pending focus; flow shapes, a context transform, a
// pending scroll; a view kept by a modal's retiring root; a
// subtree node the batch does not destroy (it may be moving elsewhere).
// While VoiceOver or Switch Control runs nothing parks: its cursor stays on
// the element it was on, never on a view that is now another row.
#if os(iOS)
import UIKit

final class NodePool {
    unowned let presenter: Presenter
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Parked trees by shape, each in preorder with its root first, and the
    /// sources its images showed: a tree showing the new row's symbols
    /// again sets no image (UIKit resolves each one it is given).
    private struct Tree { let views: [NodeView]; let images: String }
    private var parked: [String: [Tree]] = [:]
    private var roots = Set<ObjectIdentifier>()
    private(set) var count = 0
    /// Enough for the rows one fill slice retires before the next builds.
    static let perShape = 8, capacity = 32
    static let kinds: Set<String> = ["view", "text", "image", "button", "scroll"]

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
            let kept = trees.filter { $0.views[0].window != nil }
            if kept.count == trees.count { continue }
            for tree in trees where tree.views[0].window == nil { drop(tree.views) }
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
        for tree in parked.values.joined() { drop(tree.views); tree.views[0].removeFromSuperview() }
        unframed.removeAll()
        parked.removeAll()
    }
    private func drop(_ tree: [NodeView]) {
        roots.remove(ObjectIdentifier(tree[0])); count -= 1
        for view in tree { view.forget() }
    }

    /// The list `root` is a row of, when a collection owns it.
    private func list(holding root: UIView) -> NodeView? {
        guard let scroll = root.superview as? ScrollView, let list = scroll.superview as? NodeView,
              list.scroll === scroll, presenter.collections.owns(list.id) else { return nil }
        return list
    }
    /// `root`'s shape, its views appended in preorder; nil if a subview is
    /// not a node (or the node's own glyph) or a node has a container of its own.
    private func shape(_ view: NodeView, _ views: inout [NodeView]) -> String? {
        guard Self.kinds.contains(view.kind), view.container === view else { return nil }
        views.append(view)
        var shape = view.kind + "("
        for sub in view.subviews where sub !== view.symbolView {
            guard let child = sub as? NodeView, let inner = self.shape(child, &views) else { return nil }
            shape += inner
        }
        return shape + ")"
    }
    private func destroyedIDs() -> Set<UInt32> {
        if let destroyed { return destroyed }
        let ids = Set(batch?.ops.lazy.filter { $0.op == .destroy }.map(\.id) ?? [])
        destroyed = ids
        return ids
    }

    /// `root` is being destroyed: park its views and answer true, or answer
    /// false and leave the destroy to the caller.
    func retire(_ root: NodeView) -> Bool {
        guard depth == 1, count < Self.capacity, presenter.reorder == nil, !presenter.swipeActions.assistive,
              list(holding: root) != nil,
              root.canvasAbove == nil, !presenter.modals.retainsRemovedView(root) else { return false }
        var views: [NodeView] = []
        guard let shape = shape(root, &views), (parked[shape]?.count ?? 0) < Self.perShape else { return false }
        let destroyed = destroyedIDs()
        guard views.allSatisfy({ destroyed.contains($0.id) && recyclable($0) }) else { return false }
        let images = views.lazy.filter { $0.kind == "image" }.map { $0.imageSource ?? "" }.joined(separator: "|")
        for view in views { presenter.release(view.id) { $0.recycle() } }
        root.isHidden = true
        // Past the rows, so the list's next children op moves none of them.
        root.superview?.bringSubviewToFront(root)
        parked[shape, default: []].append(Tree(views: views, images: images))
        roots.insert(ObjectIdentifier(root)); count += 1
        return true
    }
    private func recyclable(_ v: NodeView) -> Bool {
        presenter.views[v.id] === v && v.scroll == nil && !v.scrollNeeded
            && v.field == nil && v.textArea == nil && v.video == nil && v.web == nil && v.metal == nil
            && v.overlay == nil && v.canvasInput == nil && v.materialView == nil
            && v.placement == nil && !v.placementHidden && v.focusRing == nil
            && !v.isFirstResponder && presenter.editing !== v && presenter.pendingFocusNode !== v
            && v.swipeHold == nil && v.heightHold == nil && v.reorderHold == nil && v.transformHold == nil
            && !v.pressed && (v.gestureRecognizers?.isEmpty ?? true) && v.interactions.isEmpty
            && v.flowShapes.isEmpty && v.contextTransform.isIdentity
            && v.pendingScrollLeft == nil && v.pendingScrollTop == nil
    }

    /// A parked view for `id`, which this batch creates: the new subtree `id`
    /// belongs to claims a tree of its shape at its first create, if its
    /// parent is a collection's list and every node in it is new.
    func take(_ id: UInt32) -> NodeView? {
        if let view = claims.removeValue(forKey: id) { return rebound(view, id) }
        guard depth == 1, count > 0, let batch else { return nil }
        let c = created ?? {
            var kind: [UInt32: String] = [:], children: [UInt32: [UInt32]] = [:], parent: [UInt32: UInt32] = [:]
            var image: [UInt32: String] = [:]
            for op in batch.ops where op.op == .create {
                kind[op.id] = op.kind
                if op.kind == "image" { image[op.id] = op.props["imageSource"] ?? "" }
            }
            for op in batch.ops where op.op == .children {
                let ids = op.ids.map { UInt32($0) }
                if kind[op.id] != nil { children[op.id] = ids }
                for child in ids where kind[child] != nil { parent[child] = op.id }
            }
            return (kind, children, parent, image)
        }()
        created = c
        var root = id
        while let up = c.parent[root], c.kind[up] != nil { root = up }
        guard tried.insert(root).inserted, let list = c.parent[root].flatMap({ presenter.views[$0] }),
              list.kind == "list", presenter.collections.owns(list.id) else { return nil }
        var ids: [UInt32] = []
        func shape(_ node: UInt32) -> String? {
            guard let kind = c.kind[node], Self.kinds.contains(kind) else { return nil }
            ids.append(node)
            var s = kind + "("
            for child in c.children[node] ?? [] {
                guard let inner = shape(child) else { return nil }
                s += inner
            }
            return s + ")"
        }
        guard let key = shape(root), var trees = parked[key], !trees.isEmpty else { return nil }
        let images = ids.lazy.compactMap { c.image[$0] }.joined(separator: "|")
        let tree = trees.remove(at: trees.lastIndex { $0.images == images } ?? trees.count - 1)
        parked[key] = trees.isEmpty ? nil : trees
        roots.remove(ObjectIdentifier(tree.views[0])); count -= 1
        for (new, view) in zip(ids, tree.views) { claims[new] = view; unframed[new] = view }
        return claims.removeValue(forKey: id).map { rebound($0, id) }
    }
    private func rebound(_ view: NodeView, _ id: UInt32) -> NodeView { view.rebind(id); return view }
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
        content = .zero
        needsCapture = false; paintedThisTurn = false
        // A parked view keeps no bitmap: its create ops paint it again.
        if layer.contents != nil { layer.contents = nil }
    }
    /// Taken for `newID`: a fresh view's state, before its create ops.
    func rebind(_ newID: UInt32) {
        id = newID
        // UIKit's setters are not free, even to the same value.
        if isHidden { isHidden = false }
        if alpha != 1 { alpha = 1 }
        translate = .zero; layoutOffset = .zero; scale = 1; rotate = 0; press = PressFeedback()
        if !transform.isIdentity { transform = .identity }
        if !isUserInteractionEnabled { isUserInteractionEnabled = true }
        if isAccessibilityElement { isAccessibilityElement = false }
        if accessibilityTraits != [] { accessibilityTraits = [] }
        if accessibilityLabel != nil { accessibilityLabel = nil }
        if accessibilityValue != nil { accessibilityValue = nil }
        if accessibilityHint != nil { accessibilityHint = nil }
        if accessibilityIdentifier != nil { accessibilityIdentifier = nil }
        if accessibilityElementsHidden { accessibilityElementsHidden = false }
        props = [:]
    }
    /// After its create ops: a node that shows no symbol keeps no glyph
    /// view; one that shows the same symbol as before reports its size for
    /// its new id (a changed symbol already did).
    func finishReuse() {
        guard kind == "image" else { return }
        if imageSource?.hasPrefix("symbol:") != true { clearSymbol(); image = nil; return }
        if symbolView != nil { presenter?.queueSymbolSize(self, generation: loadGeneration, image?.size) }
    }
}
#endif
