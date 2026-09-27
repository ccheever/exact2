// The Swift half of an app's native module (LLP 1027 D8; LLP 1067). An app's
// Swift conforms one type to `ExactNativeModule` and names it once:
//
//     func exactNativeModule() -> ExactNativeModule { MyNative() }
//
// This file supplies the C seam `exact_js::swift_native_module!()` links
// against, so the app writes no `@_cdecl`, no strdup/free pairs and no JSON
// plumbing. Compiled with the app's sources by `exact_js_bake::swift_native`.
//
// `call` answers inside the TypeScript answer that asked (`native.call`), on
// that answer's thread and inside its 100 ms budget. `later` is for work that
// takes as long as it takes (`native.later`): start it, return, and call
// `done` exactly once from whatever thread the work ends on. Its default
// answers through `call`.

import Foundation

/// Say a device topic changed: every TypeScript answer that called
/// `native.watch(topic)` is asked again, instead of polling (LLP 1016.002).
/// From any thread; before the host listens, nothing hears it.
public enum ExactNative {
  nonisolated(unsafe) fileprivate static var announce: (UnsafeMutableRawPointer?, @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>) -> Void)?
  fileprivate static let lock = NSLock()
  public static func changed(_ topic: String) {
    lock.lock()
    let target = announce
    lock.unlock()
    guard let (context, call) = target else { return }
    topic.withCString { call(context, $0) }
  }
}

/// A refusal the TypeScript side sees as a rejected promise or a thrown error.
public struct ExactNativeError: Error, CustomStringConvertible {
  public let description: String
  public init(_ description: String) { self.description = description }
}

public protocol ExactNativeModule: AnyObject {
  /// The app's own directories, before any call; open nothing here.
  func configure(data: URL, cache: URL, temporary: URL)
  func call(_ request: [String: Any]) throws -> [String: Any]
  func later(_ request: [String: Any], done: @escaping (Result<[String: Any], Error>) -> Void)
}

extension ExactNativeModule {
  public func configure(data: URL, cache: URL, temporary: URL) {}
  public func later(_ request: [String: Any], done: @escaping (Result<[String: Any], Error>) -> Void) {
    done(Result { try call(request) })
  }
}

private let module: ExactNativeModule = exactNativeModule()

private func object(_ text: UnsafePointer<CChar>) throws -> [String: Any] {
  guard let data = String(cString: text).data(using: .utf8),
    let object = try JSONSerialization.jsonObject(with: data) as? [String: Any]
  else { throw ExactNativeError("the request was not a JSON object") }
  return object
}

private func text(_ result: Result<[String: Any], Error>) -> (UnsafeMutablePointer<CChar>?, Int32) {
  switch result {
  case .success(let value):
    guard JSONSerialization.isValidJSONObject(value),
      let data = try? JSONSerialization.data(withJSONObject: value)
    else { return (strdup("the reply was not JSON"), 1) }
    return (strdup(String(decoding: data, as: UTF8.self)), 0)
  case .failure(let error):
    return (strdup(String(describing: error)), 1)
  }
}

@_cdecl("exact_native_configure")
public func exactNativeConfigure(_ data: UnsafePointer<CChar>, _ cache: UnsafePointer<CChar>, _ temporary: UnsafePointer<CChar>) {
  let url = { (path: UnsafePointer<CChar>) in URL(fileURLWithPath: String(cString: path), isDirectory: true) }
  module.configure(data: url(data), cache: url(cache), temporary: url(temporary))
}

/// JSON in; JSON out, or an error message when `failed` is set to 1.
@_cdecl("exact_native_call")
public func exactNativeCall(_ request: UnsafePointer<CChar>, _ failed: UnsafeMutablePointer<Int32>) -> UnsafeMutablePointer<CChar>? {
  let (reply, error) = text(Result { try module.call(object(request)) })
  failed.pointee = error
  return reply
}

/// Starts the work and returns; `done(context, reply, failed)` is called once.
@_cdecl("exact_native_later")
public func exactNativeLater(
  _ request: UnsafePointer<CChar>, _ context: UnsafeMutableRawPointer?,
  _ done: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?, Int32) -> Void
) {
  let finish = { (result: Result<[String: Any], Error>) in
    let (reply, failed) = text(result)
    done(context, reply, failed)
    free(reply)
  }
  do { module.later(try object(request), done: finish) } catch { finish(.failure(error)) }
}

/// Where `ExactNative.changed` announces, set once the host listens.
@_cdecl("exact_native_listen")
public func exactNativeListen(
  _ context: UnsafeMutableRawPointer?, _ call: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>) -> Void
) {
  ExactNative.lock.lock()
  ExactNative.announce = (context, call)
  ExactNative.lock.unlock()
}

@_cdecl("exact_native_free")
public func exactNativeFree(_ pointer: UnsafeMutablePointer<CChar>?) {
  free(pointer)
}
