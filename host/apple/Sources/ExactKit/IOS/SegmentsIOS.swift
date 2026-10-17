// @ref LLP 1035.001 D10 — standard tab semantics project to the platform's
// segmented control, or — when every tab is a symbol over its label, which is
// a tab bar item's own shape — to a tab bar (LLP 1059). Contract remains the
// state owner; UIKit owns the control. Layout stays authored: the control
// fills the tablist's box and reports its native minimum height to layout.
// @ref LLP 1116.004 D5.2 — a radio group of radios (a value chosen from a
// few) is the same segmented control, its selection `aria-checked`.
#if os(iOS) || os(tvOS)
import UIKit

/// A segmented control whose segments carry their tabs' names and test ids
/// (LLP 1116 D2, D6): a title segment's `aria-label`, when it says more than
/// its words, is its accessibility label, and a tab's `testId` its
/// accessibility identifier, so XCUITest and Maestro address it as any
/// other node. UIKit has no API for either on a segment: they are written
/// on its segment views, in order, after each layout, which is when UIKit
/// makes them. A segment named by nothing is UIKit's (its title).
class LabelledSegmentedControl: UISegmentedControl {
    var segmentLabels: [String?] = [] { didSet { if segmentLabels != oldValue { setNeedsLayout() } } }
    var segmentIDs: [String?] = [] { didSet { if segmentIDs != oldValue { setNeedsLayout() } } }
    var segmentUnavailable: [Bool] = [] { didSet { if segmentUnavailable != oldValue { setNeedsLayout() } } }
    override func layoutSubviews() {
        super.layoutSubviews()
        labelSegments()
    }
    /// The segment views (`UISegment`, two containers down on iOS 27), in
    /// segment order: left to right, reversed right to left.
    var segmentViews: [UIView] {
        var found: [UIView] = []
        func walk(_ view: UIView, _ depth: Int) {
            for child in view.subviews {
                if String(describing: type(of: child)) == "UISegment" { found.append(child) } else if depth < 3 { walk(child, depth + 1) }
            }
        }
        walk(self, 0)
        let views = found.enumerated().sorted { a, b in
            let x = (a.element.convert(a.element.bounds, to: self).minX, b.element.convert(b.element.bounds, to: self).minX)
            return x.0 == x.1 ? a.offset < b.offset : x.0 < x.1
        }.map(\.element)
        return effectiveUserInterfaceLayoutDirection == .rightToLeft ? views.reversed() : views
    }
    func labelSegments() {
        let views = segmentViews
        guard views.count == numberOfSegments else { return }
        for (index, view) in views.enumerated() {
            let label = segmentLabels.indices.contains(index) ? segmentLabels[index] : nil
            let id = segmentIDs.indices.contains(index) ? segmentIDs[index] : nil
            if view.accessibilityLabel != label { view.accessibilityLabel = label }
            if view.accessibilityIdentifier != id { view.accessibilityIdentifier = id }
            let unavailable = segmentUnavailable.indices.contains(index) && segmentUnavailable[index]
            if unavailable { view.accessibilityTraits.insert(.notEnabled) }
            else { view.accessibilityTraits.remove(.notEnabled) }
        }
    }
}

private final class ExactSegmentedControl: LabelledSegmentedControl {
    let ownerID: UInt32
    var icons: [Int: (source: AnyObject, size: CGSize, label: String)] = [:]
    /// The selection last reported or applied. A finger moves the selection
    /// before the change is reported (on iOS 26 after the lift, as the
    /// selection settles); until then the control keeps the newer choice.
    var settled = UISegmentedControl.noSegment
    init(ownerID: UInt32) {
        self.ownerID = ownerID
        super.init(items: [])
    }
    required init?(coder: NSCoder) { nil }
}

private final class ExactTabBar: UITabBar {
    let ownerID: UInt32
    var onMeasure: (() -> Void)?
    override func layoutSubviews() {
        super.layoutSubviews()
        onMeasure?()
    }
    init(ownerID: UInt32) {
        self.ownerID = ownerID
        super.init(frame: .zero)
    }
    required init?(coder: NSCoder) { nil }
}

