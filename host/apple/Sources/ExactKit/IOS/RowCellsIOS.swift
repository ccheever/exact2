// @ref LLP 1115.000 D16 — rows of a vertical `list` are cells, on iOS: a
// list Contract marked (`listRows`) shows what a hand-built plain list
// shows. The reference is `UICollectionView` with a plain list
// configuration, measured on an iOS 27.0 simulator at 3x, light and dark
// (`~/audits/exact2-no-tells-2026-10-11/d16/ios-reference/RESULTS.md`):
//
// - a separator under every row, the last included, and one above the
//   first: one point thick (three pixels, not a hairline) in
//   `UIColor.separator`, from where the row's text starts (16 points in a
//   row of text; past a leading image, not under it) to 16 points short of
//   the trailing edge;
// - a pressed row's ground is `systemGray4`, edge to edge with square
//   corners, the same when it is selected, shown and taken away with no
//   animation of its own; text keeps its colours; the separators above and
//   below a highlighted or selected row are hidden;
// - a tapped row stays selected while the screen it pushed is up, and
//   fades as the reader comes back, clocked by the pop's progress; with no
//   transition to ride, it fades as a spring of 0.3 s (no bounce), and the
//   separators are back at once.
//
// These are Exact's own rows, drawn as cells: the runner's views stay where
// the presenter put them, and this draws the separators over them and the
// ground under them, in the list's own content. They are not UIKit's cells.
// What that leaves to the rows themselves is what they already do: a row's
// swipe actions are UIKit's own (`SwipeActionsIOS.swift`), and a reordered
// row is lifted by the presenter. What real cells would add is in the LLP's
// as-built row.
//
// What the author writes wins: a row whose author wrote its bottom edge, or
// the next row's top edge (`none` is written too), has no separator there;
// a row with a ground of its own is its author's card, whose press is a
// custom pressable's (the dim); a row's own press (`-exact-press-scale`,
// `pointerdown`, `pointerup`) replaces the highlight.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// A list whose rows are cells: Contract's mark.
    var hasRowCells: Bool { kind == "list" && props["listRows"] == "true" }
    /// The nearest node view above this one.
    fileprivate var nodeAbove: NodeView? {
        var above = superview
        while let view = above, !(view is NodeView) { above = view.superview }
        return above as? NodeView
    }
    /// The row this node is in, itself included, and its list: the nearest
    /// node from it up whose parent is a list whose rows are cells. A
    /// windowed list's row is the runner's wrapper.
    var rowCell: (row: NodeView, list: NodeView)? {
        var at = self
        while let above = at.nodeAbove {
            if above.hasRowCells { return (at, above) }
            at = above
        }
        return nil
    }
    /// What the author wrote of a row: the row, or the row inside a
    /// windowed list's wrapper.
    fileprivate var authoredRow: NodeView? {
        guard props["listItemKey"] != nil else { return self }
        return container.subviews.lazy.compactMap { $0 as? NodeView }.first
    }
    fileprivate var presses: Bool { handlers.contains("press") || defaultLink != nil }
    /// The node a tap on this row presses, as the Mac's table has it
    /// (`TableFeed.pressing`): the row itself when it presses; else, where
    /// the row is a swipe container, the content it names. Nil for a row of
    /// text, whose own pressables are controls in a cell.
    fileprivate var rowPress: NodeView? {
        guard let root = authoredRow else { return nil }
        if root.presses { return root }
        guard let name = root.props["swipeContent"], !name.isEmpty else { return nil }
        func find(_ view: UIView) -> NodeView? {
            for case let node as NodeView in (view as? NodeView)?.container.subviews ?? view.subviews {
                if node.props["id"] == name { return node.presses ? node : nil }
                if let found = find(node) { return found }
            }
            return nil
        }
        return find(root)
    }
    /// Whether the row's author gave it a ground of its own, on the row or
    /// on anything from what it presses up to it: the cell's ground would
    /// lie under it, unseen, so such a row is its author's card, and its
    /// press is a custom pressable's (the dim).
    fileprivate func hasOwnGround(pressing node: NodeView) -> Bool {
        var at: NodeView? = node
        while let current = at {
            if current.wrote("background_color") || current.wrote("background_image") { return true }
            if current === self { return false }
            at = current.nodeAbove
        }
        return false
    }
    /// The first paragraph in the row, in the order it is written: where a
    /// cell's separator begins.
    fileprivate var firstText: NodeView? {
        if kind == "text" { return self }
        for case let child as NodeView in container.subviews where !child.isHidden {
            if let found = child.firstText { return found }
        }
        return nil
    }
}

