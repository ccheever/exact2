// A confirmation in the alert shape on AppKit (LLP 1115.000 D7.6; LLP 1115
// D6's Mac half): an `alertdialog` — a `dialog` shown by `showModal` or its
// `command="show-modal"`, or a popover opened by its invoker — whose rows are
// text, one hide-only cancel and at most three buttons with it
// (`ConfirmationShape`, the rule iOS's alert follows) is `NSAlert`, a sheet
// on its window. Its title is the `aria-label`, else its first line, and its
// message the rest. Each authored button is an alert button: a destructive
// one says so (`hasDestructiveAction`), the cancel answers Escape, and the
// default (`defaultRow`: exactly one `filled`, non-destructive button)
// answers Return and is drawn so by AppKit; with none or several nothing is
// the default, as AppKit leaves a destructive alert.
// A chosen button presses its authored button by view id once, on the turn
// after the sheet has ended, and only while the alert still showed what is
// there (as the chooser's menu does, ChooserMac.swift). Any other shape is
// no alert: a chooser is the `NSMenu`, and the rest is a sheet or a popover
// holding its own subtree (LLP 1115.002 D7; SheetsMac.swift,
// PopoversMac.swift). An alert has no outside (LLP 1115.000 D13.4): a click
// on the window behind the sheet does nothing and Escape presses the cancel,
// which is the `closedby` lowering implies for the shape, `closerequest`. So
// a `dialog` whose author wrote `closedby="any"` is not presented as one: it
// is the dialog's own sheet, which a click on the window behind it dismisses;
// and under `closedby="none"` the cancel is given no Escape. Asked for while
// a dialog is open, the alert is a sheet on that dialog's sheet (on the
// page's window AppKit would hold it back until the first had gone), and
// its rows are its own, not inert under the dialog beneath. The agent sees
// the same alert: a tap on an authored button presses the alert's button,
// and Return and Escape go to the sheet (AgentMac.swift).
#if os(macOS)
import AppKit

final class AlertHost {
    private weak var presenter: Presenter?
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// One of the alert's buttons as presented: the authored button, the
    /// title and enablement it showed, and whether it is the cancel.
    final class Row {
        weak var node: NodeView?
        let title: String, enabled: Bool, cancel: Bool
        init(_ node: NodeView, title: String, enabled: Bool, cancel: Bool) { self.node = node; self.title = title; self.enabled = enabled; self.cancel = cancel }
    }
    /// An alert while its sheet is up, kept until the sheet has ended.
    final class Presented {
        let alert = NSAlert()
        weak var node: NodeView?
        weak var source: NodeView?
        /// Opened by an invoker, not by `showModal` or a dialog's command.
        let sourced: Bool
        weak var window: NSWindow?
        let rows: [Row]
        /// Which row Return presses, when one does.
        let defaultRow: Int?
        /// Ended by this host (a batch changed what it shows, `close(id)`, a
        /// reset), not by a button: nothing is pressed.
        var withdrawn = false
        init(node: NodeView, source: NodeView?, window: NSWindow, rows: [Row], defaultRow: Int?) {
            self.node = node; self.source = source; sourced = source != nil; self.window = window
            self.rows = rows; self.defaultRow = defaultRow
        }
    }
    private(set) var presented: Presented?
    /// The alertdialog whose chosen button is being pressed, after its sheet has ended.
    private weak var answering: NodeView?
    /// The alertdialog that is the top modal: the one presented, or being
    /// answered. Over an open dialog its rows are its own, not the dialog's
    /// to block (`DialogHost.blocks`).
    var modal: NodeView? { presented?.node ?? answering }
    /// The sheet's window while an alert is up.
    var sheet: NSWindow? { presented?.alert.window }
    func isOpen(_ node: NodeView) -> Bool { presented?.node === node }

    private func closes(_ row: NodeView, _ pop: NodeView) -> Bool {
        if pop.props["popover"] != nil { return presenter?.menus.closes(row, pop) == true }
        return row.props["commandfor"] == pop.props["id"] && row.props["command"] == "close"
    }
    /// Live, enabled, not inert, and not hidden by the page, though the
    /// alertdialog itself is hidden in place while the alert presents it.
    /// Inert by its own tree (an authored `inert`, a route's), not by the
    /// dialog that is open: the alert goes over that one, as `showModal()`
    /// puts a dialog over the modal before it (an alert over a sheet).
    private func choosable(_ row: NodeView, in pop: NodeView) -> Bool {
        guard let menus = presenter?.menus else { return false }
        let inert = sequence(first: row as NSView, next: { menus.parent(of: $0) }).contains { ($0 as? NodeView).map { $0.routeInert || $0.props["inert"] == "true" } ?? false }
        return menus.live(row) && !row.disabled && !inert && menus.shown(row, in: pop)
    }

