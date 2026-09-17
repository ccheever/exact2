// Immutable retained ICC bytes only; never a raw UI context identity or CT layout object.
import Foundation
import CoreGraphics

enum ProfileRefusal: String, Error, Sendable {
    case missing, tooLarge, reconstruction, model, notEquivalent, roundtrip, capacity
}
struct NativeProfileStats: Sendable, Equatable {
    var bytes = 0
    var owners = 0
    var peakBytes = 0
    var drops = 0
}
final class NativeProfileAccount: @unchecked Sendable {
    private let lock = NSLock()
    private var value = NativeProfileStats()
    var stats: NativeProfileStats { lock.lock(); defer { lock.unlock() }; return value }
    fileprivate func reserve(_ count: Int) -> NativeProfileCharge? {
        lock.lock(); defer { lock.unlock() }
        guard count > 0, count <= NativeProfile.maximumBytes, value.owners < 2,
              count <= 2 * NativeProfile.maximumBytes - value.bytes else { return nil }
        value.bytes += count; value.owners += 1; value.peakBytes = max(value.peakBytes, value.bytes)
        return NativeProfileCharge(account: self, count: count)
    }
    fileprivate func release(_ count: Int) {
        lock.lock(); value.bytes -= count; value.owners -= 1; value.drops += 1; lock.unlock()
    }
}
fileprivate final class NativeProfileCharge: Sendable {
    let account: NativeProfileAccount
    let count: Int
    init(account: NativeProfileAccount, count: Int) { self.account = account; self.count = count }
    deinit { account.release(count) }
}
struct NativeProfileCapture {
    let owner: NativeProfile?
    let refusal: ProfileRefusal?
}
final class NativeProfile: Sendable, Equatable {
    static let maximumBytes = 256 * 1024
    let bytes: Data
    private let charge: NativeProfileCharge
    private init(bytes: Data, charge: NativeProfileCharge) { self.bytes = bytes; self.charge = charge }
    static func == (lhs: NativeProfile, rhs: NativeProfile) -> Bool { lhs.bytes == rhs.bytes }

    static func capture(original: CGColorSpace, data: CFData?, account: NativeProfileAccount) -> NativeProfileCapture {
        func refused(_ reason: ProfileRefusal) -> NativeProfileCapture { NativeProfileCapture(owner: nil, refusal: reason) }
        guard let data else { return refused(.missing) }
        let count = CFDataGetLength(data)
        guard count <= maximumBytes else { return refused(.tooLarge) }
        guard count > 0, let reconstructed = CGColorSpace(iccData: data) else { return refused(.reconstruction) }
        guard original.model == .rgb, original.numberOfComponents == 3,
              reconstructed.model == .rgb, reconstructed.numberOfComponents == 3 else { return refused(.model) }
        guard CFEqual(original, reconstructed) else { return refused(.notEquivalent) }
        guard let roundtrip = reconstructed.copyICCData(), CFEqual(data, roundtrip) else { return refused(.roundtrip) }
        guard let charge = account.reserve(count) else { return refused(.capacity) }
        // Own the bytes explicitly; temporary API-provided CFData and framework heaps are separate.
        let bytes = Data(bytes: CFDataGetBytePtr(data)!, count: count)
        return NativeProfileCapture(owner: NativeProfile(bytes: bytes, charge: charge), refusal: nil)
    }

    func makeSpace() -> CGColorSpace? {
        guard let space = CGColorSpace(iccData: bytes as CFData), space.model == .rgb, space.numberOfComponents == 3,
              let roundtrip = space.copyICCData(), (roundtrip as Data) == bytes else { return nil }
        return space
    }
}

struct NativeFormat: Sendable, Equatable {
    var width = 0
    var height = 0
    var bytesPerRow = 0
    var bitsPerComponent = 0
    var bitsPerPixel = 0
    var bitmapInfo: UInt32 = 0
    var antialias = false
    var valid: Bool {
        width > 0 && width <= 16384 && height > 0 && height <= 16384
            && bytesPerRow >= width * 4 && bitsPerComponent == 8 && bitsPerPixel == 32
            && bitmapInfo == CGImageAlphaInfo.premultipliedLast.rawValue && antialias
    }
}