/// One list's separators: a shape over its rows, in the list's content, in
/// UIKit's separator colour for the appearance it is shown in.
final class RowLinesView: UIView {
    override class var layerClass: AnyClass { CAShapeLayer.self }
    var shape: CAShapeLayer { layer as! CAShapeLayer } // swiftlint:disable:this force_cast
    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        isAccessibilityElement = false
        layer.zPosition = 1
        tint()
        registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitAccessibilityContrast.self]) { (view: RowLinesView, _: UITraitCollection) in view.tint() }
    }
    required init?(coder: NSCoder) { nil }
    private func tint() { shape.fillColor = RowCells.separator.resolvedColor(with: traitCollection).cgColor }
    /// Never a hit: a touch on a separator is the row's.
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }
}

/// The ground of the row that is pressed or selected, under the list's rows.
final class RowGroundView: UIView {
    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        isAccessibilityElement = false
        layer.zPosition = -1
        backgroundColor = RowCells.highlight
    }
    required init?(coder: NSCoder) { nil }
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }
}

final class RowCells {
    /// UIKit's separator, as a plain list's cell draws it.
    static let separator = UIColor.separator
    /// A separator's thickness: a point, as measured on iOS 27 (earlier
    /// systems drew a hairline).
    static let thickness: CGFloat = 1
    /// How far short of the trailing edge a separator ends, and where one
    /// starts in a row with no text: a cell's layout margin at an iPhone's
    /// width (an iPad's wider margin was not measured).
    static let margin: CGFloat = 16
    /// The ground of a highlighted or selected cell of a plain list. (tvOS
    /// has no such grey and draws no row of this: its focus is the feedback.)
    #if os(iOS)
    static let highlight = UIColor.systemGray4
    #else
    static let highlight = UIColor.systemGray
    #endif
    /// How long a cell's ground fades when it is let go with no transition
    /// to ride: the spring `deselectItem(at:animated:)` adds (perceptual
    /// duration 0.3, no bounce).
    static let fade: TimeInterval = 0.3
    /// How long a highlight shows at least, so a quick tap still flashes.
    static let hold: TimeInterval = 0.1

    private weak var presenter: Presenter?
    private var lines: [UInt32: RowLinesView] = [:]
    /// The row whose ground shows, its list, and whether it is selected (a
    /// tap landed on it) or only held down.
    private var shown: (row: NodeView, list: UInt32, ground: RowGroundView, selected: Bool, since: CFTimeInterval)?
    /// The selected row's ground is fading with a pop: its separators are back.
    private var fading = false
    init(_ presenter: Presenter) { self.presenter = presenter }

    func reset() {
        for view in lines.values { view.removeFromSuperview() }
        lines.removeAll()
        shown?.ground.removeFromSuperview()
        shown = nil
    }

    // MARK: Separators

    /// After a batch: the separators of each list it touched (a list is in
    /// `changed` when anything in it was), and of lists that came or went.
    func sync(changed: Set<UInt32>?) {
        #if os(iOS)
        guard let presenter else { return }
        for (id, view) in lines where presenter.listViews[id]?.hasRowCells != true {
            view.removeFromSuperview()
            lines[id] = nil
        }
        if let held = shown, presenter.views[held.row.id] !== held.row || held.row.window == nil { drop() } else if let held = shown { held.ground.frame = held.row.frame }
        for (id, list) in presenter.listViews where list.hasRowCells && (changed?.contains(id) != false || lines[id] == nil) { draw(list) }
        #endif
    }