/// A tab bar item that remembers the symbol it was made with.
private final class TabBarItem: UITabBarItem { var made = "" }
private extension UITabBarItem { var madeWith: String? { (self as? TabBarItem)?.made } }

/// A tab a tab bar item can show: one symbol and one label, nothing else.
private struct TabBarFace: Equatable {
    let symbol: String
    let title: String
    /// Its accessible name: the tab's explicit label, else its title.
    let label: String
    /// The selected item's colour when the author gave one; nil leaves the
    /// bar UIKit's tint (LLP 1115 D4).
    let tint: UIColor?

    init?(_ tab: NodeView) {
        // A native tab's face, from the kernel; its tint its accent
        // (LLP 1069.011.000 D4).
        if tab.isNativeButton {
            guard let face = tab.face, face.fits, let symbol = face.symbol, let title = face.title else { return nil }
            self.symbol = symbol
            self.title = title
            label = tab.props["accessibilityLabel"] ?? title
            tint = tab.channels("accent_color").map { TextEngine.color($0) }
            return
        }
        let children = tab.container.subviews.compactMap { $0 as? NodeView }
        guard children.count == 2,
              let image = children.first(where: { $0.kind == "image" }),
              image.props["imageSource"]?.hasPrefix("symbol:") == true,
              let label = children.first(where: { $0 !== image }), label.isParagraph,
              !label.accessibleText.isEmpty else { return nil }
        self.symbol = image.props["symbolName"] ?? ""
        title = label.accessibleText
        self.label = tab.props["accessibilityLabel"] ?? title
        tint = image.ownUIColor("tint_color")
    }
}

final class SegmentHost: NSObject, UIGestureRecognizerDelegate, UITabBarDelegate {
    unowned let presenter: Presenter
    private var controls: [UInt32: ExactSegmentedControl] = [:]
    /// The segmented control a tablist or radio group projects to, if any:
    /// a hatched node's platform object (LLP 1075.003.000 §3.2).
    func control(of id: UInt32) -> UISegmentedControl? { controls[id] }
    private var bars: [UInt32: ExactTabBar] = [:]
    private var sizes: [UInt32: CGSize] = [:]
    /// A segmented control's own size, with what it was measured from: its
    /// segments, their fonts and the text size. Measuring is UIKit laying out
    /// every segment, and a sync runs after every batch (a fling's fills).
    private var naturals: [UInt32: (source: String, size: CGSize)] = [:]
    private var hidden: [UInt32: Bool] = [:]
    private var members: [UInt32: [UInt32]] = [:]
    /// Tablists a tab container's bar has taken the place of, hidden here.
    private var adoptedLists = Set<UInt32>()
    /// The last projection decision journaled per tablist, so each is said once.
    private var decisions: [UInt32: String] = [:]
    /// LLP 1080.001 D3: the bar or control standing in for a tablist, and
    /// the tabs and adopted lists hidden for it.
    func inspectionOwns(_ view: UIView) -> Bool { bars.values.contains { $0 === view } || controls.values.contains { $0 === view } }
    func hides(_ node: NodeView) -> Bool { members.values.contains { $0.contains(node.id) } || adoptedLists.contains(node.id) }

    init(_ presenter: Presenter) { self.presenter = presenter; super.init() }

    private func contextTab(_ gesture: UIGestureRecognizer) -> NodeView? {
        guard let control = gesture.view as? ExactSegmentedControl,
              let ids = members[control.ownerID], control.bounds.width > 0,
              let owner = presenter.views[control.ownerID], available(owner) else { return nil }
        let point = gesture.location(in: control)
        guard control.bounds.contains(point) else { return nil }
        var index = Int(point.x / control.bounds.width * CGFloat(ids.count))
        if control.effectiveUserInterfaceLayoutDirection == .rightToLeft { index = ids.count - 1 - index }
        guard ids.indices.contains(index), let tab = presenter.views[ids[index]],
              !tab.disabled, tab.handlers.contains("contextmenu") else { return nil }
        return tab
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        contextTab(gestureRecognizer) != nil
    }

