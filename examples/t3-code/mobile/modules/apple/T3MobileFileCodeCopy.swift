#if os(iOS)
import UIKit

private final class T3FileCodeCopyButton: UIButton {
    override var isHighlighted: Bool { didSet { alpha = isHighlighted ? 0.52 : 1 } }
    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        bounds.insetBy(dx: -8, dy: -8).contains(point)
    }
}

final class T3MobileFileCodeCopy: ExactNativeInstance {
    private weak var owner: T3MobileDocumentMenus?
    private let agent: Bool
    private let button = T3FileCodeCopyButton(type: .custom)
    private var routeKey = "", identity: [String] = [], text = "", label = ""
    private var tint = UIColor.label, successTint = UIColor.link
    private var copied = false, active = false, alive = true
    private var feedback: Timer?
    override var view: UIView { button }

    init(owner: T3MobileDocumentMenus, agent: Bool, events: ExactNativeEvents) {
        self.owner = owner; self.agent = agent
        super.init(events: events)
        button.layer.cornerRadius = 9; button.layer.borderWidth = 1
        button.addAction(UIAction { [weak self] _ in self?.copy() }, for: .touchUpInside)
    }
    override func setProps(_ props: [String: String]) throws {
        guard alive, let key = props["route-key"], !key.isEmpty,
              let owner = props["copy-owner"], !owner.isEmpty,
              let path = props["copy-path"], !path.isEmpty,
              let block = props["copy-block"], let text = props["copy-text"] else {
            throw ExactNativeRefusal("Invalid file code Copy configuration.")
        }
        let nextIdentity = [key, owner, path, block, text]
        if identity != nextIdentity { reset(); identity = nextIdentity }
        routeKey = key; self.text = text
        let language = props["copy-language"] ?? ""
        label = "Copy \(language.isEmpty ? "code" : language.lowercased()) code"
        tint = try T3SymbolView.color(props["copy-tint"] ?? "#27272a")
        successTint = try T3SymbolView.color(props["copy-success-tint"] ?? "#1b4ed8")
        button.backgroundColor = try T3SymbolView.color(props["copy-background"] ?? "#ffffff")
        button.layer.borderColor = try T3SymbolView.color(props["copy-border"] ?? "#e4e4e7").cgColor
        active = props["copy-active"] == "true"
        if !active { reset() }
        render()
    }
    private func copy() {
        guard alive, active, !agent, !text.isEmpty,
              owner?.canCopy(from: button, routeKey: routeKey) == true else { return }
        UIPasteboard.general.string = text
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
        feedback?.invalidate(); copied = true; render()
        feedback = Timer.scheduledTimer(withTimeInterval: 1.2, repeats: false) { [weak self] _ in
            self?.feedback = nil; self?.copied = false; self?.render()
        }
    }
    private func render() {
        button.isEnabled = alive && active && !text.isEmpty && !agent
        button.accessibilityLabel = copied ? "Copied" : label
        button.tintColor = copied ? successTint : tint
        button.setImage(UIImage(systemName: copied ? "checkmark" : "doc.on.doc",
                                withConfiguration: UIImage.SymbolConfiguration(pointSize: 14, weight: .regular)), for: .normal)
    }
    private func reset() { feedback?.invalidate(); feedback = nil; copied = false }
    override func destroy() {
        reset(); alive = false; text = ""; identity = []; owner = nil; render()
    }
}
#endif