    private func isSpacer(_ row: NodeView) -> Bool { row.props["accessibilityElementsHidden"] == "true" }
    private func wroteBottom(_ row: NodeView) -> Bool { row.authoredRow.map { $0.wrote("border_width_bottom") || $0.wrote("border_style_bottom") } ?? false }
    private func wroteTop(_ row: NodeView) -> Bool { row.authoredRow.map { $0.wrote("border_width_top") || $0.wrote("border_style_top") } ?? false }
    /// The list's rows as they stand, in order: its children that show. A
    /// windowed list's spacers are among them, and are no rows.
    private func rows(of list: NodeView) -> [NodeView] { list.container.subviews.compactMap { $0 as? NodeView }.filter { !$0.isHidden } }

    /// Whether UIKit's separator belongs under `row`, with `next` after it
    /// (nil for the last, which has one too): not under a spacer, and not
    /// where the row's author wrote that edge or the next row's top one.
    private func separated(_ row: NodeView, next: NodeView?) -> Bool {
        guard !isSpacer(row), !wroteBottom(row) else { return false }
        if let next, !isSpacer(next), wroteTop(next) { return false }
        return true
    }

    private func draw(_ list: NodeView) {
        let content = list.container
        let view = lines[list.id] ?? RowLinesView(frame: .zero)
        if view.superview !== content { content.addSubview(view) }
        lines[list.id] = view
        let rows = rows(of: list)
        let lit = shown.flatMap { $0.list == list.id && !fading ? $0.row : nil }
        let path = CGMutablePath()
        /// A line at a height of `row`'s box, from where its text starts.
        func line(_ row: NodeView, at y: CGFloat) {
            let frame = row.frame
            // From the row's text, as a cell's separator is from its label's
            // leading edge and not under a leading image; a row with no
            // text, from the cell's margin.
            let lead = row.firstText.map { row.convert(CGPoint(x: $0.bounds.minX, y: 0), from: $0).x } ?? Self.margin
            let from = frame.minX + max(0, min(lead, frame.width)), to = frame.maxX - Self.margin
            if to > from { path.addRect(CGRect(x: from, y: y, width: to - from, height: Self.thickness)) }
        }
        for (index, row) in rows.enumerated() {
            let next = index + 1 < rows.count ? rows[index + 1] : nil
            // The first row has one above it too, in its own top point: the
            // first of the list, so not after a windowed list's spacer.
            if index == 0, !isSpacer(row), !wroteTop(row), row !== lit { line(row, at: row.frame.minY) }
            // A highlighted or selected row hides the one above it and its own.
            if separated(row, next: next), row !== lit, next == nil || next !== lit { line(row, at: row.frame.maxY - Self.thickness) }
        }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        view.shape.path = path
        CATransaction.commit()
    }
    private func redraw(_ list: UInt32) { if let view = presenter?.listViews[list], view.hasRowCells { draw(view) } }

    // MARK: The press

    /// `node`'s `pressed` changed. True when it is a row's press, and its
    /// cell's ground stands in for the dim.
    func press(_ node: NodeView, _ down: Bool) -> Bool {
        #if os(iOS)
        guard let (row, list) = node.rowCell.map({ ($0.row, $0.list) }), row.rowPress === node, !row.hasOwnGround(pressing: node) else { return false }
        if down {
            show(row, in: list, selected: false)
        } else if let held = shown, held.row === row, !held.selected {
            letGo(animated: true)
        }
        return true
        #else
        return false
        #endif
    }

    /// A tap landed on `node`, before its press runs: where its press is its
    /// row's cell's (and not the author's own), the row is selected, so the
    /// ground stays while the press runs. Returns the row, for `ran`.
    func select(_ node: NodeView) -> NodeView? {
        #if os(iOS)
        guard node.platformPressDims, let (row, list) = node.rowCell.map({ ($0.row, $0.list) }), row.rowPress === node, !row.hasOwnGround(pressing: node) else { return nil }
        show(row, in: list, selected: true)
        return row
        #else
        return nil
        #endif
    }

