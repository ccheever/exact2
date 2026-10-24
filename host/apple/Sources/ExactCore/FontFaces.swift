// LLP 1019 D6: a family's faces and CSS Fonts 4 §5.2 matching inside it.
import Foundation
import CoreText

package struct RegisteredFace {
    package let family: String
    package let generic: Int?
    /// The CSS weights it covers, as `@font-face`'s `font-weight`: a variable
    /// face declared over a range draws each on its `wght` axis (LLP 1019 D2).
    package let weights: ClosedRange<Int>
    /// The widths it covers, in percent, as `@font-face`'s `font-stretch`.
    package let stretches: ClosedRange<CGFloat>
    package let italic: Bool
    /// Created from the registered URL itself — never from the Contract alias
    /// or a lookup in the system font library (LLP 1019 D3).
    package let descriptor: CTFontDescriptor
    /// Whether the file has an `opsz` axis.
    let opticalSize: Bool
    package init(family: String, generic: Int?, weight: Int, italic: Bool, descriptor: CTFontDescriptor) {
        self.init(family: family, generic: generic, weights: weight...weight, stretches: 100...100, italic: italic, descriptor: descriptor)
    }
    package init(family: String, generic: Int?, weights: ClosedRange<Int>, stretches: ClosedRange<CGFloat>, italic: Bool,
                 descriptor: CTFontDescriptor) {
        self.family = family; self.generic = generic; self.weights = weights; self.stretches = stretches
        self.italic = italic; self.descriptor = descriptor
        let axes = CTFontCopyVariationAxes(CTFontCreateWithFontDescriptor(descriptor, 16, nil)) as? [[CFString: Any]] ?? []
        opticalSize = axes.contains { ($0[kCTFontVariationAxisIdentifierKey] as? NSNumber)?.intValue == 0x6f70737a }
    }

    /// Its descriptor drawn at a requested weight, width and size: each axis
    /// the face declares a range of is set to the request, clamped to the
    /// range, as a browser sets a variable face's axes from `font-weight` and
    /// `font-stretch` (CSS Fonts 4 §5.2).
    /// Once a descriptor carries a variation (these, or a pinned weight)
    /// CoreText no longer sets `opsz` from the size, so it is set here, as
    /// `font-optical-sizing: auto` sets it; CoreText clamps it to the axis.
    /// Under `none` (`FontAxis.ownOpticalSize`) the face keeps its own.
    /// `variations` (`font-variation-settings`) are set last, each axis the
    /// file has winning over the weight's and width's (CSS Fonts 4 §7.2).
    package func drawn(weight: Int, stretch: CGFloat, size: CGFloat, variations: [FontAxis] = []) -> CTFontDescriptor {
        var axes: [NSNumber: NSNumber] = [:]
        if weights.count > 1 { axes[0x77676874] = min(max(weight, weights.lowerBound), weights.upperBound) as NSNumber } // 'wght'
        if stretches.lowerBound < stretches.upperBound {
            axes[0x77647468] = min(max(stretch, stretches.lowerBound), stretches.upperBound) as NSNumber // 'wdth'
        }
        let current = CTFontDescriptorCopyAttribute(descriptor, kCTFontVariationAttribute) as? [NSNumber: NSNumber] ?? [:]
        guard !axes.isEmpty || !current.isEmpty || !variations.isEmpty else { return descriptor }
        let own = variations.first == FontAxis.ownOpticalSize
        if opticalSize && !own { axes[FontAxis.opsz as NSNumber] = size as NSNumber }
        for v in variations where v != FontAxis.ownOpticalSize { axes[v.tag as NSNumber] = v.value as NSNumber }
        var attributes: [CFString: Any] = [kCTFontVariationAttribute: current.merging(axes) { $1 }]
        if own { attributes[kCTFontOpticalSizeAttribute] = "none" }
        return CTFontDescriptorCreateCopyWithAttributes(descriptor, attributes as CFDictionary)
    }
}

