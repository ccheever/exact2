#if os(iOS)
import UIKit
#endif

final class DuoArrangements {
    #if os(iOS)
    private var containers: [ObjectIdentifier: (controller: UIViewController, mode: String)] = [:]
    #endif

    func update(_ element: ExactElement) {
        #if os(iOS)
        guard element.hatch == .arrangedPanes else { return }
        element.reusable = true
        let key = ObjectIdentifier(element)
        let mode = element.data[.paneLayout] ?? "split"
        if #available(iOS 27.1, *) {
            if mode == "css" { element.arrange(using: nil); containers[key]?.mode = mode; return }
            let cached = containers[key]
            let controller = cached?.controller as? UIArrangementViewController ?? UIArrangementViewController()
            if cached?.mode != mode {
                if mode == "overlay" { controller.updateArrangement(.overlay) }
                else { controller.updateArrangement(.split) }
            }
            if element.arrange(using: controller) { containers[key] = (controller, mode) }
        }
        #endif
    }

    func remove(_ element: ExactElement) {
        #if os(iOS)
        containers[ObjectIdentifier(element)] = nil
        #endif
    }
}
