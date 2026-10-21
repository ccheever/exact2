// Atomic inline boxes use the text engine's own line breaking and bidi positions.
import Foundation
import CoreText
import CExact

struct TextInlineBox: Hashable {
    let width: CGFloat
    let height: CGFloat
    let baseline: CGFloat

    init(width: CGFloat, height: CGFloat, baseline: CGFloat) {
        self.width=width; self.height=height; self.baseline=baseline
    }
    init?(_ raw: ExactTextRun) {
        guard raw.has_inline_box != 0 else {return nil}
        self.init(width:CGFloat(raw.inline_width),height:CGFloat(raw.inline_height),baseline:CGFloat(raw.inline_baseline))
    }
    func delegate() -> CTRunDelegate {
        let storage=UnsafeMutablePointer<TextInlineBox>.allocate(capacity:1)
        storage.initialize(to:self)
        var callbacks=CTRunDelegateCallbacks(version:kCTRunDelegateCurrentVersion, dealloc: { p in
            let value=p.assumingMemoryBound(to:TextInlineBox.self)
            value.deinitialize(count:1); value.deallocate()
        }, getAscent: { $0.assumingMemoryBound(to:TextInlineBox.self).pointee.baseline }, getDescent: {
            let b=$0.assumingMemoryBound(to:TextInlineBox.self).pointee; return b.height-b.baseline
        }, getWidth: { $0.assumingMemoryBound(to:TextInlineBox.self).pointee.width })
        return CTRunDelegateCreate(&callbacks,storage)!
    }
}

extension TextEngine {
    func measureInline(_ request: ExactMeasureRequest) -> ExactMetrics {
        func run(_ raw: ExactTextRun) -> Run {
            Run(text:String(decoding:UnsafeBufferPointer(start:raw.text,count:raw.len),as:UTF8.self),size:CGFloat(raw.font_size),weight:Int(raw.font_weight),family:Int(raw.font_family),italic:raw.italic != 0,lineHeight:raw.has_line_height != 0 ? CGFloat(raw.line_height) : nil,letterSpacing:CGFloat(raw.letter_spacing),numeric:Int(raw.font_variant_numeric),inlineBox:TextInlineBox(raw))
        }
        let runs=UnsafeBufferPointer(start:request.runs,count:request.count).map(run)
        let spec=Spec(runs:runs,align:Int(request.align),lineClamp:Int(request.line_clamp),color:[0,0,0,255],overflowWrap:Int(request.overflow_wrap),direction:Int(request.direction),whiteSpace:Int(request.white_space),strut:run(request.strut))
        let width=request.width >= 0 ? CGFloat(request.width) : .infinity
        let flow=request.exclusions.map {UnsafeBufferPointer(start:$0,count:request.exclusion_count).map(TextFlowShape.init)} ?? []
        let p=paragraph(spec,width:width,flow:flow)
        for (index,point) in p.atomicPositions(spec, width:width) {
            request.inline_positions?[index]=ExactInlinePosition(x:Float(point.x),y:Float(point.y))
        }
        return ExactMetrics(width:Float(p.width),height:Float(p.height),baseline:Float(p.baselines.last ?? p.firstBaseline))
    }
}

extension Paragraph {
    /// Read the delegate glyph's visual origin; caret offsets at bidi boundaries are ambiguous.
    func atomicPositions(_ spec: Spec, width: CGFloat) -> [(Int,CGPoint)] {
        var starts: [Int:Int]=[:], offset=0
        for (index,run) in spec.runs.enumerated() {
            if run.inlineBox != nil {starts[offset]=index}
            offset+=run.text.utf16.count
        }
        var result: [(Int,CGPoint)]=[]
        for line in lines.indices {
            for run in CTLineGetGlyphRuns(lines[line]) as! [CTRun] {
                guard (CTRunGetAttributes(run) as NSDictionary)[kCTRunDelegateAttributeName] != nil,
                      let index=starts[CTRunGetStringRange(run).location], let box=spec.runs[index].inlineBox,
                      CTRunGetGlyphCount(run)>0 else {continue}
                var point=CGPoint.zero
                CTRunGetPositions(run,CFRange(location:0,length:1),&point)
                result.append((index,CGPoint(x:origin(line,align:spec.align,width:width)+point.x,y:baselines[line]-box.baseline)))
            }
        }
        return result
    }
}
