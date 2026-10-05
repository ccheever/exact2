// LLP 1019 D6: a family's faces and CSS Fonts 4 §5.2 matching inside it.
import Foundation
import CoreText

struct RegisteredFace {
    let family: String
    let generic: Int?
    let weight: Int
    let italic: Bool
    /// Created from the registered URL itself — never from the Contract alias
    /// or a lookup in the system font library (LLP 1019 D3).
    let descriptor: CTFontDescriptor
}

extension TextEngine {
    /// The faces of an installed family at CSS's normal stretch, its default
    /// face first so it wins a tie (Iowan Old Style's Roman over Titling).
    /// Weight is the face's OS/2 `usWeightClass`, the number CSS matches.
    static func installedFaces(_ name: String) -> [RegisteredFace] {
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

    static func matched(_ faces: [RegisteredFace], weight: Int, italic: Bool) -> RegisteredFace {
        let first = faces.filter { $0.family == faces[0].family }
        let styled = first.filter { $0.italic == italic }
        let candidates = styled.isEmpty ? first : styled
        func rank(_ face: RegisteredFace) -> (Int, Int) {
            let w = face.weight
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
