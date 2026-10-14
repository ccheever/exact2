// @ref LLP 1115.002 D7.4 — a modal route is a sheet. A route presented
// modally (`navigationPresentation="modal"`) over the route beneath it is,
// in a hand-built Mac app, a sheet attached to the window: a window of its
// own that AppKit centres on its parent, dims it under and keeps it from
// taking a click. It is the sheet a `dialog` is (SheetsMac.swift) and is
// kept by the same host, as an entry whose node is the route: the route's
// own subtree moves into the sheet's window, so its fields, their text, the
// focus and every test id are the authored ones, and the kernel lays it out
// in a containing block (D8.2) whose used size is the sheet's.
//
// **Its size.** The block is a maximum on each axis: the content area of
// the page's window, less this sheet's own rows. With no authored size the
// route fits its content under it, as a `dialog` does, and its `inset` is
// not read, since AppKit places the sheet. An authored `width` or `height`
// is used as written (a percentage is of the maximum) and wins over the
// content. It does not win over the window: a route wider than the window's
// content area stops there, where AppKit's own sheet of a fixed size
// overhangs its parent. A block that *is* the authored size needs that size
// here, and a host is sent no layout row (`width`, `height`, `inset`).
//
// **Its header**, read by the shared rule (`HeaderRule`), is laid out as
// SwiftUI lays a sheet's out (measured on macOS 27.0.1,
// `~/audits/exact2-no-tells-2026-10-11/mac-structure/png/sheet-swiftui-new-book.png`):
// the heading is a title row at the top, 48 points with a hairline under
// it, and the two actions the rule can identify are AppKit's push buttons
// in a row at the bottom right, 64 points under a hairline: the one that
// cancels (the stack's Back control, `navigationBack`) first, then the one
// that confirms (the header's one prominent, non-destructive button), which
// is the sheet's default button and answers Return. Each presses its
// authored button. What they stand for is hidden in the page and its box is
// the platform's (`HostCovers`, D8.1): the whole header when it shows
// nothing else, else the heading and the two buttons, and anything else the
// header holds (a search field, a menu's button, a third action) stays in
// it, where the author put it, at the top of the sheet's content.
//
// **Kept painted, whole.** A header whose heading cannot be taken out of it
// (it is inside a pressable) keeps the route painted in the window as
// before, on its ground (`ModalGround`), and the session's log says why. So
// does the driver's own chrome (`--chrome agent`), a route with no window to
// attach to, and a route pushed over a modal one, which is not a sheet's
// content yet.
//
// **What dismisses it** is the route's `closedby`, as a dialog's: unwritten
// or `closerequest`, Escape and Command-period; `none`, only its own buttons
// and the app; `any`, a click on the window behind it too. The dismissal is
// the stack's way back (BackMac.swift: its Back control pressed, else the
// root's `navigate`, else the router's `back`), so the app decides, and the
// sheet leaves when the route is no longer the one shown.
#if os(macOS)
import AppKit

/// What a sheet shows of a modal route's header.
struct SheetHeader {
    let header: NodeView, heading: NodeView
    /// The heading's words: the sheet's title row.
    let title: String
    /// The one that cancels, and the one that confirms.
    let cancel: NodeView?, confirm: NodeView?
    /// The header shows nothing else: its whole box is the platform's.
    let whole: Bool

    /// The header's nodes the sheet's own rows stand for.
    var stoodFor: [NodeView] { whole ? [header] : [heading] + [cancel, confirm].compactMap { $0 } }

    enum Read {
        /// No header the sheet's rows could be made from: the route is its content.
        case none
        /// A header the sheet cannot take its title out of, and why: the route stays painted.
        case kept(String)
        case shown(SheetHeader)
    }

    /// The styles AppKit draws as its accent push button (`buttonStyles`' macOS column).
    private static let prominent: Set<String> = ["filled", "bordered-prominent", "prominent-glass", "prominent-clear-glass"]

    /// The words on a button: its title, else its name (a close symbol's `aria-label`).
    static func words(of button: NodeView) -> String {
        let shown = button.isNativeButton ? button.face?.title ?? "" : button.accessibleText
        return shown.isEmpty ? button.authoredLabel ?? "" : shown
    }

