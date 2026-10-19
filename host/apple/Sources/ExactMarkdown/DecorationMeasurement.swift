// Intrinsic content uses the same attributed storage and TextKit 2 layout as paint.
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import CExact
import CoreText
import ExactCore
import ExactKit

enum DecorationMeasurement {
    static func measure(_ engine:TextEngine,request:ExactMeasureRequest,record:String,styles:String,root:Double,scale:Double,selection:NSRange? = nil) -> ExactMetrics? {
        let source = UnsafeBufferPointer(start:request.runs,count:request.count).map {
            String(decoding:UnsafeBufferPointer(start:$0.text,count:$0.len),as:UTF8.self)
        }.joined()
        let base = request.strut
        let face: (CGFloat,Int,Int,Bool,Int)->PlatformFont = {size,weight,family,italic,numeric in
            engine.font(size:size,weight:weight,family:family,italic:italic,numeric:numeric)
        }
        let look = MarkupEditor.Look(font:{face($0,$1,$2,$3,Int(base.font_variant_numeric))},size:CGFloat(base.font_size),weight:Int(base.font_weight),family:Int(base.font_family),italic:base.italic != 0,
                                     lineHeight:base.has_line_height != 0 ? CGFloat(base.line_height) : nil,ink:SystemColor.canvasText)
        let storage = NSTextStorage(string:source), editor = DecorationEditor()
        editor.font = face
        guard editor.style(storage,record:record,table:styles,look:look,selection:selection ?? NSRange(location:source.utf16.count,length:0),root:root,scale:scale,numeric:Int(base.font_variant_numeric)) else { return nil }
        guard storage.length > 0 else { return nil }
        let content = NSTextContentStorage(), layout = NSTextLayoutManager()
        content.textStorage = storage
        content.addTextLayoutManager(layout)
        let container = NSTextContainer(size:CGSize(width:request.width >= 0 ? CGFloat(request.width) : 1_000_000,height:1_000_000))
        container.lineFragmentPadding = 0
        layout.textContainer = container
        layout.ensureLayout(for:content.documentRange)
        var bounds = CGRect.null, baseline: CGFloat?
        layout.enumerateTextLayoutFragments(from:content.documentRange.location,options:[.ensuresLayout,.ensuresExtraLineFragment]) { fragment in
            bounds = bounds.union(fragment.layoutFragmentFrame)
            if baseline == nil, let line = fragment.textLineFragments.first { baseline = fragment.layoutFragmentFrame.minY + line.glyphOrigin.y }
            return true
        }
        if bounds.isNull { return ExactMetrics(width:0,height:0,baseline:-1) }
        // TextKit's fragment frame spans its container. Typographic line bounds
        // carry the actual inline extent for an unconstrained width offer.
        var width: CGFloat = 0
        layout.enumerateTextLayoutFragments(from:content.documentRange.location,options:[]) { fragment in
            for line in fragment.textLineFragments { width = max(width,line.typographicBounds.maxX) }
            return true
        }
        if request.width == -2, request.white_space != 2, request.white_space != 4 {
            let value = source as NSString
            let boundaries = TextEngine.lineBreaks(value,length:value.length).filter { offset in
                !editor.atoms.contains { $0.range.location < offset && offset < NSMaxRange($0.range) }
            }
            width = TextEngine.pieceRanges(value,boundaries:boundaries).reduce(0) { widest,piece in
                let text = NSMutableAttributedString(attributedString:storage.attributedSubstring(from:piece.range))
                if piece.hyphenated {
                    text.replaceCharacters(in:NSRange(location:text.length-1,length:1),with:"-")
                }
                let line = CTLineCreateWithAttributedString(text)
                let paragraph = text.attribute(.paragraphStyle,at:0,effectiveRange:nil) as? NSParagraphStyle
                let indent = (paragraph?.headIndent ?? 0) - min(0,paragraph?.tailIndent ?? 0)
                return max(widest,CGFloat(CTLineGetTypographicBounds(line,nil,nil,nil)) + indent)
            }
        }
        return ExactMetrics(width:Float(ceil(width)),height:Float(ceil(bounds.maxY)),baseline:baseline.map(Float.init) ?? -1)
    }
}
