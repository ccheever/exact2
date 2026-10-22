#if os(iOS) || os(tvOS)
import UIKit

/// Item appearances keep an authored title colour local to its route.
final class NavigationTitleInk {
    private var saved: [UINavigationBarAppearance?]?
    private var ink: UIColor?

    func update(_ color: UIColor?, item: UINavigationItem, bar: UINavigationBar?) {
        guard color != ink else { return }
        ink = color
        guard let color else {
            guard let saved else { return }
            item.standardAppearance = saved[0]
            item.compactAppearance = saved[1]
            item.scrollEdgeAppearance = saved[2]
            item.compactScrollEdgeAppearance = saved[3]
            self.saved = nil
            return
        }
        if saved == nil { saved = [item.standardAppearance, item.compactAppearance, item.scrollEdgeAppearance, item.compactScrollEdgeAppearance] }
        let base = bar?.standardAppearance ?? UINavigationBarAppearance()
        func painted(_ appearance: UINavigationBarAppearance?) -> UINavigationBarAppearance {
            let copy = (appearance ?? base).copy() as! UINavigationBarAppearance
            copy.titleTextAttributes[.foregroundColor] = color
            #if !os(tvOS)
            copy.largeTitleTextAttributes[.foregroundColor] = color
            #endif
            return copy
        }
        item.standardAppearance = painted(saved?[0])
        item.compactAppearance = painted(saved?[1] ?? bar?.compactAppearance)
        item.scrollEdgeAppearance = painted(saved?[2] ?? bar?.scrollEdgeAppearance)
        item.compactScrollEdgeAppearance = painted(saved?[3] ?? bar?.compactScrollEdgeAppearance)
    }
}
#endif