    @objc private func longPressed(_ gesture: UILongPressGestureRecognizer) {
        guard gesture.state == .began, let tab = contextTab(gesture),
              let control = gesture.view as? ExactSegmentedControl else { return }
        // UIKit cancels the pending segment tap. Opening a context action must
        // not first navigate to that tab, nor commit selection when lifted.
        control.cancelTracking(with: nil)
        #if !os(tvOS)
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
        #endif
        presenter.contextmenu(tab.id)
    }

    /// A tablist's pressable tabs, or a radio group's pressable radios
    /// (LLP 1116.004 D5.2: a value chosen from a few is the same control).
    private func tabs(in owner: NodeView) -> [NodeView] {
        let role = Self.member(of: owner)
        return owner.container.subviews.compactMap { $0 as? NodeView }.filter {
            $0.isButton && $0.props["accessibilityRole"] == role && $0.handlers.contains("press")
        }
    }
    static func member(of owner: NodeView) -> String { owner.props["accessibilityRole"] == "radiogroup" ? "radio" : "tab" }
    /// The chosen member: a tab `aria-selected`, a radio `aria-checked`.
    static func chosen(_ tab: NodeView) -> Bool {
        tab.props[tab.props["accessibilityRole"] == "radio" ? "accessibilityChecked" : "accessibilitySelected"] == "true"
    }

    /// Whether the owner takes input. `visibility` is read on the owner
    /// alone: its value is computed, and a box under a hidden one may set it
    /// back to `visible`.
    private func available(_ owner: UIView) -> Bool {
        if (owner as? NodeView)?.cssVisibilityHidden == true { return false }
        var ancestor: UIView? = owner
        while let view = ancestor {
            if view.isHidden || (view as? NodeView)?.props["inert"] == "true" { return false }
            ancestor = presenter.eventParent(of: view)
        }
        return owner.window != nil
    }

    /// An image-only authored tab stays image-only in UIKit. Its accessible
    /// name belongs to the segment image; it is not a visible fallback title.
    /// A text-only tab's words are its title, and an `aria-label` that says
    /// otherwise its accessibility label (`SegmentFace`, LLP 1116 D6).
    private func content(_ tab: NodeView, at index: Int, in control: ExactSegmentedControl) {
        let label = tab.accessibleName
        if case .image(let icon)? = tab.segmentFace {
            let raster = icon.raster?.image.image
            // A transparent image carries the segment's accessible name when
            // the OS has no glyph; a nil image loses that native label.
            let source = raster.map({ UIImage(cgImage: $0) }) ?? icon.image
            let identity: AnyObject = raster.map { $0 as AnyObject } ?? source ?? icon
            let size = CGSize(width: max(1, icon.bounds.width), height: max(1, icon.bounds.height))
            if let old = control.icons[index], old.source === identity, old.size == size, old.label == label { return }
            // An image segment shows no title (a title-only face had set one).
            if control.titleForSegment(at: index)?.isEmpty == false { control.setTitle(nil, forSegmentAt: index) }
            let image = UIGraphicsImageRenderer(size: size).image { _ in
                guard let source else { return }
                let ratio = min(size.width / source.size.width, size.height / source.size.height)
                let fit = CGSize(width: source.size.width * ratio, height: source.size.height * ratio)
                source.draw(in: CGRect(x: (size.width - fit.width) / 2, y: (size.height - fit.height) / 2, width: fit.width, height: fit.height))
            }.withRenderingMode(.alwaysOriginal)
            image.accessibilityLabel = label
            control.setImage(image, forSegmentAt: index)
            control.icons[index] = (identity, size, label)
        } else if case .symbol(let name)? = tab.segmentFace {
            // A native tab's symbol, carrying its label (LLP 1069.011.000 D4).
            let size = CGSize(width: -1, height: -1)
            if let old = control.icons[index], (old.source as? NSString) == name as NSString, old.label == label { return }
            let image = UIImage(systemName: name) ?? UIGraphicsImageRenderer(size: CGSize(width: 1, height: 1)).image { _ in }
            if control.titleForSegment(at: index)?.isEmpty == false { control.setTitle(nil, forSegmentAt: index) }
            image.accessibilityLabel = label
            control.setImage(image, forSegmentAt: index)
            control.icons[index] = (name as NSString, size, label)
        } else if case .title(let title)? = tab.segmentFace {
            control.icons.removeValue(forKey: index)
            if control.imageForSegment(at: index) != nil { control.setImage(nil, forSegmentAt: index) }
            if control.titleForSegment(at: index) != title { control.setTitle(title, forSegmentAt: index) }
        }
    }

