// @ref LLP 1115.000 D11.3, LLP 1127 — which rows of a grouped list are
// selected: a tapped row while its press runs and under the screen it
// pushed, and in a split's sidebar the row whose detail shows.
#if os(iOS) || os(tvOS)
import ExactKit
import UIKit

extension GroupedListHost {
    func highlight(carried node: NodeView, _ down: Bool) -> Bool {
        list(carrying: node)?.highlight(node.id, down) ?? false
    }
    func open(from row: NodeView, press: () -> Void) {
        guard let list = lists.values.first(where: { $0.rows[row.id] != nil }), list.sidebar else { return press() }
        list.pressedSidebarRow = row.id
        press()
        // Selected here: a detail that took the shown one's place is the
        // same screen, so no update of the split selects the row
        #if os(iOS)
        list.selectDetailRow()
        #endif
    }
}

extension GroupedListView {
    /// A pressed custom row is its cell pressed, as UIKit draws a row it
    /// owns: in a plain list the cell's pressed ground, with the list's
    /// lines beside it hidden; in a sidebar the row faded (`stateFollower`).
    func highlight(_ id: UInt32, _ down: Bool) -> Bool {
        // Only a pressable row takes a highlight; any row lets one go
        guard !down || pressable(id), let path = source.indexPath(for: id), let cell = collection.cellForItem(at: path) else { return false }
        if cell.isHighlighted != down { cell.isHighlighted = down }
        return true
    }
    /// Draws a custom row as UIKit draws its own cell content in the cell's
    /// state. While the cell's ground is dark (the accent under a sidebar's
    /// selected row) the row is in the dark appearance, and its label and
    /// secondary label take the colours UIKit's content takes there
    /// (`SelectedItemInk`). While it is pressed it fades as far as UIKit
    /// fades its own label: a sidebar's held row keeps a clear ground and
    /// fades its content (to 30% on iOS 27), where a plain list's fills its
    /// ground and fades nothing. A row with a context menu keeps its content
    /// as it is: the owner chose the peek's lift alone to show its press.
    var stateFollower: UICollectionViewListCell.ConfigurationUpdateHandler? {
        #if os(iOS)
        return { cell, state in
            let fill = cell.defaultBackgroundConfiguration().updated(for: state).resolvedBackgroundColor(for: cell.tintColor)
            var r: CGFloat = 1, g: CGFloat = 1, b: CGFloat = 1, a: CGFloat = 0
            let dark = fill.getRed(&r, green: &g, blue: &b, alpha: &a) && a > 0.5 && 0.2126 * r + 0.7152 * g + 0.0722 * b < 0.5
            let wanted: UIUserInterfaceStyle = dark ? .dark : .unspecified
            if cell.contentView.overrideUserInterfaceStyle != wanted { cell.contentView.overrideUserInterfaceStyle = wanted }
            let content = (cell as? UICollectionViewListCell)?.defaultContentConfiguration().updated(for: state)
            let ink = dark ? content.map { SelectedItemInk.Ink(text: $0.textProperties.resolvedColor(), secondary: $0.secondaryTextProperties.resolvedColor()) } : nil
            let inked = cell.contentView.traitOverrides.contains(SelectedItemInk.self)
            if let ink, !inked || cell.contentView.traitOverrides[SelectedItemInk.self] != ink { cell.contentView.traitOverrides[SelectedItemInk.self] = ink }
            if ink == nil, inked { cell.contentView.traitOverrides.remove(SelectedItemInk.self) }
            let label = { (state: UICellConfigurationState) in
                (cell as? UICollectionViewListCell)?.defaultContentConfiguration().updated(for: state)
                    .textProperties.resolvedColor().resolvedColor(with: cell.traitCollection).cgColor.alpha ?? 1
            }
            let peeks = cell.contentView.subviews.contains { $0.interactions.contains { $0 is UIContextMenuInteraction } }
            let pressed = state.isHighlighted && !state.isSelected && !peeks
            let alpha = pressed ? min(1, label(state) / max(label(UICellConfigurationState(traitCollection: cell.traitCollection)), 0.01)) : 1
            if cell.contentView.alpha != alpha { cell.contentView.alpha = alpha }
        }
        #else
        return nil
        #endif
    }
    /// Takes off what `stateFollower` set for the row a cell drew before.
    func forgetState(of cell: UICollectionViewCell) {
        cell.contentView.overrideUserInterfaceStyle = .unspecified
        #if os(iOS)
        if cell.contentView.traitOverrides.contains(SelectedItemInk.self) { cell.contentView.traitOverrides.remove(SelectedItemInk.self) }
        #endif
        cell.contentView.alpha = 1
    }
    func collectionView(_ view: UICollectionView, shouldSelectItemAt path: IndexPath) -> Bool {
        collectionView(view, shouldHighlightItemAt: path)
    }
    /// A tapped row stays selected while its press runs and, when the press
    /// pushed a screen over the list's, under that screen: it fades as the
    /// reader comes back (`deselect`), as a table view's row does
    /// (`clearsSelectionOnViewWillAppear`; LLP 1115.000 D11.3). A row that
    /// pushed nothing, or opened a sheet, lets go once its press has run.
    func collectionView(_ view: UICollectionView, didSelectItemAt path: IndexPath) {
        chose(path, endingEdits: true) { host.presenter.press($0) }
    }
    /// The row at `path` was chosen. A carried row passes a press of its own
    /// view, which keeps its focus and popover handling (`select(_:press:)`);
    /// a row UIKit draws ends editing first.
    func chose(_ path: IndexPath, endingEdits: Bool, press: (UInt32) -> Void) {
        let view = collection
        let letGo = { for selected in view.indexPathsForSelectedItems ?? [] { view.deselectItem(at: selected, animated: !ExactEnv.agentFreezes) } }
        guard let id = source.itemIdentifier(for: path), pressable(id) else { return letGo() }
        if endingEdits { host.presenter.endEditing() }
        // The screen the list is on, read before the press can change it.
        let screen = sequence(first: owner as UIResponder, next: { $0.next }).lazy.compactMap { $0 as? UIViewController }.first
        // A radio's row checks its radio, as a click on the radio does:
        // `input` and `change` carry its value, and the checkmark follows
        // the bound `checked` (LLP 1115.000 D13.2).
        if let row = rows[id], let target = radio(row) { host.presenter.checkRadio(target) }
        guard rows[id]?.pressable == true else { return letGo() }
        // Before the press, which can push the detail at once
        if sidebar { pressedSidebarRow = id }
        press(id)
        if let screen, let stack = screen.navigationController, stack.viewControllers.contains(screen), stack.topViewController !== screen { return }
        #if os(iOS)
        // UIKit has selected it: the list's next update checks it against the detail
        if sidebar { return }
        #endif
        letGo()
    }

