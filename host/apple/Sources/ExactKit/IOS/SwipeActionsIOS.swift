// @ref LLP 1008 §9 — authored row content and action controls projected into
// UIKit swipe cells. The kernel owns dimensions; UIKit owns the gesture.
#if os(iOS) || os(tvOS)
import UIKit
import UIKit.UIGestureRecognizerSubclass

/// A row pays for its UIKit cell only while a swipe can start. At rest the
/// authored scroll holds the content, as on the web, and a batch walks no
/// hierarchy. A touch landing on the row projects it into a one-row table
/// (`touch`, from the row's hit test: before UIKit gathers the touch's
/// recognizers, so the table's own swipe sees the first pan). Once the touch
/// has ended, the row is closed and UIKit's animation has settled, the
/// content goes back and the table goes. VoiceOver and Switch Control read a
/// row's actions from its cell, so while either runs every row stays projected.
///
/// A grouped list's row (LLP 1116.001 D1) is a list cell already: UIKit's
/// own, which the list carries the row's views into. Its swipe is that
/// cell's, as a hand-built list's is: the list asks `configuration` for its
/// actions, and the row's controls stay hidden while the row is one.
package final class SwipeActionsHost {
    unowned let presenter: Presenter
    private var rows: [UInt32: Row] = [:]
    private var refusals: [UInt32: String] = [:]
    #if os(iOS)
    /// Rows whose actions keep their paint (PaintedSwipesIOS.swift).
    private(set) lazy var painted = PaintedSwipes(presenter)
    #endif
    private var observers: [NSObjectProtocol] = []
    init(_ presenter: Presenter) {
        self.presenter = presenter
        for name in [UIAccessibility.voiceOverStatusDidChangeNotification, UIAccessibility.switchControlStatusDidChangeNotification] {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                guard let self else { return }
                for row in rows.values { if assistive { row.mount() } else { row.settle() } }
            })
        }
    }
    deinit { observers.forEach { NotificationCenter.default.removeObserver($0) } }
    var assistive: Bool { UIAccessibility.isVoiceOverRunning || UIAccessibility.isSwitchControlRunning }

    /// Grouped rows that `prepare` left in their cells' state, for `sync` to keep.
    private var kept = Set<UInt32>()
    /// Before a batch, rows go back to the authored hierarchy (`batch` nil: every one). A grouped row stays as its cell
    /// has it when the batch changes nothing in it but boxes and sets no `inert` (which its actions read from above
    /// it): a row inserted at the top of a long list moves every row below, and each row's restore and mount set
    /// `isHidden` on its actions twice.
    func prepare(_ batch: Batch? = nil) {
        kept = []
        var touched: Set<UInt32>?
        if let batch, !batch.ops.contains(where: { [.roots, .flight, .land, .exit].contains($0.op) || ($0.op == .props && ($0.props["inert"] != nil || $0.clear.contains("inert"))) }) {
            var owners = Set<UInt32>()
            // A node created inside a row is under a `children` op on its parent there.
            for op in batch.ops where ![.frame, .content, .fieldContent].contains(op.op) {
                var at: UIView? = presenter.views[op.id]
                while let view = at {
                    if let node = view as? NodeView, rows[node.id]?.owner === node { owners.insert(node.id); break }
                    at = view.superview
                }
            }
            touched = owners
        }
        for row in rows.values where row.projected || row.grouped {
            if row.grouped, row.mounted, let touched, !touched.contains(row.owner.id) { kept.insert(row.owner.id); continue }
            row.restore()
        }
    }
    /// A touch is landing on `owner`: project its row now, and release it
    /// again if the touch never reaches the table.
    func touch(_ owner: NodeView) {
        #if os(iOS)
        if painted.touch(owner) { return }
        #endif
        guard let row = rows[owner.id], row.owner === owner, !row.projected, !row.grouped else { return }
        row.mount()
        DispatchQueue.main.async { [weak row] in row?.settle() }
    }
    /// The row projected for `owner`, if any (tests and diagnostics).
    func cell(of owner: NodeView) -> UITableViewCell? { rows[owner.id]?.projection }
    /// Why `owner`'s native swipe is refused, as the log says it, if it is.
    func refusal(of owner: NodeView) -> String? { refusals[owner.id] }
    func reset() {
        for row in rows.values { row.discard() }
        rows.removeAll(); refusals.removeAll()
        #if os(iOS)
        painted.reset()
        #endif
    }
    #if os(iOS)
    /// A painted row began a drag (PaintedSwipesIOS.swift): every native
    /// row closes, one row of either kind open at a time.
    func closeNative() {
        for row in rows.values { row.table?.setEditing(false, animated: true) }
    }
    #endif
    #if os(iOS)
    /// A grouped list row's actions on one side, as its list cell offers
    /// them (LLP 1116.001 D1); nil for a row that names none there, or that
    /// is no grouped list's swipe row.
    package func configuration(row id: UInt32, leading: Bool) -> UISwipeActionsConfiguration? {
        guard let row = rows[id], row.grouped else { return nil }
        let offered = row.configuration(leading ? row.leading : row.trailing)
        // The list asks as its row's swipe begins: a painted row that is
        // open closes, one row of either kind open at a time.
        if offered != nil { painted.closeAll() }
        return offered
    }
    #endif

    /// Whether `owner` is a grouped list's row: its section's group's child
    /// (LLP 1084 D3), drawn in UIKit's list cell. Read from the authored
    /// hierarchy: a row the list carries is its parent's child from its cell.
    private func groupedRow(_ owner: NodeView) -> Bool {
        #if os(iOS)
        guard let lists = presenter.groupedLists else { return false }
        func parent(_ view: UIView?) -> NodeView? {
            var at = view.flatMap(lists.logicalParent) ?? view?.superview
            while let v = at { if let node = v as? NodeView { return node }; at = v.superview }
            return nil
        }
        guard let list = parent(parent(parent(owner))) else { return false }
        return list.kind == "list" && list.props["listStyle"] != nil
        #else
        return false
        #endif
    }

    /// `changed`: the views the batch touched and their ancestors. An owner
    /// with a row, outside it and none of whose names changed carriers,
    /// keeps its row as it was; nil revisits every owner. `reshaped`: those
    /// of them an op other than a frame touched. An owner the batch only
    /// moved (two rows inserted above it) keeps its row while its content
    /// still fills it: its names resolve to the same nodes.
    func sync(changed: Set<UInt32>? = nil, reshaped: Set<UInt32>? = nil) {
        let renamed = presenter.takeChangedNames()
        // The views whose name is read from an element renamed now, found once, not once a row.
        let relabelled = renamed.isEmpty ? [] : Set(presenter.carrying("accessibilityLabelledBy").filter {
            $0.props["accessibilityLabelledBy"]?.split(separator: " ").contains { renamed.contains(String($0)) } == true
        }.map(\.id))
        var wanted = Set<UInt32>(), paintedOwners = Set<UInt32>()
        var claimed = Set<UInt32>()
        #if os(iOS)
        painted.dropStale()
        #endif
        for owner in presenter.carrying("swipeContent") {
            guard let content = owner.props["swipeContent"] else { continue }
            #if os(iOS)
            // A painted row the batch did not reach keeps what it was adopted
            // with: a list scrolling builds a few rows a report, not every
            // row again. Its names resolve inside it, so a name the batch
            // gave a row it built (every row's `id`s are the same) changes
            // nothing here; one given inside it put it in `reshaped`.
            if let changed, let row = painted.held(owner),
               relabelled.isEmpty || !row.controls.contains(where: { relabelled.contains($0.id) }),
               !changed.contains(owner.id) || (reshaped?.contains(owner.id) == false && fills(row.body, owner)),
               claimed.insert(row.body.id).inserted {
                paintedOwners.insert(owner.id)
                continue
            }
            #endif
            // A refused owner is retried every batch, as the names its
            // controls read from their subtrees can arrive a batch later.
            // An owner outside `changed` has the props its row was built
            // from, so the row's names are still the owner's.
            // A kept row's subtree is as it was, so its names resolve to the same nodes whatever was renamed
            // elsewhere (a new row's `id`s are every row's); only an action labelled by an id can read another name.
            if let row = rows[owner.id], row.owner === owner, kept.contains(owner.id),
               relabelled.isEmpty || !(row.leading + row.trailing).contains(where: { relabelled.contains($0.id) }) {
                claimed.insert(row.body.id)
                wanted.insert(owner.id)
                continue
            }
            if let changed, !renamed.contains(content), let row = rows[owner.id], row.owner === owner, !row.names.contains(where: renamed.contains),
               !changed.contains(owner.id) || (reshaped?.contains(owner.id) == false && (row.grouped || fills(row.body, owner))) {
                claimed.insert(row.body.id)
                if row.projected || row.grouped || assistive { row.mount() }
                wanted.insert(owner.id)
                continue
            }
            let leadingNames = (owner.props["swipeLeading"] ?? "").split(whereSeparator: \.isWhitespace).map(String.init)
            let trailingNames = (owner.props["swipeTrailing"] ?? "").split(whereSeparator: \.isWhitespace).map(String.init)
            let names = leadingNames + trailingNames
            // A name resolves within its row, as an `id` repeated by a `for`
            // (and `delete=`'s expansion) names one element in each row.
            func resolve(_ name: String) -> NodeView? {
                var found: [NodeView] = []
                func visit(_ view: UIView) {
                    for child in view.subviews {
                        if let node = child as? NodeView, node.props["id"] == name { found.append(node) }
                        visit(child)
                    }
                }
                visit(owner)
                return found.count == 1 ? found[0] : nil
            }
            let controls = names.compactMap(resolve)
            let body = resolve(content)
            // A grouped row's cell is UIKit's: its content need not fill the row.
            let grouped = groupedRow(owner)
            // The first rule the row breaks, by name and with the sizes it
            // compared (splitter rough 10: a hairline border on the scroll
            // left its content half a point short, and the refusal said only
            // what a row needs).
            let broken: String? = {
                let size = { (v: NodeView) in String(format: "%gx%g", v.layoutBounds.width, v.layoutBounds.height) }
                if owner.scroll == nil && !owner.scrollDormant { return "it is not a scroll container (overflow-x)" }
                guard let body else { return "swipeContent \"\(content)\" names no one descendant of it" }
                if names.isEmpty { return "it names no swipeLeading or swipeTrailing control" }
                if let twice = names.first(where: { n in names.filter { $0 == n }.count > 1 }) { return "\"\(twice)\" is named twice" }
                if let missing = names.first(where: { resolve($0) == nil }) { return "\"\(missing)\" names no one descendant of it" }
                for (name, control) in zip(names, controls) {
                    if !control.handlers.contains("press") { return "\"\(name)\" has no press handler" }
                    if control === body || control.isDescendant(of: body) { return "\"\(name)\" is inside swipeContent \"\(content)\"" }
                    if label(control).isEmpty { return "\"\(name)\" has no accessible name (aria-label or text)" }
                }
                if !grouped, !fills(body, owner) {
                    return "swipeContent \"\(content)\" is \(size(body)), not the row's \(size(owner)) border box (a border or padding on the row shrinks it)"
                }
                if claimed.contains(body.id) { return "swipeContent \"\(content)\" is another row's too" }
                return nil
            }()
            guard broken == nil, let body, claimed.insert(body.id).inserted else {
                let message = "swipeContent on #\(owner.id) is refused: \(broken ?? "its content is another row's"); a row needs one full-size descendant and uniquely named descendant press controls with accessible names"
                if refusals[owner.id] != message { fputs("exact: \(message)\n", stderr); refusals[owner.id] = message }
                // A refused row swipes as the web does: by its scroll.
                owner.needScroll()
                continue
            }
            refusals.removeValue(forKey: owner.id)
            #if os(iOS)
            // Actions that say `appearance="none"` keep their paint: the row
            // swipes by its scroll, with UIKit's swipe's behaviours. In a
            // grouped list too: its cell then offers no swipe actions
            // (`configuration` finds no row) and carries the scroll.
            if PaintedSwipes.keepsPaint(controls) {
                rows.removeValue(forKey: owner.id)?.discard()
                painted.adopt(owner, body: body, leading: Array(controls.prefix(leadingNames.count)),
                              trailing: Array(controls.dropFirst(leadingNames.count)), label: label)
                paintedOwners.insert(owner.id)
                continue
            }
            #endif
            if let old = rows[owner.id], old.body !== body || old.grouped != grouped { old.discard(); rows.removeValue(forKey: owner.id) }
            let row = rows[owner.id] ?? Row(owner: owner, body: body, host: self, grouped: grouped)
            rows[owner.id] = row
            row.names = names
            row.leading = Array(controls.prefix(leadingNames.count))
            row.trailing = Array(controls.dropFirst(leadingNames.count))
            if row.projected || grouped || assistive { row.mount() }
            wanted.insert(owner.id)
        }
        for id in Array(rows.keys) where !wanted.contains(id) { rows.removeValue(forKey: id)?.discard() }
        #if os(iOS)
        painted.retain(paintedOwners)
        #endif
        refusals = refusals.filter { presenter.views[$0.key] != nil }
        kept = []
    }

    /// The row's content is its owner's border box, as a row's must be.
    private func fills(_ body: NodeView, _ owner: NodeView) -> Bool {
        abs(body.layoutBounds.width - owner.layoutBounds.width) < 0.5 && abs(body.layoutBounds.height - owner.layoutBounds.height) < 0.5
    }

    /// An action's name; a native button's is its label, else its title
    /// (LLP 1069.011.000 D6). Read from the node, not its view's
    /// `accessibilityLabel`: with assistive technology's runtime loaded,
    /// UIKit answers that getter, and each row a scroll builds read it.
    private func label(_ node: NodeView) -> String {
        node.props["accessibilityLabel"] ?? (node.isNativeButton ? node.face?.title : nil) ?? node.accessibleName
    }
    /// LLP 1080.001 D3: a projected row's table, the views it hides, and
    /// the content it carries into its cell.
    func inspectionOwns(_ view: UIView) -> Bool { rows.values.contains { $0.inspectionOwns(view) } }
    func hides(_ node: NodeView) -> Bool { rows.values.contains { $0.hides(node) } }
    func projects(_ view: UIView) -> Bool { rows.values.contains { $0.projects(view) } }
    func ownsAction(_ id: UInt32) -> Bool { rows.values.contains { ($0.leading + $0.trailing).contains { $0.id == id } } }
    func actionView(_ id: UInt32) -> UIButton? {
        for row in rows.values {
            if let button = row.actionView(id) { return button }
        }
        return nil
    }
    #if EXACT_INSPECT
    /// The action a button UIKit draws for a row stands for: inspect mode
    /// selects it (InspectIOS.swift).
    func action(for button: UIButton) -> UInt32? {
        for row in rows.values {
            for action in row.leading + row.trailing where row.actionView(action.id) === button { return action.id }
        }
        return nil
    }
    #endif
    /// The agent's tap on an action no swipe has revealed (an authored
    /// test's `tap delete-…`, splitter rough 12): the action UIKit would
    /// offer, performed as VoiceOver performs a cell's swipe action, with
    /// no swipe; the web's tap scrolls the row to the same control. False
    /// when UIKit would offer no such action (disabled, hidden, inert); nil
    /// when no row owns `id`.
    func perform(_ id: UInt32, before: () -> Bool = { true }) -> Bool? {
        // A loop, not `lazy.compactMap(…).first`, which asks its element twice: two presses.
        for row in rows.values { if let offered = row.perform(id, before: before) { return offered } }
        return nil
    }

    private final class Ancestor {
        weak var view: UIView?
        init(_ view: UIView) { self.view = view }
    }

    private final class Cell: UITableViewCell, AgentOwned {
        weak var control: NodeView?
        var agentViewId: UInt32? { isAccessibilityElement ? control?.id : nil } // LLP 1080.002 D5
        override func accessibilityActivate() -> Bool {
            control?.accessibilityActivate() ?? super.accessibilityActivate()
        }
    }

    /// Recognizes nothing and prevents nothing: sees a projected row's
    /// touches begin, and the sequence end (UIKit resets it then).
    private final class TouchWatch: UIGestureRecognizer, UIGestureRecognizerDelegate {
        var began: () -> Void = {}
        var ended: () -> Void = {}
        init() {
            super.init(target: nil, action: nil)
            cancelsTouchesInView = false; delaysTouchesEnded = false; delegate = self
        }
        override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) { began() }
        override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) { finish(event) }
        override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) { finish(event) }
        private func finish(_ event: UIEvent) {
            if event.allTouches?.allSatisfy({ $0.phase == .ended || $0.phase == .cancelled }) ?? true { state = .failed }
        }
        override func reset() { super.reset(); ended() }
        override func canPrevent(_ other: UIGestureRecognizer) -> Bool { false }
        override func canBePrevented(by other: UIGestureRecognizer) -> Bool { false }
        func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }
    }

    /// A contact in a swipe row that leaves the slop toward a side with
    /// actions is the swipe's, never a press (LLP 1057.001 rule 3: the drag
    /// that begins takes the press; LLP 1116.004 D2). Recognizing cancels
    /// the touch in the row's views — its press, a checkbox's or a button's
    /// activation — whether or not UIKit's swipe, which keeps its own
    /// threshold, goes on to reveal the actions: a sideways drag too short
    /// for UIKit to begin (Maestro's swipe from a row's leading checkbox,
    /// nine points to the screen's tenth) or a flick that lifts before it
    /// does presses nothing. It prevents nothing and is prevented by
    /// nothing, so UIKit's swipe and the list's scroll see every touch. A
    /// drag toward a side with no actions, or up and down, stays the press's.
    final class SwipeClaim: UIGestureRecognizer, UIGestureRecognizerDelegate {
        enum Decision: Equatable { case wait, claim, fail }
        /// What a touch's travel (dx, dy) from where it began decides, with
        /// actions to reveal by dragging `left` or `right`: up or down past
        /// the slop is the list's scroll; sideways past it toward actions, the swipe's.
        static func decide(dx: CGFloat, dy: CGFloat, slop: CGFloat, left: Bool, right: Bool) -> Decision {
            if abs(dy) > slop, abs(dy) >= abs(dx) { return .fail }
            guard abs(dx) > slop, abs(dx) > abs(dy) else { return .wait }
            return (dx < 0 ? left : right) ? .claim : .fail
        }
        /// Whether dragging left and right reveals actions, read as the touch moves.
        var sides: () -> (left: Bool, right: Bool) = { (false, false) }
        private var start: CGPoint?
        init() {
            super.init(target: nil, action: nil)
            cancelsTouchesInView = true; delaysTouchesBegan = false; delaysTouchesEnded = false; delegate = self
        }
        override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
            guard start == nil, touches.count == 1, let touch = touches.first, !Self.drags(touch.view) else { state = .failed; return }
            start = touch.location(in: nil)
        }
        override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
            guard let start, let touch = touches.first else { return }
            let p = touch.location(in: nil), sides = sides()
            switch Self.decide(dx: p.x - start.x, dy: p.y - start.y, slop: CGFloat(Gesture.slop), left: sides.left, right: sides.right) {
            case .claim: state = .recognized
            case .fail: state = .failed
            case .wait: break
            }
        }
        override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) { state = .failed }
        override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) { state = .failed }
        override func reset() { super.reset(); start = nil }
        override func canPrevent(_ other: UIGestureRecognizer) -> Bool { false }
        override func canBePrevented(by other: UIGestureRecognizer) -> Bool { false }
        func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }
        /// A control whose own gesture is a sideways drag keeps its touch:
        /// a switch, a slider, an editor.
        static func drags(_ view: UIView?) -> Bool {
            var at = view
            while let v = at, !(v is UICollectionViewCell || v is UITableViewCell) {
                #if os(iOS)
                if v is UISwitch || v is UISlider { return true }
                #endif
                if v is UITextField || v is UITextView { return true }
                if let node = v as? NodeView, node.field != nil || node.textArea != nil { return true }
                at = v.superview
            }
            return false
        }
    }

    private final class Row: NSObject, UITableViewDataSource, UITableViewDelegate {
        unowned let host: SwipeActionsHost
        let owner: NodeView
        let body: NodeView
        /// A grouped list's row: its list cell swipes, and no table is made.
        let grouped: Bool
        var leading: [NodeView] = []
        var trailing: [NodeView] = []
        /// The owner's leading and trailing names, as its props said when
        /// this row was last built.
        var names: [String] = []
        private weak var logicalParent: UIView?
        private var carrier: UIView?
        private var hiddenControls: [(NodeView, Bool)] = []
        func inspectionOwns(_ view: UIView) -> Bool { table === view }
        func hides(_ node: NodeView) -> Bool { hiddenControls.contains { $0.0 === node } }
        func projects(_ view: UIView) -> Bool { carrier === view }
        // Captured before projection: UIKit disables its cell while an action
        // completes. That temporary state is not an authored input restriction.
        private var actionAncestors: [UInt32: [Ancestor]] = [:]
        private var scrollWasHidden = false
        /// A grouped row's: its owner's authored inertness, read in the authored
        /// hierarchy, and whether its scroll was stilled for the cell's swipe.
        /// Not a presentation's: a row inserted under a sheet (a contact its
        /// form saves) is mounted while the sheet makes it inert and mounted
        /// again only by a batch that changes it, so its cell would offer no
        /// actions once the sheet closed.
        private var ownerInert = false
        private var stilled = false
        /// A grouped row's actions are hidden and its scroll stilled for its cell (`mountGrouped`), until `restore`.
        private(set) var mounted = false
        private var logicalFrame = CGRect.zero
        private var priorSize = CGSize.zero
        /// The projection, only while a swipe can start or is shown.
        private(set) var table: UITableView?
        private var cell: Cell?
        private var touching = false
        /// UIKit is presenting this row's actions (between its begin and end editing).
        private var open = false
        private var images: [UInt32: UIImage] = [:]
        /// A grouped row's claim on its sideways touches, on the owner the
        /// list carries into its cell; a projected row's is its table's.
        private var claim: SwipeClaim?
        var projected: Bool { table != nil }
        var projection: UITableViewCell? { cell }

        init(owner: NodeView, body: NodeView, host: SwipeActionsHost, grouped: Bool) {
            self.owner = owner; self.body = body; self.host = host; self.grouped = grouped
            super.init()
        }
        /// A claim whose sides are this row's actions, as they now stand.
        private func makeClaim() -> SwipeClaim {
            let claim = SwipeClaim()
            claim.sides = { [weak self] in
                guard let self else { return (false, false) }
                let leading = leading.contains(where: enabled), trailing = trailing.contains(where: enabled)
                // A drag toward the left reveals the actions on the right.
                return owner.effectiveUserInterfaceLayoutDirection == .rightToLeft ? (leading, trailing) : (trailing, leading)
            }
            return claim
        }
        private func project() -> (UITableView, Cell) {
            if let table, let cell { return (table, cell) }
            let table = SwipeTable(frame: owner.bounds, style: .plain)
            let cell = Cell(style: .default, reuseIdentifier: nil)
            table.dataSource = self; table.delegate = self
            // Only the outer authored scroll container scrolls vertically.
            table.isScrollEnabled = false
            table.contentInsetAdjustmentBehavior = .never
            #if !os(tvOS)
            table.separatorStyle = .none
            #endif
            table.backgroundColor = .clear
            table.estimatedRowHeight = 0; table.sectionHeaderTopPadding = 0
            table.allowsSelection = false
            // The row paints itself (its own background, its corners); the
            // cell is only where UIKit's swipe happens.
            cell.backgroundConfiguration = .clear()
            let watch = TouchWatch()
            watch.began = { [weak self] in self?.touching = true }
            watch.ended = { [weak self] in
                guard let self, touching else { return }
                touching = false
                DispatchQueue.main.async { [weak self] in self?.settle() }
            }
            table.addGestureRecognizer(watch)
            #if os(iOS)
            table.addGestureRecognizer(makeClaim())
            #endif
            self.table = table; self.cell = cell
            return (table, cell)
        }
        func restore() {
            // Home is where the owner keeps its content now: a scroll view
            // released while the row was projected left its children to the owner.
            if let carrier, let parent = logicalParent ?? (carrier.superview === cell?.contentView ? owner.scroll ?? owner : nil) {
                parent.addSubview(carrier); carrier.frame = logicalFrame
                if let node = carrier as? NodeView { owner.presenter?.regrid(node) } // placed from the cell while projected
            }
            for (control, hidden) in hiddenControls { control.isHidden = hidden }
            hiddenControls.removeAll()
            if stilled { owner.scroll?.isScrollEnabled = true; stilled = false }
            owner.scroll?.isHidden = scrollWasHidden
            mounted = false
        }
        /// The row is gone: its claim with it.
        func discard() {
            if let claim { claim.view?.removeGestureRecognizer(claim); self.claim = nil }
            remove()
        }
        func remove() {
            if grouped { restore(); return }
            guard let table else { return }
            restore(); carrier = nil; logicalParent = nil
            table.setEditing(false, animated: false); table.removeFromSuperview()
            self.table = nil; cell = nil; images.removeAll(); touching = false; open = false
        }
        /// Release the projection once nothing can be swiping: no touch, the
        /// row closed, and UIKit's close animation finished.
        func settle() {
            guard let table, !touching, !open, !host.assistive else { return }
            func moving(_ layer: CALayer) -> Bool {
                // Authored content may animate for ever; only UIKit's own layers count.
                if layer === carrier?.layer { return false }
                return !(layer.animationKeys() ?? []).isEmpty || (layer.sublayers ?? []).contains(where: moving)
            }
            if moving(table.layer) {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in self?.settle() }
                return
            }
            remove()
        }
        /// A grouped row in its list cell: its controls hidden (the cell's
        /// actions stand for them) and its content at the scroll's start,
        /// which takes no pan, so the cell's swipe sees the touch.
        private func mountGrouped() {
            actionAncestors.removeAll()
            for control in leading + trailing {
                // Up to the row: above it is the list's, which draws in its
                // hidden scroll's place, and the cell carries the row's inertness.
                var ancestors: [Ancestor] = [], current: UIView? = control
                while let view = current { ancestors.append(Ancestor(view)); if view === owner { break }; current = view.superview }
                actionAncestors[control.id] = ancestors
            }
            ownerInert = owner.authoredInert
            #if os(iOS)
            if claim == nil { let made = makeClaim(); owner.addGestureRecognizer(made); claim = made }
            #endif
            scrollWasHidden = owner.scroll?.isHidden ?? false
            for control in leading + trailing { hide(control) }
            if let scroll = owner.scroll {
                let start = -scroll.adjustedContentInset.left
                if scroll.contentOffset.x != start { scroll.contentOffset = PixelGrid.offset(CGPoint(x: start, y: scroll.contentOffset.y), in: scroll, clamped: false) }
                if scroll.isScrollEnabled { scroll.isScrollEnabled = false; stilled = true }
            }
            mounted = true
        }
        func mount() {
            if grouped { mountGrouped(); return }
            // A waiting scroll's children are the owner's own (`scrollDormant`).
            guard owner.scroll != nil || owner.scrollDormant else { return }
            // Mounted again while projected (VoiceOver starting, a batch with
            // no `prepare`): its content comes home first. Walking up from the
            // body in the cell would find the table, and the cell would take
            // its own table (CALayerInvalid, a layer tree cycle: r28's tasks).
            if carrier != nil, carrier?.superview === cell?.contentView { restore() }
            let scroll: UIView = owner.scroll ?? owner
            var content: UIView = body
            while let parent = content.superview, parent !== scroll { content = parent }
            guard content.superview === scroll, !(table?.isDescendant(of: content) ?? false) else { return }
            actionAncestors.removeAll()
            for control in leading + trailing {
                var ancestors: [Ancestor] = [], current: UIView? = control
                while let view = current { ancestors.append(Ancestor(view)); current = view.superview }
                actionAncestors[control.id] = ancestors
            }
            scrollWasHidden = scroll.isHidden
            carrier = content; logicalParent = scroll; logicalFrame = content.frame
            let origin = body.convert(CGPoint.zero, to: content)
            let (table, cell) = project()
            if table.superview !== owner { owner.addSubview(table) }
            owner.scroll?.isHidden = true
            // What the hidden scroll would hide: the content's siblings.
            if owner.scroll == nil {
                for case let sibling as NodeView in owner.subviews where sibling !== content { hide(sibling) }
            }
            if priorSize != owner.bounds.size || host.presenter.navigation.isInactiveRoute(containing: owner) {
                table.setEditing(false, animated: false)
            }
            priorSize = owner.bounds.size
            table.frame = owner.bounds
            // The cell follows the row: UIKit keeps a row's height until it reloads.
            if table.rowHeight != body.bounds.height { table.rowHeight = body.bounds.height; table.reloadData() }
            if content.superview !== cell.contentView { cell.contentView.addSubview(content) }
            // Keep the original ancestors between content and the row. Their
            // opacity, clips, inherited semantics and input restrictions apply.
            content.frame = CGRect(origin: CGPoint(x: -origin.x, y: -origin.y), size: logicalFrame.size)
            for control in leading + trailing { hide(control) }
            // UIKit derives its cell label from native text controls; an
            // authored button paints its own text. Preserve that button's
            // explicit name and activation at the native presentation boundary.
            if body.kind == "button", let label = body.accessibilityLabel, !label.isEmpty {
                cell.control = body
                cell.isAccessibilityElement = true
                cell.accessibilityLabel = label
                cell.accessibilityIdentifier = body.accessibilityIdentifier
                cell.accessibilityTraits = body.accessibilityTraits.union(.button)
            } else {
                cell.control = nil
                cell.isAccessibilityElement = false
                cell.accessibilityLabel = nil
                cell.accessibilityIdentifier = nil
                cell.accessibilityTraits = []
            }
            table.layoutIfNeeded()
        }
        /// See `SwipeActionsHost.perform`: projected for the check UIKit's
        /// own configuration makes, then released again as after a touch.
        func perform(_ id: UInt32, before: () -> Bool = { true }) -> Bool? {
            guard let target = (leading + trailing).first(where: { $0.id == id }) else { return nil }
            // A projected row's content is the cell's already: mounting it
            // again would carry the table into its own cell.
            // A grouped row is mounted by every batch; its cell is the list's.
            let was = projected || grouped
            if !was { mount() }
            let offered = (projected || grouped) && enabled(target)
            if !was { DispatchQueue.main.async { [weak self] in self?.settle() } }
            guard offered, before(), enabled(target) else { return false }
            host.presenter.press(target.id)
            return true
        }
        /// Hidden while projected; `restore` gives back what it was, once.
        private func hide(_ view: NodeView) {
            if !hiddenControls.contains(where: { $0.0 === view }) { hiddenControls.append((view, view.hiddenByHost)) }
            view.isHidden = true
        }
        func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { 1 }
        func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell { cell ?? UITableViewCell() }
        func tableView(_ tableView: UITableView, heightForRowAt indexPath: IndexPath) -> CGFloat { body.bounds.height }
        // tvOS has no row editing or swipe actions.
        #if !os(tvOS)
        func tableView(_ tableView: UITableView, willBeginEditingRowAt indexPath: IndexPath) {
            open = true
            for other in host.rows.values where other !== self { other.table?.setEditing(false, animated: true) }
            host.painted.closeAll()
            DispatchQueue.main.async { [weak self] in self?.nameActions() }
        }
        func tableView(_ tableView: UITableView, didEndEditingRowAt indexPath: IndexPath?) {
            open = false
            DispatchQueue.main.async { [weak self] in self?.settle() }
        }
        func tableView(_ tableView: UITableView, leadingSwipeActionsConfigurationForRowAt indexPath: IndexPath) -> UISwipeActionsConfiguration? { configuration(leading) }
        func tableView(_ tableView: UITableView, trailingSwipeActionsConfigurationForRowAt indexPath: IndexPath) -> UISwipeActionsConfiguration? { configuration(trailing) }
        #endif

        private func enabled(_ target: NodeView) -> Bool {
            guard let ancestors = actionAncestors[target.id], !ancestors.isEmpty else { return false }
            if grouped && ownerInert { return false }
            for ancestor in ancestors {
                guard let view = ancestor.view else { return false }
                // What the projection saved is the host's word; CSS's `display: none` hides besides.
                let hidden = view === owner.scroll ? scrollWasHidden :
                    hiddenControls.first(where: { $0.0 === view }).map { $0.1 || $0.0.style["display"]?.string == "none" } ?? view.isHidden
                if hidden || !view.isUserInteractionEnabled || (view as? NodeView)?.disabled == true ||
                    (view as? NodeView)?.props["inert"] == "true" { return false }
            }
            return true
        }
        /// A visible title beside an action's image: UIKit draws it under the
        /// glyph, as the authored button shows it. An image alone stays alone,
        /// and so does one whose text is authored hidden (the face reads text
        /// whatever its style; a native button draws its title itself).
        private func shownTitle(_ target: NodeView) -> String? {
            guard let title = target.face?.title, !title.isEmpty else { return nil }
            if target.isNativeButton { return title }
            guard let text = target.container.subviews.compactMap({ $0 as? NodeView }).first(where: \.isParagraph),
                  text.style["display"]?.string != "none", !text.isHidden, text.alpha > 0 else { return nil }
            return title
        }
        #if !os(tvOS)
        fileprivate func configuration(_ controls: [NodeView]) -> UISwipeActionsConfiguration? {
            let actions = controls.filter(enabled).map { target in
                let destructive = target.props["destructive"] == "true"
                let action = UIContextualAction(style: destructive ? .destructive : .normal, title: nil) { [weak self, weak target] _, _, complete in
                    guard let self, let target, self.host.presenter.views[target.id] === target, self.enabled(target) else { complete(false); return }
                    complete(true)
                    self.host.presenter.press(target.id)
                }
                action.accessibilityLabel = host.label(target)
                if target.isNativeButton {
                    // A native action (LLP 1069.011.000 D6): the accent its
                    // author wrote on it (LLP 1115.000 D2), or UIKit's colour
                    // for its style (grey; LLP 1115 D4), which an accent it
                    // only inherits leaves; a destructive one sets none,
                    // whatever its accent (UIKit's red); its symbol, recorded
                    // for discovery as a snapshot is.
                    if !destructive, target.wrote("accent_color"),
                       let accent = target.channels("accent_color").map({ TextEngine.color($0) }) {
                        action.backgroundColor = accent
                    }
                    if let symbol = target.face?.symbol, let image = FaceSymbol.image(symbol) {
                        image.accessibilityLabel = host.label(target)
                        images[target.id] = image; action.image = image
                        action.title = shownTitle(target)
                    } else { action.title = host.label(target) }
                    return action
                }
                // `delete=`'s Delete (LLP 1116.020 D1) is a hand-built list's: UIKit's
                // red, the trash symbol over its title (its web paint is the page's).
                if destructive, target.props["id"] == "exact-swipe-delete", let image = UIImage(systemName: "trash") {
                    image.accessibilityLabel = host.label(target)
                    images[target.id] = image; action.image = image
                    action.title = target.face?.title ?? host.label(target)
                    return action
                }
                // Its authored background, else the accent its author wrote
                // on it, as a native action's (Mail's Flag is orange), else
                // UIKit's for its style.
                if let fill = target.cgColor("background_color") {
                    action.backgroundColor = UIColor(cgColor: fill)
                } else if !destructive, target.wrote("accent_color"),
                          let accent = target.channels("accent_color").map({ TextEngine.color($0) }) {
                    action.backgroundColor = accent
                }
                // A custom action's symbol is a system image, as a native one's
                // is (LLP 1069.011.000 D1); anything else is its snapshot.
                if let face = target.face, face.fits, !face.raster, let symbol = face.symbol, let image = FaceSymbol.image(symbol) {
                    image.accessibilityLabel = host.label(target)
                    images[target.id] = image; action.image = image
                    action.title = shownTitle(target)
                    return action
                }
                // A word its author left unstyled (no font or colour written
                // on it or its button), as `delete=`'s Delete (LLP 1116.020
                // D1): UIKit's title, in UIKit's font, not a picture of the word.
                let faces = target.container.subviews.compactMap { $0 as? NodeView }
                if let word = faces.first, faces.count == 1, word.isParagraph, let title = target.face?.title, !title.isEmpty,
                   ![word, target].contains(where: { node in ["font_size", "font_weight", "font_family", "text_color"].contains(where: node.wrote) }) {
                    action.title = title
                    return action
                }
                if let glyph = target.container.subviews.first as? NodeView, !glyph.bounds.isEmpty {
                    func display(_ view: UIView) { view.layer.displayIfNeeded(); for child in view.subviews { display(child) } }
                    display(glyph)
                    // Layer rendering omits the root view's transform. Capture
                    // its transformed box, so an authored icon scale/rotation
                    // survives projection into UIKit's centered image slot.
                    let bounds = glyph.bounds.applying(glyph.transform)
                    if !bounds.isEmpty {
                        let image = UIGraphicsImageRenderer(size: bounds.size).image { context in
                            context.cgContext.translateBy(x: -bounds.minX, y: -bounds.minY)
                            context.cgContext.concatenate(glyph.transform)
                            glyph.layer.render(in: context.cgContext)
                        }.withRenderingMode(.alwaysOriginal)
                        image.accessibilityLabel = host.label(target)
                        images[target.id] = image; action.image = image
                        if !glyph.isParagraph { action.title = shownTitle(target) }
                    }
                } else { action.title = host.label(target) }
                return action
            }
            guard !actions.isEmpty else { return nil }
            DispatchQueue.main.async { [weak self] in self?.nameActions() }
            let configuration = UISwipeActionsConfiguration(actions: actions)
            configuration.performsFirstActionWithFullSwipe = controls.first.map(enabled) ?? false
            return configuration
        }
        #endif

        // Observe public UIKit controls by their label/image. Never infer a
        // target from a private class name, an action's position, or testId.
        private func nameActions() {
            for target in leading + trailing {
                if let button = actionView(target.id, visibleOnly: false) {
                    button.accessibilityLabel = host.label(target)
                    button.accessibilityIdentifier = target.props["id"]
                }
            }
        }
        /// Where UIKit draws the row's actions: its table, or a grouped row's
        /// list, in the span of the row's cell (UIKit opens one cell at a time).
        private var drawn: (root: UIView, span: CGRect?)? {
            if let table { return (table, nil) }
            guard grouped, let list = host.presenter.groupedLists?.scroller(for: owner.id),
                  let cell = sequence(first: owner as UIView, next: { $0.superview }).first(where: { $0 is UICollectionViewCell }),
                  cell.isDescendant(of: list) else { return nil }
            return (list, cell.convert(cell.bounds, to: list))
        }
        func actionView(_ id: UInt32, visibleOnly: Bool = true) -> UIButton? {
            guard let (table, span) = drawn, let target = (leading + trailing).first(where: { $0.id == id }) else { return nil }
            var matches: [UIButton] = []
            func hasImage(_ view: UIView) -> Bool {
                if let actual = (view as? UIImageView)?.image, let expected = images[id], actual === expected || actual.isEqual(expected) { return true }
                return view.subviews.contains(where: hasImage)
            }
            func visit(_ view: UIView) {
                // UIKit's rendered actions only: never the authored row, its
                // body or its hidden actions, so a native body button labelled
                // like an action is not taken for it (LLP 1069.011.000 D6).
                if view is NodeView { return }
                if visibleOnly && (view.isHidden || view.alpha <= 0.01) { return }
                if let button = view as? UIButton,
                   (!visibleOnly || table.bounds.intersects(button.convert(button.bounds, to: table))),
                   span.map({ $0.contains(CGPoint(x: $0.midX, y: button.convert(button.bounds, to: table).midY)) }) ?? true,
                   button.accessibilityLabel == host.label(target) || hasImage(button) { matches.append(button) }
                for child in view.subviews { visit(child) }
            }
            visit(table)
            guard matches.count == 1 else { return nil }
            return matches[0]
        }
    }
}
/// The swipe row's table: a pan cancels a touch in a native button in the
/// row's body, as it does a custom button's (LLP 1069.011.000 D6).
private final class SwipeTable: UITableView {
    override func touchesShouldCancel(in view: UIView) -> Bool {
        view is NativeButtonIOS || super.touchesShouldCancel(in: view)
    }
}
#endif