    /// Journal why a tablist or radio group is or is not a segmented control, once per change.
    private func decide(_ owner: NodeView, _ decision: String) {
        guard decisions[owner.id] != decision else { return }
        decisions[owner.id] = decision
        presenter.session?.log("\(owner.props["accessibilityRole"] ?? "tablist") #\(owner.id): \(decision)")
    }

    private func restore(owner id: UInt32) {
        for childID in members.removeValue(forKey: id) ?? [] {
            if let child = presenter.views[childID] { child.isHidden = hidden.removeValue(forKey: childID) ?? false }
            else { hidden.removeValue(forKey: childID) }
        }
        controls.removeValue(forKey: id)?.removeFromSuperview()
        removeBar(owner: id)
        clearSize(owner: id)
    }

    private func removeBar(owner id: UInt32) {
        guard let bar = bars.removeValue(forKey: id) else { return }
        bar.removeFromSuperview()
        clearSize(owner: id)
    }

    private func clearSize(owner id: UInt32) {
        naturals.removeValue(forKey: id)
        if sizes.removeValue(forKey: id) != nil, let owner = presenter.views[id] {
            presenter.queueIntrinsicSize(owner, generation: owner.loadGeneration, nil)
        }
    }

    private func natural(_ id: UInt32, _ control: UISegmentedControl) -> CGSize {
        let fonts = [UIControl.State.normal, .selected].map { (control.titleTextAttributes(for: $0)?[.font] as? UIFont).map { "\($0.fontName) \($0.pointSize)" } ?? "" }
        let segments = (0..<control.numberOfSegments).map { "\(control.titleForSegment(at: $0) ?? "")|\(control.imageForSegment(at: $0)?.size ?? .zero)|\(control.widthForSegment(at: $0))" }
        let t = control.traitCollection
        let source = "\(segments)|\(fonts)|\(t.preferredContentSizeCategory.rawValue)|\(t.legibilityWeight.rawValue)"
        // A hatch's own look (a background image, a divider) sizes it too:
        // such a control is measured every time, as before.
        let customized = control.backgroundImage(for: .normal, barMetrics: .default) != nil
            || control.dividerImage(forLeftSegmentState: .normal, rightSegmentState: .normal, barMetrics: .default) != nil
        if !customized, let known = naturals[id], known.source == source { return known.size }
        // As equal segments: the width they fit in before the control
        // apportions a narrower box by their content (`sync`).
        let apportioned = control.apportionsSegmentWidthsByContent
        if apportioned { control.apportionsSegmentWidthsByContent = false }
        let size = control.intrinsicContentSize
        if apportioned { control.apportionsSegmentWidthsByContent = true }
        naturals[id] = customized ? nil : (source, size)
        return size
    }

    private func measure(_ owner: NodeView, _ control: UIView) {
        guard bars[owner.id] === control || controls[owner.id] === control, presenter.views[owner.id] === owner else { return }
        var size: CGSize
        if control is UISegmentedControl {
            // Its own height, at any width of the box: its height does not
            // follow that width, so a resize does not remeasure. Its width
            // is the narrowest it can be (LLP 1116.004 D2), the tablist's
            // automatic minimum: none, as UIKit fits its segments to any
            // box, so a row's other parts never push it past the row's edge
            // (unit-converter: four text tabs overflowed a grouped row by
            // the hidden tabs' own widths). It fills the content box, so a
            // border-box minimum also holds the padding and border around it.
            let natural = natural(owner.id, control as! UISegmentedControl)
            size = CGSize(width: 1, height: natural.height)
            // The padding and border around the content box layout sent
            if owner.style["box_sizing"]?.string == "border-box" { size.height += owner.bounds.height - owner.contentBox().height }
        } else {
            guard owner.bounds.width > 0 else { return }
            // A tab bar lays its items out in any width: none is its narrowest.
            size = CGSize(width: 1, height: control.sizeThatFits(CGSize(width: owner.bounds.width, height: 0)).height)
        }
        guard size.width.isFinite, size.height.isFinite, size.height > 0 else { return }
        guard sizes[owner.id] != size else { return }
        sizes[owner.id] = size
        presenter.queueIntrinsicSize(owner, generation: owner.loadGeneration, size)
    }

