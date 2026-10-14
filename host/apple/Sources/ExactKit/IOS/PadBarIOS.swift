// The number pads' bar on iOS (LLP 1116.003 D7): the decimal, number and
// phone pads have no Return key (and the decimal and number pads no minus
// key), so a field on one gets the keys it needs in the keyboard's accessory
// bar, as a hand-built form adds them: Done always (LLP 1116.008 D8: a pad
// with no way to put it away left expense-tracker's builder tapping a
// category to dismiss it, where Return would have acted only for a field with
// `enterkeyhint="done"` or a `submit` handler), and ± where its `min` is
// below zero (round 5's unit converter wrote `inputmode="text"` to reach a
// minus and got a letter keyboard). ± flips the sign of what the field holds
// and fires its `input` as a keystroke does; Done does what Return does
// (commit, submit) and puts the keyboard away.
#if os(iOS)
import UIKit

extension NodeView {
    /// The keys this field's pad lacks: Done on every pad with no Return key,
    /// ± on the decimal and number pads where `min` is below zero.
    var padKeys: (sign: Bool, done: Bool) {
        guard let f = field else { return (false, false) }
        switch f.keyboardType {
        case .decimalPad, .numberPad:
            let sign = props["min"].flatMap { Double($0.trimmingCharacters(in: .whitespaces)) }.map { $0 < 0 } ?? false
            return (sign, true)
        case .phonePad: return (false, true)
        default: return (false, false)
        }
    }

    /// The bar follows the field's props and handlers: made for the keys it
    /// asks for, taken down (only its own) when it asks for none.
    func applyPadBar() {
        guard let f = field else { return }
        let keys = padKeys, own = f.inputAccessoryView as? PadBar
        if keys.sign || keys.done {
            guard own?.owner !== self || own?.sign != keys.sign || own?.done != keys.done else { return }
            f.inputAccessoryView = PadBar(owner: self, sign: keys.sign, done: keys.done)
        } else if own != nil {
            f.inputAccessoryView = nil
        } else { return }
        if f.isFirstResponder { f.reloadInputViews() }
    }

    /// ±: the sign of the field's text flips (an empty field starts a negative
    /// number) and the field reports the change as UIKit's own edit does
    /// (`editingChanged`, FieldEditingIOS).
    @objc func toggleSign() {
        guard let f = field, !disabled, f.isEnabled else { return }
        let text = f.text ?? ""
        f.text = text.hasPrefix("-") || text.hasPrefix("\u{2212}") ? String(text.dropFirst()) : "-" + text
        fieldChanged()
    }

    /// Done: what Return does (the key its handlers hear, the commit, an
    /// implicit submission, then the keyboard goes away unless the action
    /// focused the field again, LLP 1115.000 D9.2). A Return a `key`
    /// handler prevented does none of that, and Done still says done.
    @objc func padDone() {
        guard let f = field else { return }
        _ = textFieldShouldReturn(f)
        if f.isFirstResponder, presenter?.focusAsked !== self { f.resignFirstResponder() }
    }
}

/// The bar over the pad: ± at its leading edge where the field asks for it, Done at its trailing one.
final class PadBar: UIToolbar {
    weak var owner: NodeView?
    let sign: Bool, done: Bool
    init(owner: NodeView, sign: Bool, done: Bool) {
        self.owner = owner
        self.sign = sign
        self.done = done
        super.init(frame: CGRect(x: 0, y: 0, width: 320, height: 44))
        var items: [UIBarButtonItem] = []
        if sign {
            let key = UIBarButtonItem(title: "\u{00B1}", style: .plain, target: owner, action: #selector(NodeView.toggleSign))
            key.accessibilityIdentifier = "sign-key"
            items.append(key)
        }
        items.append(UIBarButtonItem(barButtonSystemItem: .flexibleSpace, target: nil, action: nil))
        if done { items.append(UIBarButtonItem(barButtonSystemItem: .done, target: owner, action: #selector(NodeView.padDone))) }
        self.items = items
        sizeToFit()
    }
    required init?(coder: NSCoder) { nil }
}
#else
extension NodeView { func applyPadBar() {} }
#endif