    /// What the alert for an `alertdialog` shows: its words, its buttons in
    /// order (the actions, then the cancel), which are destructive, and
    /// which one Return presses.
    struct Plan {
        let title: String, message: String
        let rows: [Row]
        let destructive: [Bool]
        let defaultRow: Int?
    }
    /// The alert for `pop`, or nil for any shape but the alert's, which is
    /// presented as before.
    func plan(of pop: NodeView) -> Plan? {
        guard let menus = presenter?.menus, pop.props["accessibilityRole"] == "alertdialog",
              pop.props["popover"] != nil || pop.props["semanticTag"] == "dialog" else { return nil }
        if pop.props["semanticTag"] == "dialog", pop.props["closedby"] == "any" { return nil }
        let children = pop.container.subviews.compactMap { $0 as? NodeView }
        let actions = children.filter { $0.isButton && $0.handlers.contains("press") }
        let cancels = children.filter { $0.isButton && !$0.handlers.contains("press") && closes($0, pop) }
        let texts = children.filter { $0.kind == "text" }
        guard children.count == actions.count + cancels.count + texts.count, let cancel = cancels.first,
              actions.allSatisfy({ closes($0, pop) }), actions.contains(where: { choosable($0, in: pop) }) else { return nil }
        let lines = texts.map(menus.title(of:)).filter { !$0.isEmpty }
        guard ConfirmationShape.isAlert(texts: lines.count, actions: actions.count, cancels: cancels.count) else { return nil }
        let nodes = actions + [cancel]
        let rows = nodes.map { Row($0, title: menus.title(of: $0), enabled: $0 === cancel ? !$0.disabled : choosable($0, in: pop), cancel: $0 === cancel) }
        let destructive = nodes.map { $0.props["destructive"] == "true" }
        // Where the one filled button is the cancel the alert has no Return
        // default: AppKit gives a cancel Escape and never Return (its own
        // "Cancel", added first, has Escape alone), and a button has one key.
        let emphasized = Self.defaultRow(filled: nodes.map { $0.face?.style == "filled" }, destructive: destructive)
        let words = ConfirmationShape.alertText(label: pop.props["accessibilityLabel"].flatMap { $0.isEmpty ? nil : $0 }, lines: lines)
        return Plan(title: words.title ?? "", message: words.message ?? "", rows: rows, destructive: destructive,
                    defaultRow: emphasized.flatMap { rows[$0].cancel ? nil : $0 })
    }

    /// The row Return presses (LLP 1115.000 D7.3): the one `filled`,
    /// non-destructive button, when there is exactly one. Emphasis alone
    /// never picks among several, and a destructive button is never it.
    static func defaultRow(filled: [Bool], destructive: [Bool]) -> Int? {
        let candidates = filled.indices.filter { filled[$0] && !destructive[$0] }
        return candidates.count == 1 ? candidates[0] : nil
    }

    /// Present `pop` as the platform's alert when it has the alert's shape
    /// and its window can take a sheet; false leaves it to its other
    /// presentations.
    @discardableResult
    func present(_ pop: NodeView, from source: NodeView?) -> Bool {
        // On the sheet of the dialog that is open, when one is: AppKit keeps
        // a second sheet on the page's window waiting behind the first.
        guard let presenter, presented == nil, presenter.menus.live(pop), let window = presenter.frontWindow,
              window.attachedSheet == nil, let plan = plan(of: pop) else { return false }
        let menus = presenter.menus, rows = plan.rows
        let owner = Presented(node: pop, source: source, window: window, rows: rows, defaultRow: plan.defaultRow)
        owner.alert.messageText = plan.title
        owner.alert.informativeText = plan.message
        for (index, row) in rows.enumerated() {
            let button = owner.alert.addButton(withTitle: row.title)
            button.isEnabled = row.enabled
            button.hasDestructiveAction = plan.destructive[index]
            // AppKit makes the first button the default and gives Escape to
            // one titled "Cancel": here the author's emphasis says which is
            // the default, and the authored cancel is the cancel, whatever
            // its title. (The Return button is the window's default button
            // cell; AppKit holds its key equivalent while the sheet is key.)
            button.keyEquivalent = row.cancel ? (pop.props["closedby"] == "none" ? "" : "\u{1b}") : index == owner.defaultRow ? "\r" : ""
            button.setAccessibilityIdentifier(row.node?.props["testId"])
        }
        // Other popovers end as they do under a dialog.
        if source == nil { menus.reset() }
        presented = owner
        owner.alert.beginSheetModal(for: window) { [weak self, owner] response in
            // Off the window now, before the press's batch (Apple's own
            // advice for work in this handler).
            owner.alert.window.orderOut(nil)
            self?.ended(owner, response)
        }
        presenter.shortcuts.sync()
        return true
    }