    /// Hide the authored tabs the control stands in for, remembering how
    /// they were, once per membership.
    private func adopt(_ owner: NodeView, _ tabs: [NodeView]) {
        let ids = tabs.map(\.id)
        if members[owner.id] != ids {
            restore(owner: owner.id)
            members[owner.id] = ids
            for tab in tabs { hidden[tab.id] = tab.hiddenByHost; tab.isHidden = true }
        } else {
            for tab in tabs { tab.isHidden = true }
        }
    }

    /// Symbol-over-label tabs as a tab bar in the tablist's box. The tints
    /// are the authored ones — a selected symbol's and an unselected one's —
    /// so an app's accent is the tab bar's; unsaid, the platform's tint.
    private func project(_ owner: NodeView, _ tabs: [NodeView], _ faces: [TabBarFace]) {
        controls.removeValue(forKey: owner.id)?.removeFromSuperview()
        adopt(owner, tabs)
        let bar = bars[owner.id] ?? {
            let value = ExactTabBar(ownerID: owner.id)
            value.delegate = self
            value.onMeasure = { [weak self, weak owner, weak value] in
                if let owner, let value { self?.measure(owner, value) }
            }
            owner.addSubview(value)
            bars[owner.id] = value
            return value
        }()
        if bar.superview !== owner { owner.addSubview(bar) }
        if bar.frame != owner.bounds { bar.frame = owner.bounds }
        assign(bar, \.isUserInteractionEnabled, available(owner))
        assign(bar, \.accessibilityLabel, owner.props["accessibilityLabel"])
        let current = bar.items ?? []
        if current.count != faces.count || zip(current, faces).contains(where: { $0.title != $1.title || $0.madeWith != $1.symbol }) {
            bar.setItems(faces.enumerated().map { index, face in
                let item = TabBarItem(title: face.title, image: face.symbol.isEmpty ? nil : UIImage(systemName: face.symbol), tag: index)
                item.made = face.symbol
                return item
            }, animated: false)
        }
        for ((item, tab), face) in zip(zip(bar.items ?? [], tabs), faces) {
            assign(item, \.isEnabled, !tab.disabled)
            if tab.disabled || tab.ariaUnavailable { item.accessibilityTraits.insert(.notEnabled) }
            else { item.accessibilityTraits.remove(.notEnabled) }
            // Its test id is the tab's (LLP 1116 D2).
            if item.accessibilityIdentifier != tab.props["testId"] { item.accessibilityIdentifier = tab.props["testId"] }
            // Its name is the tab's, as the hidden tab's was (astra's code review).
            if item.accessibilityLabel != face.label { item.accessibilityLabel = face.label }
        }
        let selected = tabs.firstIndex { $0.props["accessibilitySelected"] == "true" }
        let item = selected.flatMap { bar.items?[$0] }
        if bar.selectedItem !== item { bar.selectedItem = item }
        // The selected item takes the authored accent, else UIKit's tint;
        // the rest keep the bar's own face. (Title attributes through a UITabBarAppearance
        // replace the bar's own rendering of every title; not used.)
        if let selected, bar.tintColor != faces[selected].tint { bar.tintColor = faces[selected].tint }
        owner.bringSubviewToFront(bar)
        measure(owner, bar)
    }

    func tabBar(_ tabBar: UITabBar, didSelect item: UITabBarItem) {
        guard let bar = tabBar as? ExactTabBar, let ids = members[bar.ownerID], ids.indices.contains(item.tag),
              let tab = presenter.views[ids[item.tag]], !tab.disabled,
              let owner = presenter.views[bar.ownerID], available(owner) else { sync(); return }
        presenter.press(tab.id)
    }