    /// The press of a selected row has run. A row that pushed a screen over
    /// its own stays selected under it, as a table view's does
    /// (`clearsSelectionOnViewWillAppear`); any other lets go.
    func ran(_ row: NodeView, on screen: UIViewController?) {
        guard let held = shown, held.row === row, held.selected else { return }
        if let screen, let stack = screen.navigationController, stack.viewControllers.contains(screen), stack.topViewController !== screen { return }
        letGo(animated: true)
    }

    /// The screen a view is on, read before its press can change it.
    func screen(of view: UIView) -> UIViewController? {
        sequence(first: view as UIResponder, next: { $0.next }).lazy.compactMap { $0 as? UIViewController }.first
    }

    /// A route's screen is coming back into view: the row its list kept
    /// selected under the screen it pushed fades with the transition, so
    /// the fade follows a back swipe's finger, and a swipe let go of early
    /// leaves the row selected. With no transition, at once.
    func appearing(_ route: NodeView, alongside coordinator: UIViewControllerTransitionCoordinator?) {
        guard let held = shown, held.selected, held.row.isDescendant(of: route) else { return }
        guard let coordinator, !ExactEnv.agentFreezes else { return drop() }
        let ground = held.ground, list = held.list
        // The separators beside it are back as the fade begins, as UIKit's are.
        fading = true
        redraw(list)
        coordinator.animate(alongsideTransition: { _ in ground.alpha = 0 }, completion: { [weak self] context in
            guard let self else { return ground.removeFromSuperview() }
            self.fading = false
            if context.isCancelled { ground.alpha = 1; self.redraw(list) } else if self.shown?.ground === ground { self.drop() } else { ground.removeFromSuperview() }
        })
    }

    private func show(_ row: NodeView, in list: NodeView, selected: Bool) {
        if let held = shown, held.row === row {
            held.ground.layer.removeAllAnimations()
            held.ground.alpha = 1
            shown = (row, list.id, held.ground, selected || held.selected, held.since)
            return
        }
        drop()
        let ground = RowGroundView(frame: row.frame)
        list.container.insertSubview(ground, at: 0)
        shown = (row, list.id, ground, selected, CACurrentMediaTime())
        redraw(list.id)
    }

    /// The ground goes: faded as a deselected cell's, after it has shown
    /// long enough to be seen, or at once under the agent's clock.
    private func letGo(animated: Bool) {
        guard let held = shown else { return }
        shown = nil
        fading = false
        // The separators beside it are back at once, as UIKit's are.
        redraw(held.list)
        let ground = held.ground
        guard animated, !ExactEnv.agentFreezes else { ground.removeFromSuperview(); return }
        let wait = max(0, Self.hold - (CACurrentMediaTime() - held.since))
        UIView.animate(springDuration: Self.fade, bounce: 0, initialSpringVelocity: 0, delay: wait, options: [.beginFromCurrentState, .allowUserInteraction], animations: { ground.alpha = 0 }, completion: { _ in ground.removeFromSuperview() })
    }
    private func drop() { letGo(animated: false) }

    // MARK: The agent

    /// What a list or one of its rows shows as a cell: `layout <node>`.
    func observation(_ node: NodeView) -> [String: Any]? {
        if node.hasRowCells { return ["rows": "cells", "separators": lines[node.id].map { $0.shape.path?.boundingBoxOfPath.isNull == false } ?? false] }
        guard let (row, list) = node.rowCell.map({ ($0.row, $0.list) }), row === node else { return nil }
        let rows = rows(of: list)
        let at = rows.firstIndex { $0 === row }
        let next = at.flatMap { $0 + 1 < rows.count ? rows[$0 + 1] : nil }
        var seen: [String: Any] = ["row": "cell", "separator": separated(row, next: next)]
        if let held = shown, held.row === row { seen["ground"] = held.selected ? "selected" : "highlighted" }
        return seen
    }
}
#endif
