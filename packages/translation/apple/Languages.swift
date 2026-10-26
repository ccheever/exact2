// Languages for @exact/translation on Apple (LLP 1133.027 D7, §9): the tag
// table every native implementation shares, detection's results by the
// specification's algorithm, and the agent's fixed list and detection rule.
import Foundation

enum Languages {
    /// The agent's languages, every pair of two distinct ones `available` (§9).
    static let agent = ["ar", "de", "en", "es", "fr", "hi", "it", "ja", "ko", "pt", "ru", "zh", "zh-Hant"]

    /// `tag` canonicalised as BCP 47 is (`zh_tw` is `zh-TW`), or nil for no tag.
    static func canonical(_ tag: String) -> String? {
        let t = tag.replacingOccurrences(of: "_", with: "-")
        let parts = t.split(separator: "-", omittingEmptySubsequences: false).map(String.init)
        guard let first = parts.first, (2...8).contains(first.count), first.allSatisfy(\.isLetter),
              parts.allSatisfy({ !$0.isEmpty && $0.count <= 8 && $0.allSatisfy { $0.isLetter || $0.isNumber } }) else { return nil }
        return parts.enumerated().map { i, p in
            i == 0 ? p.lowercased() : p.count == 4 ? p.prefix(1).uppercased() + p.dropFirst().lowercased() : p.count == 2 ? p.uppercased() : p.lowercased()
        }.joined(separator: "-")
    }

    /// D7's table: the script a region implies made explicit (`zh-TW` is `zh-Hant`; bare `zh`, `zh-Hans`), then the region dropped.
    static func scripted(_ tag: String) -> String {
        let parts = tag.split(separator: "-").map(String.init)
        guard let language = parts.first else { return tag }
        var script = parts.dropFirst().first { $0.count == 4 }
        if language == "zh", script == nil {
            let region = parts.dropFirst().first { $0.count == 2 }
            script = ["TW", "HK", "MO"].contains(region ?? "") ? "Hant" : "Hans"
        }
        return [language, script].compactMap { $0 }.joined(separator: "-")
    }

    /// `tag` truncated as LLP 1060 truncates until `listed` has it, or nil.
    static func listed(_ tag: String, in listed: [String]) -> String? {
        var parts = scripted(tag).split(separator: "-").map(String.init)
        while !parts.isEmpty {
            let t = parts.joined(separator: "-")
            if listed.contains(t) { return t }
            parts.removeLast()
        }
        return nil
    }

    /// Detection's results by the specification's algorithm (D7): the
    /// platform's probabilities sorted by confidence, those below the unknown
    /// share dropped, stopping once they sum to 0.99, and `und` last with the
    /// unknown share.
    static func results(_ hypotheses: [String: Double]) -> [[String: Any]] {
        let total = hypotheses.values.reduce(0, +)
        let unknown = max(0, 1 - total)
        var out: [[String: Any]] = [], sum = 0.0
        for (language, confidence) in hypotheses.sorted(by: { $0.value > $1.value }) {
            if confidence < unknown || sum >= 0.99 { break }
            out.append(["detectedLanguage": language, "confidence": confidence])
            sum += confidence
        }
        out.append(["detectedLanguage": "und", "confidence": unknown])
        return out
    }

    /// The agent's detection (§9): any kana is Japanese, then any Hangul
    /// Korean, else the first letter's script; nil where there is no letter.
    static func ruled(_ input: String) -> String? {
        let scalars = input.unicodeScalars
        let kana = { (s: Unicode.Scalar) in (0x3040...0x30FF).contains(s.value) || (0x31F0...0x31FF).contains(s.value) || (0xFF66...0xFF9D).contains(s.value) }
        let hangul = { (s: Unicode.Scalar) in (0xAC00...0xD7AF).contains(s.value) || (0x1100...0x11FF).contains(s.value) || (0x3130...0x318F).contains(s.value) }
        if scalars.contains(where: kana) { return "ja" }
        if scalars.contains(where: hangul) { return "ko" }
        guard let first = scalars.first(where: { $0.properties.isAlphabetic }) else { return nil }
        let v = first.value
        if (0x4E00...0x9FFF).contains(v) || (0x3400...0x4DBF).contains(v) || (0x20000...0x2EBEF).contains(v) { return "zh" }
        if (0x0400...0x052F).contains(v) { return "ru" }
        if (0x0600...0x06FF).contains(v) || (0x0750...0x077F).contains(v) { return "ar" }
        if (0x0900...0x097F).contains(v) { return "hi" }
        if v < 0x0250 || (0x1E00...0x1EFF).contains(v) { return "en" }
        return nil
    }
}