    #if os(iOS)
    /// A sidebar's pressed row stays selected while its detail shows, as
    /// UIKit's sidebar keeps it (LLP 1127); none other is.
    func selectDetailRow() {
        guard sidebar else { return }
        let shown = host.presenter.sidebarSelection(owner)
        let wanted = shown == pressedSidebarRow ? shown.flatMap { source.indexPath(for: $0) } : nil
        let current = collection.indexPathsForSelectedItems ?? []
        for path in current where path != wanted { collection.deselectItem(at: path, animated: false) }
        if let wanted, !current.contains(wanted) { collection.selectItem(at: wanted, animated: false, scrollPosition: []) }
    }
    #endif

    /// The list's screen is coming back into view: its selected rows fade
    /// with the transition, so the fade follows a back swipe's finger, and a
    /// swipe let go of early leaves the row selected under the screen that
    /// stayed. With no transition, at once.
    func deselect(alongside coordinator: UIViewControllerTransitionCoordinator?) {
        let paths = collection.indexPathsForSelectedItems ?? []
        guard !paths.isEmpty else { return }
        guard let coordinator, !ExactEnv.agentFreezes else {
            for path in paths { collection.deselectItem(at: path, animated: false) }
            return
        }
        let list = collection
        coordinator.animate(alongsideTransition: { _ in
            for path in paths { list.deselectItem(at: path, animated: true) }
        }, completion: { context in
            guard context.isCancelled else { return }
            for path in paths { list.selectItem(at: path, animated: false, scrollPosition: []) }
        })
    }
}
#endif