    /// `route`'s header as its sheet shows it. `back` names the stack's Back control.
    static func read(route: NodeView, back: String?) -> Read {
        guard let rule = HeaderRule(route: route, back: back, backIsPlatforms: false, projected: true) else { return .none }
        if let tap = rule.tap { return .kept("its heading is inside a pressable (#\(tap.id)), and a sheet's title is not pressed") }
        let children = { (node: NodeView) in node.container.subviews.compactMap { $0 as? NodeView } }
        // A push button says words and presses: one that opens a menu, shows no word or is not shown is not one.
        let button = { (node: NodeView) in
            node.pressable && node.props["popovertarget"] == nil && node.style["display"]?.string != "none" && !words(of: node).isEmpty
        }
        let cancel = back.flatMap { name in rule.items.first { $0.props["id"] == name && button($0) } }
        let accented = rule.items.filter { $0 !== cancel && prominent.contains($0.face?.style ?? "") && $0.props["destructive"] != "true" }
        let confirm = accented.count == 1 && button(accented[0]) ? accented[0] : nil
        func others(in node: NodeView) -> Bool {
            children(node).contains { child in
                if child.style["display"]?.string == "none" || child === rule.heading || child === cancel || child === confirm { return false }
                // A box that only groups shows nothing itself; what it holds decides.
                let groups = child.kind == "view" && !child.pressable && child.props["popover"] == nil && child.props["semanticTag"] != "dialog"
                return !groups || others(in: child)
            }
        }
        return .shown(SheetHeader(header: rule.header, heading: rule.heading, title: rule.heading.accessibleText,
                                  cancel: cancel, confirm: confirm, whole: !others(in: rule.header)))
    }
}

/// A line between a sheet's row and its content, as SwiftUI's sheet draws
/// the two (measured): under the title row a shadow, black at a tenth and
/// then at a twenty-fifth, in light and dark alike; over the buttons the
/// separator's colour.
final class RouteChromeLine: NSView {
    private let soft: Bool
    init(soft: Bool) { self.soft = soft; super.init(frame: .zero) }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func draw(_ dirtyRect: NSRect) {
        guard soft else { NSColor.separatorColor.setFill(); return bounds.fill(using: .sourceOver) }
        for (row, alpha) in [0.1, 0.04].enumerated() {
            NSColor.black.withAlphaComponent(alpha).setFill()
            NSRect(x: 0, y: CGFloat(row), width: bounds.width, height: 1).fill(using: .sourceOver)
        }
    }
}

/// A sheet's own push button. It is a Tab stop as Keyboard Navigation has
/// an `NSButton`, by the setting the session reads (a drive's or a test's
/// stands in for the person's, KeyViewLoopMac.swift).
final class RouteChromeButton: NSButton {
    override var acceptsFirstResponder: Bool { KeyboardNavigation.on && isEnabled }
    override var canBecomeKeyView: Bool { acceptsFirstResponder && !isHiddenOrHasHiddenAncestor }
}

/// A modal route's sheet, as its window's content: the title row, the
/// route's own content, and the row of buttons, in SwiftUI's measures.
final class RouteChrome: NSView {
    private(set) weak var content: BlockView?
    let title = NSTextField(labelWithString: "")
    let cancel: NSButton = RouteChromeButton(title: "", target: nil, action: nil), confirm: NSButton = RouteChromeButton(title: "", target: nil, action: nil)
    /// The buttons Tab reaches after the route's own stops, with Keyboard Navigation on.
    var stops: [NSView] { KeyboardNavigation.on ? buttons.filter(\.isEnabled) : [] }
    /// Whether `view` is one of the rows' own views: the sheet's, as the route in it is.
    func owns(_ view: NSView) -> Bool { view === cancel || view === confirm || view === title }
    private let above = RouteChromeLine(soft: true), below = RouteChromeLine(soft: false)
    /// A row's press: the authored button's id.
    var onPress: ((UInt32) -> Void)?
    private var frozen: NSImageView?

    /// The title row and the button row, each with a line on its content's side; a button's inset from the
    /// sheet's edges and from its neighbour; and the title's label, its words' ink starting 49 points in on a
    /// baseline 30 points down.
    static let titleRow: CGFloat = 48, buttonRow: CGFloat = 64, edge: CGFloat = 20, gap: CGFloat = 8, titleStart: CGFloat = 46, titleBaseline: CGFloat = 30

