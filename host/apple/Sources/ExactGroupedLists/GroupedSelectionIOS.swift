// @ref LLP 1115.000 D11.3 — which rows of a grouped list are selected: a
// tapped row while its press runs and under the screen it pushed.
#if os(iOS) || os(tvOS)
import ExactKit
import UIKit

extension GroupedListView {
    func collectionView(_ view: UICollectionView, shouldSelectItemAt path: IndexPath) -> Bool {
        collectionView(view, shouldHighlightItemAt: path)
    }
    /// A tapped row stays selected while its press runs and, when the press
    /// pushed a screen over the list's, under that screen: it fades as the
    /// reader comes back (`deselect`), as a table view's row does
    /// (`clearsSelectionOnViewWillAppear`; LLP 1115.000 D11.3). A row that
    /// pushed nothing, or opened a sheet, lets go once its press has run.
    func collectionView(_ view: UICollectionView, didSelectItemAt path: IndexPath) {
        let letGo = { for selected in view.indexPathsForSelectedItems ?? [] { view.deselectItem(at: selected, animated: !ExactEnv.agentFreezes) } }
        guard let id = source.itemIdentifier(for: path), pressable(id) else { return letGo() }
        host.presenter.endEditing()
        // The screen the list is on, read before the press can change it.
        let screen = sequence(first: owner as UIResponder, next: { $0.next }).lazy.compactMap { $0 as? UIViewController }.first
        // A radio's row checks its radio, as a click on the radio does:
        // `input` and `change` carry its value, and the checkmark follows
        // the bound `checked` (LLP 1115.000 D13.2).
        if let row = rows[id], let target = radio(row) { host.presenter.checkRadio(target) }
        guard rows[id]?.pressable == true else { return letGo() }
        host.presenter.press(id)
        if let screen, let stack = screen.navigationController, stack.viewControllers.contains(screen), stack.topViewController !== screen { return }
        letGo()
    }

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
