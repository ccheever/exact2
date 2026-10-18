// The optional editor owns paint; the text area's source remains a plain string.
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import CExact

enum DecorationMeasurements {
    static let read: ExactDecoratedMeasureFn = { ctx, id, request, record, recordCount, styles, styleCount, root, scale in
        guard let ctx, let request else { return ExactMetrics(width:-1,height:-1,baseline:-1) }
        let engine = Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue()
        let record = String(decoding:UnsafeBufferPointer(start:record,count:recordCount),as:UTF8.self)
        let styles = String(decoding:UnsafeBufferPointer(start:styles,count:styleCount),as:UTF8.self)
        return Owner.shared.callMain {
            let source = UnsafeBufferPointer(start:request.pointee.runs,count:request.pointee.count).map {
                String(decoding:UnsafeBufferPointer(start:$0.text,count:$0.len),as:UTF8.self)
            }.joined()
            let session = ExactApp.shared.sessions.first { $0.text.measuring === engine }
            let node = session?.presenter.views[id]
            let selection = node.flatMap { node in
                node.fieldText == source ? session?.presenter.fieldSelections.current(node).range : nil
            }
            return MarkdownLink.installed?.measureDecorations(engine,request:request.pointee,record:record,styles:styles,root:Double(root),scale:Double(scale),selection:selection) ?? ExactMetrics(width:-1,height:-1,baseline:-1)
        }
    }
}

package protocol FieldDecorating: AnyObject {
    var styling: Bool { get }
    var state: [String: Any] { get }
    var projection: FieldTextProjection? { get }
    func restyle(_ node: NodeView)
    func detach(_ node: NodeView)
    func selection(_ range: NSRange, previous: NSRange) -> NSRange
    func replacement(_ range: NSRange) -> NSRange
    func draw(background: Bool)
    func click(_ point: CGPoint) -> Bool
}

extension NodeView {
    package func remeasureDecorations() {
        guard let session = presenter?.session, session.booted, session.state != .destroyed else { return }
        session.apply(session.runtime.on { session.runtime.read(exact_control_text_changed(session.runtime.rt)) })
    }
    package var decorationAlignment: NSTextAlignment {
        switch style["text_align"]?.string {
        case "center": .center
        case "right", "end": .right
        case "justify": .justified
        default: .left
        }
    }
    package var fieldDecorations: FieldDecorating? { (textArea as? TextArea)?.decorations }
    func configureDecorations() {
        guard let field = textArea as? TextArea else { return }
        if props["decorations"] != nil {
            if field.decorations == nil { field.decorations = MarkdownLink.installed?.decorator() }
        } else if let editor = field.decorations, !fieldComposing {
            editor.detach(self)
            field.decorations = nil
        }
    }
    package func decorationToggle(checked: Bool, range: NSRange) {
        guard !disabled, !inert, props["editable"] != "false", handlers.contains("toggle"), let session = presenter?.session else { return }
        let old = checked ? "checked" : "unchecked", next = checked ? "unchecked" : "checked"
        session.apply(session.runtime.clipboard(id, 50, "\(old),\(next),\(range.location),\(NSMaxRange(range))", now: session.now()))
    }
}

/// Visible accessibility text and its source boundaries. An atom contributes its
/// label as one source interval; hidden source contributes no accessible text.
package struct FieldTextProjection {
    package let value: String
    package let boundaries: [Int]
    package func source(_ range: NSRange) -> NSRange {
        let start = boundaries[min(range.location, boundaries.count - 1)]
        var end = boundaries[min(NSMaxRange(range), boundaries.count - 1)]
        if range.length > 0, end == start, let next = boundaries.first(where: { $0 > start }) { end = next }
        return NSRange(location: start, length: max(0, end - start))
    }
    package func display(_ range: NSRange) -> NSRange {
        guard range.location != NSNotFound else { return range }
        let start = boundaries.firstIndex(where: { $0 >= range.location }) ?? boundaries.count - 1
        let end = boundaries.firstIndex(where: { $0 >= NSMaxRange(range) }) ?? boundaries.count - 1
        return NSRange(location: start, length: max(0, end - start))
    }
    package init(value: String, boundaries: [Int]) { self.value = value; self.boundaries = boundaries }
}
