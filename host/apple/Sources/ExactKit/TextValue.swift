// A controlled `value` written into a native editor, the way a person would
// have typed it: only the changed middle is replaced, so the selection and
// undo survive, and nothing is written while text is being composed.
//
// The app's value is authoritative; the editor's buffer is what the person
// sees. When they differ the host replaces the shortest middle that makes
// them equal and carries the selection through that replacement. A write
// that arrives during IME composition (marked text) is held and applied when
// the composition ends, so a marked range is never torn.
//
// @ref LLP 1045 D5 — the write-back fix that leads slice 2.
import Foundation

/// The shortest replacement that turns `old` into `new`, in UTF-16 units.
/// Returns nil when they are equal.
package func minimalTextEdit(from old: String, to new: String) -> (range: NSRange, text: String)? {
    if old.utf16.elementsEqual(new.utf16) { return nil }
    let a = Array(old.utf16), b = Array(new.utf16)
    var prefix = 0
    while prefix < a.count, prefix < b.count, a[prefix] == b[prefix] { prefix += 1 }
    // Never split a surrogate pair by ending the prefix between its halves.
    if prefix > 0, prefix < a.count, UTF16.isTrailSurrogate(a[prefix]) { prefix -= 1 }
    var suffix = 0
    while suffix < a.count - prefix, suffix < b.count - prefix, a[a.count - 1 - suffix] == b[b.count - 1 - suffix] { suffix += 1 }
    if suffix > 0, suffix < a.count - prefix, UTF16.isLeadSurrogate(a[a.count - 1 - suffix]) { suffix -= 1 }
    let range = NSRange(location: prefix, length: a.count - prefix - suffix)
    let text = String(utf16CodeUnits: Array(b[prefix..<(b.count - suffix)]), count: b.count - prefix - suffix)
    return (range, text)
}

/// Where a selection lands after `edit` is applied: positions after the
/// replaced range shift by its size change; positions inside it collapse to
/// the end of the new text.
func carrySelection(_ selection: NSRange, through edit: (range: NSRange, text: String)) -> NSRange {
    let delta = (edit.text.utf16.count) - edit.range.length
    let end = edit.range.location + edit.range.length
    func carry(_ p: Int) -> Int {
        if p <= edit.range.location { return p }
        if p >= end { return p + delta }
        return edit.range.location + edit.text.utf16.count
    }
    let start = carry(selection.location), stop = carry(selection.location + selection.length)
    return NSRange(location: start, length: max(0, stop - start))
}
