#if os(iOS) || os(tvOS)
import UIKit

/// UITextView's selection recognizers consume taps before touchesEnded.
/// Admit only decoration controls; the editor keeps its own selection and pan.
final class DecorationTap: NSObject, UIGestureRecognizerDelegate {
    private weak var field: TextArea?
    private let recognizer = UITapGestureRecognizer()

    init(_ field: TextArea) {
        self.field = field
        super.init()
        recognizer.addTarget(self, action: #selector(tapped))
        recognizer.delegate = self
        recognizer.cancelsTouchesInView = false
        field.addGestureRecognizer(recognizer)
    }
    func detach() { field?.removeGestureRecognizer(recognizer) }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        guard let field, field.markedTextRange == nil else { return false }
        return field.decorations?.hitTest(touch.location(in: field)) == true
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool { true }

    @objc private func tapped(_ recognizer: UITapGestureRecognizer) {
        guard recognizer.state == .ended, let field, field.markedTextRange == nil else { return }
        _ = field.decorations?.click(recognizer.location(in: field))
    }
}
#endif
