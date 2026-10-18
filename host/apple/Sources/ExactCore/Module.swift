import Foundation

/// A module's pairing receipt and compiled bytes. The caller authenticates
/// a signed update or explicitly admits a development origin before preparation.
public struct ExactModule {
    public let receipt: Data
    public let bytecode: Data
    public init(receipt: Data, bytecode: Data) { self.receipt = receipt; self.bytecode = bytecode }
}
