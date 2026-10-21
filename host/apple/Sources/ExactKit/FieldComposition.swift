// @ref LLP 1115.000 §5 (ruling 6, 2026-10-12) and LLP 1045 D5: what an app
// hears of a field or a text area while an input method composes in it
// (Japanese kana before its kanji, pinyin, a Korean syllable), and what the
// app's value does to a composition. The same on macOS and iOS.
//
// **What the app hears.** One `input` for each change of the editor's text,
// with the text the input method has marked in the value, as it happens:
// "に", "にほ", "にほん", then "日本" when the candidate replaces it. What
// changes no text is not heard: the caret or the clause moving inside what
// is marked, the commit of what is marked as it stands, and the steps a
// platform takes to do either (UIKit's text view says its text changed as a
// composition starts, before it has). A composition that is cancelled is
// heard, since its text goes.
// `change` is still the commit of editing (focus leaving or Return in a
// single line). InputEvent.isComposing identifies marked-text edits; their
// beforeinput is noncancelable. Composition start/end events remain absent.
//
// **What the app's value does.** It never ends a composition. A commit that
// sets the value the editor already shows changes nothing in it: the
// composition and the selection stand. One that sets a different value while
// text is marked is held (`pendingValue`, the last one written; one equal to
// the editor's text takes its place with nothing), and when the composition
// ends, committed or cancelled, the app is first told the text if it
// changed and then what is still held is written, if it is not the text.
// An action that rewrites what it hears (upper-cases it, strips spaces, formats
// a number) would otherwise end one at its first keystroke, and nobody
// could type Japanese or Chinese in that field. Its rewrite shows a moment
// later instead. Nothing marked, a value is written at once.
import Foundation

extension NodeView {
    /// The editor's text is now `value`, marked text and all. The app is
    /// told when that is not the text it knows: what it was last told
    /// (`toldText`), or the value it last set.
    func typedText(_ value: String) {
        if value == toldText ?? props["value"] ?? "" { return }
        toldText = value
        presenter?.typed(id, value, input: handlers.contains("input"))
    }
}