    init() {
        super.init(frame: .zero)
        title.font = .boldSystemFont(ofSize: NSFont.systemFontSize)
        title.lineBreakMode = .byTruncatingTail
        title.setAccessibilityRole(.staticText)
        for button in [cancel, confirm] {
            button.bezelStyle = .push
            button.target = self
            button.action = #selector(pressed(_:))
        }
        confirm.keyEquivalent = "\r"
        for view in [title, above, below, cancel, confirm] as [NSView] { view.isHidden = true; addSubview(view) }
    }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }

    func hold(_ content: BlockView) {
        self.content = content
        addSubview(content, positioned: .below, relativeTo: nil)
    }

    private var top: CGFloat { title.isHidden ? 0 : Self.titleRow + 1 }
    private var bottom: CGFloat { cancel.isHidden && confirm.isHidden ? 0 : Self.buttonRow + 1 }
    /// The height the rows take beside the content.
    var rows: CGFloat { top + bottom }
    private var buttons: [NSButton] { [cancel, confirm].filter { !$0.isHidden } }
    /// One width for the buttons, the wider one's, as SwiftUI gives a sheet's two actions.
    private var buttonSize: CGSize {
        let sizes = buttons.map { $0.alignmentRect(forFrame: NSRect(origin: .zero, size: $0.fittingSize)).size }
        return CGSize(width: sizes.map(\.width).max() ?? 0, height: sizes.map(\.height).max() ?? 0)
    }
    /// The sheet's size round a content of `size`: never narrower than its buttons.
    func size(around size: CGSize) -> CGSize {
        let count = CGFloat(buttons.count)
        let needed = count == 0 ? 0 : 2 * Self.edge + count * buttonSize.width + (count - 1) * Self.gap
        return CGSize(width: max(size.width, needed), height: size.height + rows)
    }

    /// What the rows show now; true when a row came or went, so the sheet's size is another.
    func show(title words: String?, cancel: (String, UInt32, Bool)?, confirm: (String, UInt32, Bool)?, escapes: Bool) -> Bool {
        let before = rows
        title.isHidden = (words ?? "").isEmpty
        above.isHidden = title.isHidden
        if let words, title.stringValue != words { title.stringValue = words }
        for (button, said) in [(self.cancel, cancel), (self.confirm, confirm)] {
            button.isHidden = said == nil
            guard let (words, id, enabled) = said else { continue }
            if button.title != words { button.title = words }
            button.tag = Int(id)
            if button.isEnabled != enabled { button.isEnabled = enabled }
        }
        // Escape and Command-period are the cancel's, as an AppKit sheet's, unless the route refuses them.
        let escape = escapes ? "\u{1b}" : ""
        if self.cancel.keyEquivalent != escape { self.cancel.keyEquivalent = escape }
        below.isHidden = buttons.isEmpty
        needsLayout = true
        return rows != before
    }

    override func layout() {
        super.layout()
        let size = bounds.size
        if !title.isHidden {
            let fit = title.fittingSize, x = Self.titleStart
            title.frame = NSRect(x: x, y: (Self.titleBaseline - title.firstBaselineOffsetFromTop).rounded(), width: max(0, min(fit.width, size.width - x - Self.edge)), height: fit.height)
            // Its second row falls on the content's first, as the reference's shadow does.
            above.frame = NSRect(x: 0, y: Self.titleRow, width: size.width, height: 2)
        }
        content?.frame = NSRect(x: 0, y: top, width: size.width, height: max(0, size.height - rows))
        frozen?.frame = content?.frame ?? .zero
        guard !buttons.isEmpty else { return }
        below.frame = NSRect(x: 0, y: size.height - bottom, width: size.width, height: 1)
        let each = buttonSize
        var right = size.width - Self.edge
        // From the trailing edge: the one that confirms, then the one that cancels before it.
        for button in buttons.reversed() {
            let rect = NSRect(x: right - each.width, y: size.height - Self.buttonRow + ((Self.buttonRow - each.height) / 2).rounded(), width: each.width, height: each.height)
            button.frame = button.frame(forAlignmentRect: rect)
            right = rect.minX - Self.gap
        }
    }

    @objc private func pressed(_ sender: NSButton) { onPress?(UInt32(sender.tag)) }

    /// The route is being destroyed with its sheet still up: the sheet
    /// leaves showing what it showed, as a person watches a sheet go.
    func freeze() {
        guard frozen == nil, let content, content.bounds.width > 0, content.bounds.height > 0,
              let pixels = content.bitmapImageRepForCachingDisplay(in: content.bounds) else { return }
        content.cacheDisplay(in: content.bounds, to: pixels)
        let image = NSImage(size: content.bounds.size)
        image.addRepresentation(pixels)
        let view = NSImageView(frame: content.frame)
        view.image = image
        view.imageScaling = .scaleNone
        view.setAccessibilityElement(false)
        addSubview(view, positioned: .above, relativeTo: content)
        frozen = view
    }
}

