// The payloads of a field's events on the wire (`exact_field_event`; `runner/src/runner/field.rs`),
// shared by the UIKit and AppKit fields and the watch's live driver, which links no UIKit field.
import Foundation

package enum FieldWire {
    /// A selection (kinds 40 to 42, and 52): `start,end,direction,` then the value verbatim.
    /// Offsets are UTF-16.
    package static func selection(start: Int, end: Int, direction: String = "none", value: String) -> String {
        "\(start),\(end),\(direction),\(value)"
    }

    /// An `input` or `beforeinput` (kinds 54 and 53): the InputEvent's type, whether it is
    /// cancelable and composing, its data's UTF-8 length (-1 for none) and data, then the
    /// field's value and selection after the edit.
    package static func input(type: String, data: String?, cancelable: Bool = true, composing: Bool = false,
                             value: String, start: Int, end: Int, direction: String = "none") -> String {
        "\(type),\(cancelable ? 1 : 0),\(composing ? 1 : 0),\(data?.utf8.count ?? -1),\(data ?? "")"
            + selection(start: start, end: end, direction: direction, value: value)
    }

    /// The whole value replaced at once, as a system input sheet returns it (the watch's
    /// dictation, Scribble or keyboard): `insertReplacementText`, the caret at the end.
    package static func replacement(_ value: String) -> String {
        let caret = value.utf16.count
        return input(type: "insertReplacementText", data: value, cancelable: false, value: value, start: caret, end: caret)
    }
}
