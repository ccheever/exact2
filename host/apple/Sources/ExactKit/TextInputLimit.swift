// HTML maxlength counts UTF-16, applies to user edits, and leaves authored values alone.
import Foundation

enum TextInputLimit {
    static func maximum(_ props: [String: String]) -> Int? {
        let type = (props["type"] ?? "text").lowercased()
        guard props["semanticTag"] == "textarea" || ["text", "search", "email", "url", "tel", "password"].contains(type),
              let value = props["maxlength"].flatMap(Int.init), value >= 0 else { return nil }
        return value
    }
    static func prefix(_ text: String, props: [String: String]) -> String {
        guard let maximum = maximum(props) else { return text }
        var length = 0, result = ""
        for scalar in text.unicodeScalars {
            length += scalar.utf16.count
            if length > maximum { break }
            result.unicodeScalars.append(scalar)
        }
        return result
    }
    static func allows(_ text: String, range: NSRange, replacement: String, props: [String: String]) -> Bool {
        guard let maximum = maximum(props) else { return true }
        let old = text.utf16.count
        let next = old - range.length + replacement.utf16.count
        return next <= maximum || next <= old
    }
}

#if os(macOS)
import AppKit

/// AppKit's field editor consults its formatter before accepting typed/pasted text.
final class TextInputFormatter: Formatter {
    weak var owner: NodeView?
    init(_ owner: NodeView) { self.owner = owner; super.init() }
    required init?(coder: NSCoder) { super.init(coder: coder) }
    override func string(for obj: Any?) -> String? { obj as? String }
    // The formatter otherwise turns attributedStringValue back into plain
    // text, dropping the native field's authored kern (LLP 1104 D3).
    override func attributedString(for obj: Any, withDefaultAttributes attrs: [NSAttributedString.Key: Any]? = nil) -> NSAttributedString? {
        guard let owner, owner.isNativeTextControl, let value = string(for: obj) else { return nil }
        return NSAttributedString(string: value, attributes: (attrs ?? [:]).merging(owner.fieldTextAttributes) { _, authored in authored })
    }
    override func getObjectValue(_ obj: AutoreleasingUnsafeMutablePointer<AnyObject?>?, for string: String, errorDescription error: AutoreleasingUnsafeMutablePointer<NSString?>?) -> Bool {
        obj?.pointee = string as NSString; return true
    }
    override func isPartialStringValid(_ partialString: String, newEditingString newString: AutoreleasingUnsafeMutablePointer<NSString?>?, errorDescription error: AutoreleasingUnsafeMutablePointer<NSString?>?) -> Bool {
        guard let owner, let maximum = TextInputLimit.maximum(owner.props) else { return true }
        return partialString.utf16.count <= maximum || partialString.utf16.count <= (owner.props["value"]?.utf16.count ?? 0)
    }
}
#endif
