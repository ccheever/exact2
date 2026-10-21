#if canImport(UIKit)
import UIKit
typealias TextContentType = UITextContentType
#else
import AppKit
typealias TextContentType = NSTextContentType
#endif

/// HTML's `autocomplete` on an input or textarea (LLP 1102 §3.6) as the
/// platform's text content type, which is what its AutoFill reads. The value
/// is HTML's token list — `section-*`, `shipping`/`billing`, a contact kind
/// before a contact field, the field name, then `webauthn` — split on ASCII
/// whitespace and compared ASCII case-insensitively, and only the field name
/// has a platform counterpart. `off` alone clears the content type. `on`,
/// nothing, a list HTML's grammar refuses, or a field name the platform has
/// no type for leave `fallback`, the one the field's `type` implies: the web
/// reads each of those as the default, autofill by its own heuristics.
/// UIKit and AppKit name their types alike.
enum Autofill {
    static func contentType(_ autocomplete: String?, fallback: TextContentType?) -> TextContentType? {
        var tokens = tokenize(autocomplete ?? "")
        if tokens == ["off"] { return nil }
        if tokens.last == "webauthn" { tokens.removeLast() }
        guard let field = tokens.popLast() else { return fallback }
        let contact = field == "tel" || field.hasPrefix("tel-") || field == "email" || field == "impp"
        if contact, let kind = tokens.last, ["home", "work", "mobile", "fax", "pager"].contains(kind) { tokens.removeLast() }
        if let mode = tokens.last, mode == "shipping" || mode == "billing" { tokens.removeLast() }
        if let section = tokens.last, section.hasPrefix("section-") { tokens.removeLast() }
        return tokens.isEmpty ? fieldNames[field] ?? fallback : fallback
    }

    /// HTML's split on ASCII whitespace, each token ASCII-lowercased.
    static func tokenize(_ value: String) -> [String] {
        var tokens: [String] = [], token = String.UnicodeScalarView()
        for c in value.unicodeScalars {
            if [" ", "\t", "\n", "\r", "\u{c}"].contains(c) {
                if !token.isEmpty { tokens.append(String(token)); token = String.UnicodeScalarView() }
            } else {
                token.append(("A"..."Z").contains(c) ? Unicode.Scalar(c.value + 32)! : c)
            }
        }
        if !token.isEmpty { tokens.append(String(token)) }
        return tokens
    }

    static let fieldNames: [String: TextContentType] = [
        "name": .name, "honorific-prefix": .namePrefix, "given-name": .givenName,
        "additional-name": .middleName, "family-name": .familyName, "honorific-suffix": .nameSuffix,
        "nickname": .nickname, "organization-title": .jobTitle, "organization": .organizationName,
        "username": .username, "current-password": .password, "new-password": .newPassword,
        "one-time-code": .oneTimeCode,
        "street-address": .fullStreetAddress, "address-line1": .streetAddressLine1,
        "address-line2": .streetAddressLine2, "address-level3": .sublocality,
        "address-level2": .addressCity, "address-level1": .addressState,
        "country-name": .countryName, "postal-code": .postalCode,
        "cc-name": .creditCardName, "cc-given-name": .creditCardGivenName,
        "cc-additional-name": .creditCardMiddleName, "cc-family-name": .creditCardFamilyName,
        "cc-number": .creditCardNumber, "cc-exp": .creditCardExpiration,
        "cc-exp-month": .creditCardExpirationMonth, "cc-exp-year": .creditCardExpirationYear,
        "cc-csc": .creditCardSecurityCode, "cc-type": .creditCardType,
        "bday": .birthdate, "bday-day": .birthdateDay, "bday-month": .birthdateMonth, "bday-year": .birthdateYear,
        "tel": .telephoneNumber, "email": .emailAddress, "url": .URL,
    ]
}
