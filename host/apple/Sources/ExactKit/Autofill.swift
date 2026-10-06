#if canImport(UIKit)
import UIKit
typealias TextContentType = UITextContentType
#else
import AppKit
typealias TextContentType = NSTextContentType
#endif

/// HTML's `autocomplete` on an input or textarea (LLP 1102 §3.6) as the
/// platform's text content type, which is what its AutoFill reads. The value
/// is HTML's token list — `section-*`, `shipping`/`billing`, a contact kind,
/// then the field name, then `webauthn` — and only the field name, the last
/// token before `webauthn`, has a platform counterpart. `off` clears the
/// content type; `on`, nothing, or a field name the platform has no type for
/// leave `fallback`, the one the field's `type` implies, as the web treats a
/// value it cannot use as no value. UIKit and AppKit name their types alike.
enum Autofill {
    static func contentType(_ autocomplete: String?, fallback: TextContentType?) -> TextContentType? {
        var tokens = (autocomplete ?? "").lowercased().split(whereSeparator: { " \t\n\r\u{c}".contains($0) })
        if tokens.last == "webauthn" { tokens.removeLast() }
        guard let field = tokens.last else { return fallback }
        if tokens.count == 1, field == "off" { return nil }
        return fieldNames[String(field)] ?? fallback
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
