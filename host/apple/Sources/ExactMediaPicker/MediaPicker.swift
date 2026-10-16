// @ref LLP 1069.002 D8, LLP 1047.001.000 E — iOS's photo picker
// (`PHPickerViewController`; no permission, no usage string) for a file
// input whose `accept` is only images and videos. PhotosUI is in its own
// module because linking it costs every launch, before `main`, the loading
// of ~300 more images (~1 s in the simulator). An app's composition links
// this module only when its plan can show the media picker (`media_picker`)
// and installs it before any session is made; the core's `Picker` asks it
// for the sheet and keeps everything else. On macOS and tvOS installing it
// does nothing: the Mac uses `NSOpenPanel`, and tvOS has no picker.
import ExactKit

public enum ExactMediaPicker {
    public static func install() {
        #if os(iOS)
        MediaPickerLink.installed = MediaPicker()
        #endif
    }
}

#if os(iOS)
import PhotosUI
import UIKit
import UniformTypeIdentifiers

final class MediaPicker: MediaPickerCapability {
    func present(over controller: UIViewController, accept: [String], multiple: Bool, done: @escaping ([(url: URL, name: String)]) -> Void) {
        var config = PHPickerConfiguration()
        config.selectionLimit = multiple ? 0 : 1
        let images = accept.contains { $0.hasPrefix("image/") }, videos = accept.contains { $0.hasPrefix("video/") }
        config.filter = images && videos ? .any(of: [.images, .videos]) : images ? .images : .videos
        // D6: HEVC/ProRes stays as is under `video/*`; otherwise H.264.
        config.preferredAssetRepresentationMode = accept.contains("video/*") || !videos ? .current : .compatible
        let picker = PHPickerViewController(configuration: config)
        let delegate = Delegate(done)
        picker.delegate = delegate
        // The picker holds its delegate weakly; the delegate lives as long as it does.
        objc_setAssociatedObject(picker, &Delegate.key, delegate, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        controller.present(picker, animated: true)
    }

    private final class Delegate: NSObject, PHPickerViewControllerDelegate {
        nonisolated(unsafe) static var key = 0
        private var done: (([(url: URL, name: String)]) -> Void)?
        init(_ done: @escaping ([(url: URL, name: String)]) -> Void) { self.done = done }

        func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
            picker.dismiss(animated: true)
            guard let done else { return }
            self.done = nil
            // A cancel finishes with no results.
            if results.isEmpty { done([]); return }
            // The provider's URL is deleted when its callback returns: stage
            // a copy there, for the core to copy into app:/tmp/picked/ (D4).
            let group = DispatchGroup(), lock = NSLock()
            var staged = [(url: URL, name: String)?](repeating: nil, count: results.count)
            for (i, result) in results.enumerated() {
                let provider = result.itemProvider
                guard let type = provider.registeredTypeIdentifiers.first(where: {
                    UTType($0).map { $0.conforms(to: .image) || $0.conforms(to: .movie) } ?? false
                }) else { continue }
                group.enter()
                provider.loadFileRepresentation(forTypeIdentifier: type) { url, _ in
                    defer { group.leave() }
                    guard let url else { return }
                    let stage = FileManager.default.temporaryDirectory.appendingPathComponent("exact-pick-\(UUID().uuidString).\(url.pathExtension)")
                    guard (try? FileManager.default.copyItem(at: url, to: stage)) != nil else { return }
                    let name = provider.suggestedName.map { $0 + "." + url.pathExtension } ?? url.lastPathComponent
                    lock.lock(); staged[i] = (stage, name); lock.unlock()
                }
            }
            group.notify(queue: .main) { done(staged.compactMap { $0 }) }
        }
    }
}
#endif