extension NavigationHost {
    /// The modal routes of a stack that are sheets, bottom to top, with what
    /// each sheet shows of its header: the modal routes at the top of the
    /// selected prefix, each standing over the route before it, from the
    /// lowest one above every route that stays painted. `routes[selected]`
    /// is the route the root shows; `concerned`: the batch can have changed
    /// a header, else what was read before stands.
    func sheetRoutes(_ routes: [NodeView], selected: Int, root: NodeView, concerned: Bool) -> [(route: NodeView, header: SheetHeader?)] {
        guard !presenter.dialogs.painted, presenter.viewport.window != nil, routes.indices.contains(selected) else { return [] }
        var found: [(route: NodeView, header: SheetHeader?)] = []
        let back = root.props["navigationBack"].flatMap { $0.isEmpty ? nil : $0 }
        var index = selected
        // A route pushed over a modal one is not a sheet's content yet: both are painted in the window, as before.
        if routes[selected].props["navigationPresentation"] != "modal", let modal = routes[..<selected].last(where: { $0.props["navigationPresentation"] == "modal" }) {
            let why = "route #\(routes[selected].id) is pushed over it, and a sheet holds one route"
            if keptSheets[modal.id] != why { keptSheets[modal.id] = why; presenter.session?.log("route sheet: modal route #\(modal.id) stays painted in the window: \(why)") }
        }
        while index > 0, routes[index].props["navigationPresentation"] == "modal" {
            let route = routes[index]
            if !concerned, keptSheets[route.id] != nil { return found }
            if !concerned, let known = sheets.first(where: { $0.route === route }) { found.insert(known, at: 0) } else {
                switch SheetHeader.read(route: route, back: back) {
                case .none: found.insert((route, nil), at: 0)
                case .shown(let header): found.insert((route, header), at: 0)
                case .kept(let why):
                    // Painted in the window, and so is every route beneath it.
                    if keptSheets[route.id] != why { keptSheets[route.id] = why; presenter.session?.log("route sheet: modal route #\(route.id) stays painted in the window: \(why)") }
                    return found
                }
            }
            keptSheets[route.id] = nil
            index -= 1
        }
        return found
    }
}

extension DialogHost {
    /// The entry of a modal route in its sheet.
    func entry(ofRoute node: NodeView) -> Entry? { entries.first { $0.route && $0.dialog === node } }
    /// Whether the open top modal is a route's sheet, not a dialog.
    var routeActive: Bool { entries.last?.route == true }

    /// After a batch, never inside one (AppKit turns the main queue while a
    /// sheet arrives or leaves): the modal routes navigation shows as
    /// sheets are sheets, and one it no longer shows leaves, the top first.
    func syncRoutes() {
        guard let presenter else { return }
        // A batch that lands while a sheet arrives or leaves is read when it has.
        if routeSync.running { routeSync.again = true; return }
        routeSync.running = true
        defer { routeSync.running = false }
        repeat {
            routeSync.again = false
            var left = false
            for entry in entries.reversed() where entry.route {
                let wanted = presenter.navigation.sheets.contains { $0.route === entry.dialog }
                if !wanted || !live(entry.dialog) { close(entry.dialog); left = true }
            }
            // The routes that left are navigation's to hide again, and those under them its to show.
            if left { presenter.navigation.resync() }
            // A header is read again only by a batch that can have changed it.
            let read = presenter.navigation.sheetsRead
            presenter.navigation.sheetsRead = false
            for wanted in presenter.navigation.sheets {
                if let entry = entry(ofRoute: wanted.route) { if read { project(entry, wanted.header) }; continue }
                guard live(wanted.route), let page = presenter.viewport.window else { continue }
                let front = window ?? page
                let entry = Entry(wanted.route, previous: focusOwner(front), route: true)
                entry.header = wanted.header
                present(entry, over: front)
            }
        } while routeSync.again
    }

    /// The sheet's window was made for `entry`: its rows press the authored buttons.
    func adopt(_ entry: Entry, _ sheet: DialogSheet) {
        sheet.chrome?.onPress = { [weak self, weak entry] id in
            guard let self, let entry, self.entries.contains(where: { $0 === entry }),
                  let node = self.presenter?.views[id], node === entry.header?.cancel || node === entry.header?.confirm else { return }
            self.presenter?.press(id, fromNativeMenu: true)
        }
        presenter?.blocks.onDestroyed = { [weak self] node in self?.entry(ofRoute: node)?.sheet?.chrome?.freeze() }
        project(entry, entry.header, fits: false)
    }

