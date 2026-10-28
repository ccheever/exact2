// @exact/geocoding on Apple (LLP 1133.021 D6-D9): MapKit's geocoding
// requests from 26 (`MKGeocodingRequest`, `MKReverseGeocodingRequest`),
// Core Location's `CLGeocoder` before, chosen by `#available`. No match
// answers `[]`; failures are DOMException's names (D8): `NetworkError`,
// `QuotaExceededError` for the platform's throttling, `AbortError` for a
// request let go.
import CoreLocation
import ExactPackage
import Foundation
import MapKit

public final class GeocodingModule: NSObject, ExactPackage {
    private let context: ExactPackageContext
    /// The request in flight for each send number, cancelled when let go.
    private var inFlight: [Int: () -> Void] = [:]

    public init(context: ExactPackageContext) {
        self.context = context
        super.init()
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        let request = context.request
        let answer = { [weak self] (places: [[String: Any]]?, error: Error?) in
            if let request { self?.inFlight[request] = nil }
            // No match is an empty answer, not a failure (D8).
            if let error, case let code = Self.code(error), code != "exact:none" { return reply(["error": code]) }
            reply(["exact:answer": places ?? []])
        }
        let locale = (args["apple"] as? [String: Any])?["preferredLocale"] as? String
        switch name {
        case "geocodingStatus":
            reply(["available": true])
        case "geocode":
            let address = args["address"] as? String ?? ""
            if let cancel = geocode(address, locale: locale, answer) { if let request { inFlight[request] = cancel } }
        case "reverseGeocode":
            let at = CLLocation(latitude: args["latitude"] as? Double ?? 0, longitude: args["longitude"] as? Double ?? 0)
            if let cancel = reverse(at, locale: locale, answer) { if let request { inFlight[request] = cancel } }
        default:
            reply(["error": "NotSupportedError"])
        }
    }

    public func cancel(_ request: Int) {
        inFlight.removeValue(forKey: request)?()
    }

    private func geocode(_ address: String, locale: String?, _ answer: @escaping ([[String: Any]]?, Error?) -> Void) -> (() -> Void)? {
        if #available(iOS 26, macOS 26, tvOS 26, *) {
            guard let r = MKGeocodingRequest(addressString: address) else { answer([], nil); return nil }
            r.preferredLocale = locale.map(Locale.init(identifier:))
            r.getMapItems { items, error in answer(items?.map(Self.place), error) }
            return { r.cancel() }
        }
        let g = CLGeocoder()
        g.geocodeAddressString(address, in: nil, preferredLocale: locale.map(Locale.init(identifier:))) { marks, error in
            answer(marks?.map(Self.place), error)
        }
        return { g.cancelGeocode() }
    }

    private func reverse(_ at: CLLocation, locale: String?, _ answer: @escaping ([[String: Any]]?, Error?) -> Void) -> (() -> Void)? {
        if #available(iOS 26, macOS 26, tvOS 26, *) {
            guard let r = MKReverseGeocodingRequest(location: at) else { answer([], nil); return nil }
            r.preferredLocale = locale.map(Locale.init(identifier:))
            r.getMapItems { items, error in answer(items?.map(Self.place), error) }
            return { r.cancel() }
        }
        let g = CLGeocoder()
        g.reverseGeocodeLocation(at, preferredLocale: locale.map(Locale.init(identifier:))) { marks, error in
            answer(marks?.map(Self.place), error)
        }
        return { g.cancelGeocode() }
    }

    /// D6's `Place` from MapKit's item (26 and later).
    @available(iOS 26, macOS 26, tvOS 26, *)
    private static func place(_ item: MKMapItem) -> [String: Any] {
        let r = item.addressRepresentations
        var p: [String: Any] = ["latitude": item.location.coordinate.latitude, "longitude": item.location.coordinate.longitude]
        p["name"] = item.name
        p["fullAddress"] = item.address?.fullAddress
        p["city"] = r?.cityName
        p["countryCode"] = r?.region?.identifier
        p["country"] = r?.regionName
        var apple: [String: Any] = [:]
        apple["shortAddress"] = item.address?.shortAddress
        apple["cityWithContext"] = r?.cityWithContext
        p["apple"] = apple
        return p
    }

    /// The same fields from Core Location's placemark (before 26).
    private static func place(_ mark: CLPlacemark) -> [String: Any] {
        var p: [String: Any] = ["latitude": mark.location?.coordinate.latitude ?? 0, "longitude": mark.location?.coordinate.longitude ?? 0]
        p["name"] = mark.name
        let lines = [mark.subThoroughfare.map { "\($0) \(mark.thoroughfare ?? "")" } ?? mark.thoroughfare, mark.locality, mark.administrativeArea, mark.postalCode, mark.country]
        let full = lines.compactMap { $0?.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }.joined(separator: ", ")
        if !full.isEmpty { p["fullAddress"] = full }
        p["city"] = mark.locality
        p["countryCode"] = mark.isoCountryCode
        p["country"] = mark.country
        return p
    }

    private static func code(_ error: Error) -> String {
        let e = error as NSError
        if e.domain == kCLErrorDomain {
            switch CLError.Code(rawValue: e.code) {
            case .geocodeFoundNoResult, .geocodeFoundPartialResult: return "exact:none"
            case .geocodeCanceled: return "AbortError"
            case .network: return "QuotaExceededError"
            default: return "NetworkError"
            }
        }
        if e.domain == MKErrorDomain {
            switch MKError.Code(rawValue: UInt(e.code)) {
            case .placemarkNotFound: return "exact:none"
            case .loadingThrottled: return "QuotaExceededError"
            default: return "NetworkError"
            }
        }
        if e.domain == NSURLErrorDomain, e.code == NSURLErrorCancelled { return "AbortError" }
        return "NetworkError"
    }
}