    /// The sheet ended. A button of it presses its authored button on the
    /// next turn, once, if the alert still showed what is there; the cancel
    /// has nothing to press.
    private func ended(_ owner: Presented, _ response: NSApplication.ModalResponse) {
        if presented === owner { presented = nil }
        presenter?.shortcuts.sync()
        let index = response.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
        guard !owner.withdrawn else { return }
        defer { owner.node?.queueDialogClose() }
        guard owner.rows.indices.contains(index), valid(owner), !owner.rows[index].cancel else { return }
        let row = owner.rows[index]
        DispatchQueue.main.async { [weak self, owner] in
            guard let self, let presenter = self.presenter, self.valid(owner), row.enabled, let node = row.node, let pop = owner.node,
                  self.choosable(node, in: pop) else { return }
            self.answering = pop
            defer { self.answering = nil }
            presenter.press(node.id, fromNativeMenu: true)
        }
    }

    /// The alert still shows what is there: its alertdialog live, in this
    /// window and still of the alert's shape, its invoker (when it has one)
    /// still opening it, and each button live, in it, closing it, its title
    /// and enablement as presented.
    func valid(_ owner: Presented) -> Bool {
        guard let presenter, let pop = owner.node, presenter.menus.live(pop), presenter.hosts(owner.window),
              plan(of: pop) != nil else { return false }
        if owner.sourced {
            guard let source = owner.source, presenter.menus.invokes(source, pop) else { return false }
        }
        return owner.rows.allSatisfy { row in
            guard let node = row.node, presenter.menus.live(node), node.isDescendant(of: pop), closes(node, pop),
                  presenter.menus.title(of: node) == row.title else { return false }
            return (row.cancel ? !node.disabled : choosable(node, in: pop)) == row.enabled
        }
    }

    /// End the sheet with nothing pressed.
    private func withdraw(_ owner: Presented) {
        owner.withdrawn = true
        if presented === owner { presented = nil }
        if let window = owner.window, window.attachedSheet === owner.alert.window { window.endSheet(owner.alert.window, returnCode: .abort) }
        owner.alert.window.orderOut(nil)
    }
    /// `close(id)` from an action, or the dialog's own `command="close"`
    /// from elsewhere: the alert ends as its cancel ends it. True when
    /// `node` was the alert.
    @discardableResult
    func close(_ node: NodeView) -> Bool {
        guard let owner = presented, owner.node === node else { return false }
        withdraw(owner)
        node.queueDialogClose()
        return true
    }
    /// After a batch: an alert that no longer shows what is there ends, and
    /// nothing it showed is pressed.
    func sync() {
        if let owner = presented, !valid(owner) { withdraw(owner) }
    }
    func reset() {
        if let owner = presented { withdraw(owner) }
    }

    /// The alert's button for an authored one, while it is up.
    private func button(for node: NodeView) -> NSButton? {
        guard let owner = presented, let index = owner.rows.firstIndex(where: { $0.node === node }),
              owner.alert.buttons.indices.contains(index) else { return nil }
        return owner.alert.buttons[index]
    }
    /// The driver's press of an authored button the alert shows (host
    /// activation: AppKit's own click of the alert's button). nil when
    /// `node` is no button of an alert that is up.
    func click(_ node: NodeView) -> Bool? {
        guard let owner = presented, let button = button(for: node) else { return nil }
        guard valid(owner), button.isEnabled else { return false }
        button.performClick(nil)
        return true
    }
    /// A key at the sheet: the button AppKit binds to it (Return the
    /// default, Escape the cancel) is clicked. The sheet of a window that is
    /// not key, a script's, takes no Return from an event, so the driver's
    /// keys come here. False when the key is no button's.
    func key(_ name: String) -> Bool {
        guard let owner = presented, valid(owner) else { return false }
        let escapes = owner.node?.props["closedby"] != "none"
        let index = name == "Enter" ? owner.defaultRow : name == "Escape" && escapes ? owner.rows.firstIndex(where: \.cancel) : nil
        guard let index, owner.alert.buttons.indices.contains(index), owner.alert.buttons[index].isEnabled else { return false }
        owner.alert.buttons[index].performClick(nil)
        return true
    }

    /// `state.dialog` (the agent).
    var observation: [String: Any]? {
        presented.flatMap { owner in
            guard let node = owner.node else { return nil }
            return ["kind": "alert", "presentation": "NSAlert", "dialog": Int(node.id), "phase": "open",
                    "title": owner.alert.messageText, "message": owner.alert.informativeText,
                    "buttons": owner.rows.map(\.title),
                    "default": owner.defaultRow.map { owner.rows[$0].title as Any } ?? NSNull(),
                    "sheet": owner.window?.attachedSheet === owner.alert.window]
        }
    }
}
#endif