    /// What `entry`'s sheet shows of its route's header now: the rows, the
    /// header's nodes they stand for hidden, and those boxes the platform's.
    func project(_ entry: Entry, _ header: SheetHeader?, fits: Bool = true) {
        guard let presenter, let sheet = entry.sheet, let chrome = sheet.chrome else { return }
        entry.header = header
        let next = header?.stoodFor ?? []
        for node in entry.stoodFor where !next.contains(where: { $0 === node }) && live(node) { node.isHidden = false }
        for node in next where !node.isHidden { node.isHidden = true }
        entry.stoodFor = next
        reportRouteCovers()
        // The sheet's window is named as its title row says, for whoever asks AppKit (VoiceOver, the Window menu).
        let name = header?.title ?? ""
        if sheet.window.title != name { sheet.window.title = name }
        let row = { (node: NodeView?) in node.map { (SheetHeader.words(of: $0), $0.id, !$0.disabled) } }
        let changed = chrome.show(title: header?.title, cancel: row(header?.cancel), confirm: row(header?.confirm),
                                  escapes: entry.dialog.props["closedby"] != "none")
        guard fits, changed else { return }
        // Its rows take another height: the content's maximum and the window's size follow.
        if let block = block(of: entry) { presenter.blocks.resize(entry.dialog, block) }
        sheet.refit()
    }

    /// The sheet has gone: what its rows stood for shows as authored again.
    func release(_ entry: Entry) {
        for node in entry.stoodFor where live(node) { node.isHidden = false }
        entry.stoodFor = []
        entry.header = nil
        reportRouteCovers()
    }

    /// The header's boxes the sheets' rows stand for are the platform's
    /// (D8.1), said in the step the rows show them and taken back when they stop.
    private func reportRouteCovers() {
        var covers: [UInt32: HostCover] = [:]
        for entry in entries where entry.route { for node in entry.stoodFor { covers[node.id] = .whole } }
        presenter?.covers.report("route-sheet", covers)
    }

    /// The containing block a sheet gives its subtree: a maximum on each
    /// axis, the content area of the page's window, and for a route less
    /// its sheet's own rows. Under a maximum the kernel uses an authored
    /// size as written (D8.2), so nothing here reads the route's rows,
    /// which a host is not sent.
    func block(of entry: Entry) -> HostBlock? {
        guard var most = maximum else { return nil }
        if entry.route, let chrome = entry.sheet?.chrome { most.height = max(0, most.height - chrome.rows) }
        return entry.route ? .fit(most) : .atMost(most)
    }

    /// The driver's tap on an authored button a sheet's row stands for:
    /// the row's own click. Nil when `node` is no such button.
    func activate(_ node: NodeView) -> Bool? {
        guard let entry = entries.last(where: { $0.route && ($0.header?.cancel === node || $0.header?.confirm === node) }),
              let chrome = entry.sheet?.chrome else { return nil }
        let button = entry.header?.cancel === node ? chrome.cancel : chrome.confirm
        guard entry === entries.last, presenter?.alerts.presented == nil, !button.isHidden, button.isEnabled, !node.inert else { return false }
        button.performClick(nil)
        return true
    }

    /// Whether `node` is shown by a sheet's own row: the heading its title, a button its push button.
    func rowShows(_ node: NodeView) -> String? {
        guard let header = entries.last(where: { $0.route && $0.header?.stoodFor.isEmpty == false && ($0.header?.heading === node || $0.header?.cancel === node || $0.header?.confirm === node) })?.header else { return nil }
        return header.heading === node ? "title" : "NSButton"
    }

    /// `state.navigation.sheet` (the agent): the top modal route's presentation.
    var routeObservation: [String: Any]? {
        guard let presenter else { return nil }
        if let entry = entries.last(where: \.route) {
            let id = { (node: NodeView?) -> Any in node.map { Int($0.id) } ?? NSNull() }
            let size = entry.sheet?.window.contentView?.frame.size ?? .zero
            return ["route": Int(entry.dialog.id), "presentation": "sheet", "title": entry.header?.title ?? NSNull(),
                    "cancel": id(entry.header?.cancel), "confirm": id(entry.header?.confirm), "header": entry.header.map { $0.whole ? "projected" : "partly" } ?? "authored",
                    "size": [Agent.r2(size.width), Agent.r2(size.height)], "closedby": entry.dialog.props["closedby"] ?? "closerequest"]
        }
        guard let painted = presenter.navigation.paintedModal else { return nil }
        return ["route": Int(painted.id), "presentation": "painted", "kept": presenter.navigation.keptSheets[painted.id] ?? NSNull()]
    }
}
#endif
