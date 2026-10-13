// @ref LLP 1116.007 D4 — what the platform's stepping control (`UIStepper`
// on iOS, `NSStepper` on the Mac) does to an `input type="number"`, by
// HTML's rules for the web's spin buttons (`stepUp()` and `stepDown()`):
// the field's text steps, in decimal as the web's does (0.1 and 0.2 make
// 0.3), by `step` (1 when unsaid, `any` or unreadable), from the step base
// (`min`, else 0), bounded by `min` and `max`. A whole step gives a whole
// number: the text is the shortest decimal, `3`, never `3.0`. A value typed
// past `min` or `max` is brought to the bound it passed when the edit
// commits (LLP 1116.008 D5).
import Foundation

struct NumberStepping {
    /// Keep numeric editing characters, including incomplete signs and exponents.
    /// A paste follows the same rule as typing; marked text is left to the IME.
    static func insertion(_ replacement: String, in text: String, range: NSRange) -> String {
        let source = text as NSString
        guard range.location <= source.length, range.length <= source.length - range.location else { return replacement }
        let remainder = source.replacingCharacters(in: range, with: "")
        var point = remainder.contains("."), exponent = remainder.lowercased().contains("e")
        var prefix = source.substring(to: range.location).lowercased()
        let suffix = source.substring(from: range.location + range.length).lowercased()
        var result = ""
        for char in replacement.precomposedStringWithCompatibilityMapping {
            let char = char == "\u{2212}" ? "-" : char
            if "0123456789".contains(char) {}
            else if char == "+" || char == "-" {
                if let e = prefix.firstIndex(of: "e") {
                    guard prefix.last == "e" else { continue }
                    let tail = prefix[prefix.index(after: e)...] + suffix
                    if tail.contains("+") || tail.contains("-") { continue }
                }
            } else if char == ".", !point, !prefix.contains("e") { point = true }
            else if char == "e" || char == "E", !exponent { exponent = true }
            else { continue }
            result.append(char)
            prefix.append(contentsOf: char.lowercased())
        }
        return result
    }

    /// The bounds the author wrote, if they read as numbers; `max` is never
    /// below `min` (a field with no value above its floor has just that one).
    var min: Decimal?, max: Decimal?
    /// The allowed value step: a positive number, 1 otherwise.
    var step: Decimal

    init(_ props: [String: String]) {
        min = Self.decimal(props["min"])
        max = Self.decimal(props["max"])
        if let lo = min, let hi = max, hi < lo { max = lo }
        let wanted = Self.decimal(props["step"])
        step = wanted.flatMap { $0 > 0 ? $0 : nil } ?? 1
    }

    /// HTML's valid floating-point number, read from a field's text: a
    /// number's digits with an optional sign, fraction and exponent. A
    /// trailing point (`1.`, on its way to `1.5`) is the number before it;
    /// anything else that is not a number reads as none, as the web's field
    /// holds nothing for it.
    static func decimal(_ text: String?) -> Decimal? {
        guard var t = text?.trimmingCharacters(in: .whitespaces) else { return nil }
        t = t.replacingOccurrences(of: "\u{2212}", with: "-")
        if t.hasSuffix(".") { t.removeLast() }
        guard !t.isEmpty, t.range(of: #"^[-+]?([0-9]+(\.[0-9]*)?|\.[0-9]+)([eE][-+]?[0-9]+)?$"#, options: .regularExpression) != nil else { return nil }
        return Decimal(string: t)
    }

    /// A number as the field shows it: no trailing zeros or `.0`.
    static func text(_ value: Decimal) -> String { NSDecimalNumber(decimal: value).stringValue }

    static func double(_ value: Decimal) -> Double { NSDecimalNumber(decimal: value).doubleValue }

    /// Where stepping starts from: the field's number; none, an empty field
    /// or one that holds no number, starts at zero, as the web's spin
    /// buttons do.
    func value(of text: String) -> Decimal { Self.decimal(text) ?? 0 }

    /// The field's text after a step up or down, or nil when stepping
    /// changes nothing: the value is at the bound it steps toward, or past
    /// it on the side it would move away from (HTML's step 10, which keeps
    /// a stepDown from a value below `min` from raising it).
    func stepped(_ text: String, up: Bool) -> String? {
        let before = value(of: text)
        let base = min ?? 0
        var value = before
        let k = (value - base) / step
        if k == Self.round(k, .plain) {
            value += up ? step : -step
        } else {
            // Off the step: to the nearest value on it in the direction.
            value = base + Self.round(k, up ? .up : .down) * step
        }
        if let lo = min, value < lo { value = base + Self.round((lo - base) / step, .up) * step }
        if let hi = max, value > hi { value = base + Self.round((hi - base) / step, .down) * step }
        if Self.decimal(text) != nil, up ? value < before : value > before { return nil }
        // The same number again, already shown: nothing happened.
        if value == before, Self.decimal(text) != nil { return nil }
        return Self.text(value)
    }

    /// `value(of:)` held to `min` and `max`: what the platform's control
    /// shows, which dims its minus at the floor and its plus at the top.
    func shown(_ text: String) -> Decimal {
        var v = value(of: text)
        if let lo = min, v < lo { v = lo }
        if let hi = max, v > hi { v = hi }
        return v
    }

    /// A typed value as the edit commits it (LLP 1116.008 D5): the bound it
    /// passed, as the stepper holds its own to `min` and `max`, the bounds
    /// the web leaves to the browser's validation. Nil when there is nothing
    /// to change: a number within the bounds, or text that reads as no
    /// number (left alone, as the web's field keeps what it was given).
    func clamped(_ text: String) -> String? {
        guard let typed = Self.decimal(text) else { return nil }
        if let lo = min, typed < lo { return Self.text(lo) }
        if let hi = max, typed > hi { return Self.text(hi) }
        return nil
    }

    private static func round(_ d: Decimal, _ mode: NSDecimalNumber.RoundingMode) -> Decimal {
        var d = d, out = Decimal()
        NSDecimalRound(&out, &d, 0, mode)
        return out
    }
}
