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
            node.scroll?.tile()
            node.fitScroll()
            #else
            node.semanticContentAttribute = direction == "rtl" ? .forceRightToLeft : .forceLeftToRight
            #endif
            #if !os(macOS)
            for child in node.subviews where !(child is NodeView) { setLanguage(child) }
            #endif
        }
    }

    #if !os(macOS)
    /// A platform view a node holds (a field, a control) is told the
    /// language; a node reads it (`NodeView.accessibilityLanguage`).
    private func setLanguage(_ view: UIView) {
        view.accessibilityLanguage = documentLanguage.isEmpty ? nil : documentLanguage
        for child in view.subviews where !(child is NodeView) { setLanguage(child) }
    }
    #endif
}

#if !os(macOS)
extension NodeView {
    /// The document's language (`lang`), which VoiceOver speaks the node in:
    /// read from the presenter. UIKit stores a set one per view (an
    /// associated object: 20 KB of malloc over euxy's 250 nodes), and an
    /// unset one is the user's language, not an ancestor's (`UIAccessibility.h`).
    package override var accessibilityLanguage: String? {
        get {
            if let own = super.accessibilityLanguage { return own }
            guard let language = presenter?.documentLanguage, !language.isEmpty else { return nil }
            return language
        }
        set { super.accessibilityLanguage = newValue }
    }
}
#endif
