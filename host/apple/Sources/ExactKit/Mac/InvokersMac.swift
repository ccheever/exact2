#if os(macOS)
import AppKit

extension MenuHost {
    /// Keep a bubbled handler's invocation separate from this click's button.
    func activating(from touched: NodeView, pressing target: NodeView?, inline: UInt32? = nil, _ press: () -> Void) {
        guard let presenter else { return press() }
        let source = presenter.activationButton(from: touched, inline: inline)
        guard source !== target else { return press() }
        let token = source?.pressMoves, previous = invocationOwner
        invocationOwner = target
        press()
        invocationOwner = previous
        guard let source, source.pressMoves == token, presenter.views[source.id] === source else { return }
        let bubbled = source.inert && !source.authoredInert
        presenter.dialogs.command(source, bubbled: bubbled)?()
        command(source, bubbled: bubbled)?()
    }
}
#endif
