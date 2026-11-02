#if os(iOS) || os(tvOS)
import UIKit

/// Item appearances keep an authored title colour local to its route.
final class NavigationTitleInk {
    private var saved: [UINavigationBarAppearance?]?
    private var projected: [UINavigationBarAppearance?]?
    private var ink: UIColor?
    private var transparent = false

    func update(_ color: UIColor?, item: UINavigationItem, bar: UINavigationBar?, transparent: Bool) {
        let current = [item.standardAppearance, item.compactAppearance, item.scrollEdgeAppearance, item.compactScrollEdgeAppearance]
        if color == ink, transparent == self.transparent, current == projected { return }
        ink = color
        self.transparent = transparent
        if saved == nil, color != nil || transparent { saved = current }
        guard let saved else { return }
        let base = bar?.standardAppearance ?? UINavigationBarAppearance()
        func painted(_ appearance: UINavigationBarAppearance?) -> UINavigationBarAppearance? {
            guard let color else { return appearance }
            let copy = (appearance ?? base).copy() as! UINavigationBarAppearance
            copy.titleTextAttributes[.foregroundColor] = color
            #if !os(tvOS)
            copy.largeTitleTextAttributes[.foregroundColor] = color
            #endif
            return copy
        }
        let next = [
            painted(saved[0]),
            painted(color == nil ? saved[1] : saved[1] ?? bar?.compactAppearance),
            painted(transparent ? NavigationHost.clearScrollEdge : color == nil ? saved[2] : saved[2] ?? bar?.scrollEdgeAppearance),
            painted(transparent ? NavigationHost.clearScrollEdge : color == nil ? saved[3] : saved[3] ?? bar?.compactScrollEdgeAppearance)
        ]
        item.standardAppearance = next[0]
        item.compactAppearance = next[1]
        item.scrollEdgeAppearance = next[2]
        item.compactScrollEdgeAppearance = next[3]
        projected = next
        if color == nil, !transparent { self.saved = nil }
    }
}
#endif