    func sync() {
        // @ref LLP 1039 D6 — only explicit vertical tablists opt out; ignore invalid ARIA values.
        // A tablist whose place a tab container's bar takes is the container's
        // (LLP 1075.003 §3.7): it is hidden, and projects nothing of its own.
        // A radio group of two or more radios is the same segmented control
        // (LLP 1116.004 D5.2); it is never a tab bar.
        let owners = presenter.carrying("role:tablist").filter {
            $0.props["accessibilityRole"] == "tablist" &&
                $0.props["accessibilityOrientation"] != "vertical" && !presenter.navigation.adopts(tablist: $0) &&
                !($0.wrote("appearance") && $0.style["appearance"]?.string == "none")
        } + presenter.carrying("role:radiogroup").filter {
            $0.props["accessibilityRole"] == "radiogroup" && $0.props["accessibilityOrientation"] != "vertical"
        }
        for list in presenter.carrying("role:tablist") {
            let adopted = presenter.navigation.adopts(tablist: list)
            guard adopted != adoptedLists.contains(list.id) else { continue }
            if adopted { adoptedLists.insert(list.id) } else { adoptedLists.remove(list.id) }
            list.isHidden = adopted
        }
        let live = Set(owners.map(\.id))
        for id in Array(controls.keys) + Array(bars.keys) where !live.contains(id) { restore(owner: id) }
        for id in Array(decisions.keys) where !live.contains(id) { decisions.removeValue(forKey: id) }
        for owner in owners {
            let tabs = tabs(in: owner)
            let radios = Self.member(of: owner) == "radio"
            let faces = radios ? [] : tabs.compactMap(TabBarFace.init)
            if tabs.count > 1, faces.count == tabs.count {
                owner.accessibilityTraits.remove(.tabBar)
                decide(owner, "projected to UITabBar (\(tabs.count) items)")
                project(owner, tabs, faces)
                continue
            }
            removeBar(owner: owner.id)
            // Authored tabs a segment cannot show stay as authored, and the
            // tablist tells VoiceOver it is a tab bar (LLP 1035.001 D10).
            let unshown = tabs.first { $0.segmentFace == nil }
            if tabs.count <= 1 || unshown != nil {
                restore(owner: owner.id)
                if !radios { owner.accessibilityTraits.insert(.tabBar) }
                let member = radios ? "radio" : "tab"
                decide(owner, unshown.map { "kept as authored: \(member) #\($0.id) is not one image or its label alone, which a segment cannot show" } ?? "kept as authored: fewer than two pressable \(member)s")
                continue
            }
            owner.accessibilityTraits.remove(.tabBar)
            decide(owner, "projected to UISegmentedControl (\(tabs.count) segments)")
            adopt(owner, tabs)
            let control = controls[owner.id] ?? {
                let value = ExactSegmentedControl(ownerID: owner.id)
                value.addTarget(self, action: #selector(changed(_:)), for: .valueChanged)
                let context = UILongPressGestureRecognizer(target: self, action: #selector(longPressed(_:)))
                context.delegate = self
                value.addGestureRecognizer(context)
                value.autoresizingMask = [.flexibleWidth, .flexibleTopMargin, .flexibleBottomMargin]
                owner.addSubview(value)
                controls[owner.id] = value
                return value
            }()
            if control.superview !== owner { owner.addSubview(control) }
            assign(control, \.isEnabled, available(owner))
            control.accessibilityLabel = owner.props["accessibilityLabel"]
            if control.numberOfSegments != tabs.count {
                control.removeAllSegments()
                control.icons.removeAll()
                for index in tabs.indices { control.insertSegment(withTitle: "", at: index, animated: false) }
            }
            for (index, tab) in tabs.enumerated() {
                content(tab, at: index, in: control)
                control.setEnabled(!tab.disabled, forSegmentAt: index)
            }
            control.accessibilityIdentifier = owner.props["testId"]
            control.segmentLabels = tabs.map(Self.segmentLabel)
            control.segmentIDs = tabs.map { $0.props["testId"] }
            control.segmentUnavailable = tabs.map { $0.disabled || $0.ariaUnavailable }
            let selected = tabs.firstIndex(where: Self.chosen) ?? UISegmentedControl.noSegment
            // A choice the finger made and the control has not reported yet
            // stays; resetting it would make the report name the old segment.
            let pending = control.isTracking || control.selectedSegmentIndex != control.settled
            if control.selectedSegmentIndex != selected, !pending { control.selectedSegmentIndex = selected }
            if control.selectedSegmentIndex == selected { control.settled = selected }
            // As wide as the content box and its own height, centred in it
            // (LLP 1116.019 D1): a taller box (an authored height, a stretch)
            // does not stretch UIKit's control, as a hand-built screen's
            // never is.
            let box = owner.contentBox()
            let height = min(box.height, natural(owner.id, control).height)
            let frame = CGRect(x: box.minX, y: box.midY - height / 2, width: box.width, height: height)
            if control.frame != frame { control.frame = frame }
            owner.bringSubviewToFront(control)
            measure(owner, control)
            // Equal segments where they fit; in a narrower box UIKit gives
            // each segment its content's share, so "Temperature" keeps its
            // letters beside "Length" (LLP 1116.004 D2).
            assign(control, \.apportionsSegmentWidthsByContent, frame.width + 0.5 < natural(owner.id, control).width)
        }
    }

    /// A title segment's accessibility label: its tab's name where that is
    /// not its words (an image segment's is its image's, `content`).
    static func segmentLabel(_ tab: NodeView) -> String? {
        guard case .title(let title)? = tab.segmentFace, tab.accessibleName != title else { return nil }
        return tab.accessibleName
    }

    @objc private func changed(_ sender: ExactSegmentedControl) {
        sender.settled = sender.selectedSegmentIndex
        guard let ids = members[sender.ownerID], ids.indices.contains(sender.selectedSegmentIndex),
              let tab = presenter.views[ids[sender.selectedSegmentIndex]], !tab.disabled,
              let owner = presenter.views[sender.ownerID], available(owner) else { sync(); return }
        presenter.press(tab.id)
    }

    /// Agent activation names the authored tab even though UIKit owns its pixels.
    func activate(_ node: NodeView) -> Bool? {
        guard let entry = members.first(where: { $0.value.contains(node.id) }) else { return nil }
        guard let owner = presenter.views[entry.key], available(owner), !node.disabled else { return false }
        presenter.press(node.id)
        return true
    }

    /// Where a finger aimed at tab `node` lands, for a segment a control
    /// draws (LLP 1080.000 D4; r5 weather): the control, and the segment's
    /// box in it — its segment view's, else an equal share. Nil for a node no
    /// segmented control draws (a tab bar's items are UIKit's private views).
    func drawn(_ node: NodeView) -> (control: UIControl, segment: CGRect)? {
        guard let entry = members.first(where: { $0.value.contains(node.id) }), let control = controls[entry.key],
              let index = entry.value.firstIndex(of: node.id), control.numberOfSegments == entry.value.count else { return nil }
        let views = control.segmentViews
        if views.count == control.numberOfSegments { return (control, views[index].convert(views[index].bounds, to: control)) }
        let share = control.bounds.width / CGFloat(entry.value.count)
        let slot = control.effectiveUserInterfaceLayoutDirection == .rightToLeft ? entry.value.count - 1 - index : index
        return (control, CGRect(x: share * CGFloat(slot), y: 0, width: share, height: control.bounds.height))
    }

    func observation(_ node: NodeView) -> [String: Any]? {
        if let entry = members.first(where: { $0.value.contains(node.id) }), let bar = bars[entry.key],
           let index = entry.value.firstIndex(of: node.id) {
            return ["view": "UITabBar", "item": index, "items": bar.items?.count ?? 0,
                    "selected": bar.selectedItem.map(\.tag) ?? -1]
        }
        guard let entry = members.first(where: { $0.value.contains(node.id) }),
              let control = controls[entry.key], let segment = entry.value.firstIndex(of: node.id) else { return nil }
        return ["view": "UISegmentedControl", "segment": segment,
                "selected": control.selectedSegmentIndex, "segments": control.numberOfSegments]
    }

    func reset() {
        for id in Array(members.keys) { restore(owner: id) }
        decisions.removeAll()
    }
}
#endif