extension TextEngine {
    /// A declared face drawn at its declared weight. A variable file's
    /// descriptors are its named instances, so the first is often Thin; a
    /// browser draws a face at the weight its `@font-face` names, on the
    /// `wght` axis, so the axis is set to it (clamped to the axis's range).
    /// A static face has no axis and is returned as it is.
    package static func declaredFace(_ descriptor: CTFontDescriptor, weight: Int) -> CTFontDescriptor {
        let tag = 0x77676874 // 'wght'
        let font = CTFontCreateWithFontDescriptor(descriptor, 16, nil)
        guard let axes = CTFontCopyVariationAxes(font) as? [[CFString: Any]],
              let axis = axes.first(where: { ($0[kCTFontVariationAxisIdentifierKey] as? NSNumber)?.intValue == tag }) else { return descriptor }
        let low = (axis[kCTFontVariationAxisMinimumValueKey] as? NSNumber)?.doubleValue ?? Double(weight)
        let high = (axis[kCTFontVariationAxisMaximumValueKey] as? NSNumber)?.doubleValue ?? Double(weight)
        return CTFontDescriptorCreateCopyWithVariation(descriptor, tag as CFNumber, CGFloat(min(max(Double(weight), low), high)))
    }

    /// The faces of an installed family at CSS's normal stretch, its default
    /// face first so it wins a tie (Iowan Old Style's Roman over Titling).
    /// Weight is the face's OS/2 `usWeightClass`, the number CSS matches.
    package static func installedFaces(_ name: String) -> [RegisteredFace] {
        let request = CTFontDescriptorCreateWithAttributes([kCTFontFamilyNameAttribute: name] as CFDictionary)
        let mandatory = NSSet(object: kCTFontFamilyNameAttribute) as CFSet
        guard let first = CTFontDescriptorCreateMatchingFontDescriptor(request, mandatory) else { return [] }
        let all = [first] + (CTFontDescriptorCreateMatchingFontDescriptors(request, mandatory) as? [CTFontDescriptor] ?? [])
        var seen = Set<String>(), faces: [(RegisteredFace, Bool)] = []
        for descriptor in all {
            let font = CTFontCreateWithFontDescriptor(descriptor, 16, nil)
            guard seen.insert(CTFontCopyPostScriptName(font) as String).inserted else { continue }
            let traits = CTFontCopyTraits(font) as NSDictionary
            var weight = 400
            if let os2 = CTFontCopyTable(font, CTFontTableTag(kCTFontTableOS2), []) as Data?, os2.count >= 6 {
                weight = Int(os2[os2.startIndex + 4]) << 8 | Int(os2[os2.startIndex + 5])
            } else if let trait = traits[kCTFontWeightTrait] as? Double {
                weight = Int((400 + trait * 500).rounded())
            }
            let normalWidth = abs(traits[kCTFontWidthTrait] as? Double ?? 0) < 0.01
            faces.append((RegisteredFace(family: name, generic: nil, weight: min(max(weight, 1), 1000),
                                         italic: CTFontGetSymbolicTraits(font).contains(.traitItalic), descriptor: descriptor), normalWidth))
        }
        let normal = faces.filter(\.1)
        return (normal.isEmpty ? faces : normal).map(\.0)
    }

    /// CSS Fonts 4 §5.2 inside the first family: the nearest width, then
    /// the style, then the nearest weight. A face declared over a range
    /// matches every value in it exactly, and is otherwise as near as the
    /// range's nearer end.
    package static func matched(_ faces: [RegisteredFace], weight: Int, italic: Bool, stretch: CGFloat = 100) -> RegisteredFace {
        let first = faces.filter { $0.family == faces[0].family }
        // Narrower faces first at or below normal width, wider ones above
        // it; nearer before farther in either. One number orders them.
        func width(_ face: RegisteredFace) -> CGFloat {
            let range = face.stretches
            if range.contains(stretch) { return 0 }
            let narrower = range.upperBound < stretch
            let distance: CGFloat = narrower ? stretch - range.upperBound : range.lowerBound - stretch
            let preferred = narrower == (stretch <= 100)
            return preferred ? distance : 100_000 + distance
        }
        var nearest = CGFloat.infinity
        for face in first { nearest = min(nearest, width(face)) }
        let wide = first.filter { width($0) == nearest }
        let styled = wide.filter { $0.italic == italic }
        let candidates = styled.isEmpty ? wide : styled
        func rank(_ face: RegisteredFace) -> (Int, Int) {
            let w = min(max(weight, face.weights.lowerBound), face.weights.upperBound)
            if weight >= 400 && weight <= 500 {
                if w >= weight && w <= 500 { return (0, w - weight) }
                if w < weight { return (1, weight - w) }
                return (2, w - 500)
            }
            if weight < 400 {
                return w <= weight ? (0, weight - w) : (1, w - weight)
            }
            return w >= weight ? (0, w - weight) : (1, weight - w)
        }
        return candidates.dropFirst().reduce(candidates[0]) { best, face in
            let a = rank(best), b = rank(face)
            return b.0 < a.0 || (b.0 == a.0 && b.1 < a.1) ? face : best
        }
    }
}
