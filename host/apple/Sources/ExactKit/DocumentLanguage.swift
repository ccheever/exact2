// The strings table sets HTML's lang/dir on native presentation too.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension Presenter {
    func applyLanguage(_ batch: Batch) {
        let language = batch.ops.last { $0.op == .language }
        if let language {
            documentLanguage = language.payload["lang"] as? String ?? ""
            documentDirection = language.payload["dir"] as? String ?? "ltr"
        }
        guard language != nil || batch.ops.contains(where: { [.create, .style, .props, .children].contains($0.op) }) else { return }
        let changed: [NodeView]
        if language != nil {
            changed = Array(views.values)
        } else {
            changed = Set(batch.ops.compactMap(\.nodeID)).compactMap { views[$0] }
        }
        #if os(macOS)
        root.userInterfaceLayoutDirection = documentDirection == "rtl" ? .rightToLeft : .leftToRight
        #else
        root.semanticContentAttribute = documentDirection == "rtl" ? .forceRightToLeft : .forceLeftToRight
        root.accessibilityLanguage = documentLanguage.isEmpty ? nil : documentLanguage
        #endif
        for node in changed {
            let direction = node.style["direction"]?.string ?? documentDirection
            #if os(macOS)
            node.userInterfaceLayoutDirection = direction == "rtl" ? .rightToLeft : .leftToRight
            #else
            node.semanticContentAttribute = direction == "rtl" ? .forceRightToLeft : .forceLeftToRight
            #endif
            #if !os(macOS)
            setLanguage(node)
            #endif
        }
    }

    #if !os(macOS)
    private func setLanguage(_ view: UIView) {
        view.accessibilityLanguage = documentLanguage.isEmpty ? nil : documentLanguage
        for child in view.subviews where !(child is NodeView) { setLanguage(child) }
    }
    #endif
}
