#if os(iOS)
// Actual question picker and native conversions; no ordinary editor or JS bridge owns these bytes.
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import UIKit
import PhotosUI
import UniformTypeIdentifiers
import ImageIO

struct T3QuestionAttachmentConversion {
    let metadata: T3QuestionAttachmentMetadata
    let bytes: Data
    enum Source { case photo, file }
    private static func failure() -> T3QuestionAttachmentFailure { .init("This attachment could not be read or exceeds its size limit.") }
    private static func sourceData(_ url: URL, limit: Int) throws -> Data {
        guard url.isFileURL else { throw failure() }
        let fd = Darwin.open(url.path, O_RDONLY | O_NOFOLLOW | O_NONBLOCK)
        guard fd >= 0 else { throw failure() }
        defer { Darwin.close(fd) }
        var before = stat(), after = stat()
        guard fstat(fd, &before) == 0, (before.st_mode & S_IFMT) == S_IFREG, before.st_size > 0, before.st_size <= limit else { throw failure() }
        var bytes = Data(), buffer = [UInt8](repeating: 0, count: 65_536)
        while true {
            let count = Darwin.read(fd, &buffer, buffer.count)
            if count < 0 && errno == EINTR { continue }
            guard count >= 0 else { throw failure() }
            if count == 0 { break }
            guard bytes.count + count <= limit else { throw failure() }
            bytes.append(contentsOf: buffer.prefix(count))
        }
        var current = stat()
        guard fstat(fd, &after) == 0, lstat(url.path, &current) == 0,
              bytes.count == before.st_size, before.st_dev == after.st_dev, before.st_ino == after.st_ino,
              before.st_dev == current.st_dev, before.st_ino == current.st_ino, before.st_size == after.st_size,
              before.st_mtimespec.tv_sec == after.st_mtimespec.tv_sec, before.st_mtimespec.tv_nsec == after.st_mtimespec.tv_nsec,
              before.st_ctimespec.tv_sec == after.st_ctimespec.tv_sec, before.st_ctimespec.tv_nsec == after.st_ctimespec.tv_nsec,
              after.st_size == current.st_size, after.st_mtimespec.tv_sec == current.st_mtimespec.tv_sec,
              after.st_mtimespec.tv_nsec == current.st_mtimespec.tv_nsec,
              after.st_ctimespec.tv_sec == current.st_ctimespec.tv_sec,
              after.st_ctimespec.tv_nsec == current.st_ctimespec.tv_nsec else { throw failure() }
        return bytes
    }
    static func file(_ url: URL, name suppliedName: String?, source: Source, fileLimit: Int) throws -> Self {
        guard url.isFileURL else { throw failure() }
        let access = url.startAccessingSecurityScopedResource(); defer { if access { url.stopAccessingSecurityScopedResource() } }
        let name = suppliedName?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false
            ? suppliedName!.trimmingCharacters(in: .whitespacesAndNewlines) : url.lastPathComponent
        if source == .file {
            let bytes = try sourceData(url, limit: min(fileLimit, 50 * 1024 * 1024))
            let type = UTType(filenameExtension: url.pathExtension.lowercased())?.preferredMIMEType ?? "application/octet-stream"
            let metadata = T3QuestionAttachmentMetadata(kind: .file, name: name.isEmpty ? "file" : name, mimeType: type)
            try metadata.validate(size: bytes.count, fileLimit: fileLimit)
            return Self(metadata: metadata, bytes: bytes)
        }
        // ImageIO receives the already bounded, no-follow snapshot rather than reopening a path.
        let sourceBytes = try sourceData(url, limit: 50 * 1024 * 1024)
        guard let image = CGImageSourceCreateWithData(sourceBytes as CFData, [
            kCGImageSourceShouldCache: false,
        ] as CFDictionary) else { throw failure() }
        if let identifier = CGImageSourceGetType(image), let type = UTType(identifier as String),
           let mime = type.preferredMIMEType, T3QuestionAttachmentMetadata.images.contains(mime),
           sourceBytes.count <= 10 * 1024 * 1024 {
            return Self(metadata: .init(kind: .image, name: name.isEmpty ? "image" : name, mimeType: mime), bytes: sourceBytes)
        }
        guard let rendered = CGImageSourceCreateThumbnailAtIndex(image, 0, [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: 2048,
            kCGImageSourceShouldCacheImmediately: true,
        ] as CFDictionary) else { throw failure() }
        let output = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(output, UTType.jpeg.identifier as CFString, 1, nil) else { throw failure() }
        CGImageDestinationAddImage(destination, rendered, [kCGImageDestinationLossyCompressionQuality: 0.85] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else { throw failure() }
        let metadata = T3QuestionAttachmentMetadata(kind: .image, name: (name as NSString).deletingPathExtension + ".jpg", mimeType: "image/jpeg")
        try metadata.validate(size: output.length, fileLimit: fileLimit)
        return Self(metadata: metadata, bytes: output as Data)
    }
    /// expo-paste-input0.1.15: GIF is byte-preserved; other data normalizes to alpha PNG or opaque JPEG0.8.
    static func paste(_ data: Data) throws -> Self {
        if data.starts(with: Data("GIF87a".utf8)) || data.starts(with: Data("GIF89a".utf8)) {
            guard CGImageSourceCreateWithData(data as CFData, nil) != nil else { throw failure() }
            let metadata = T3QuestionAttachmentMetadata(kind: .image, name: "pasted-image.gif", mimeType: "image/gif")
            try metadata.validate(size: data.count, fileLimit: 0)
            return Self(metadata: metadata, bytes: data)
        }
        guard let image = UIImage(data: data), image.size.width > 0, image.size.height > 0 else { throw failure() }
        return try paste(image)
    }
    static func paste(_ image: UIImage) throws -> Self {
        guard image.size.width > 0, image.size.height > 0 else { throw failure() }
        let alpha = image.cgImage.map { ![CGImageAlphaInfo.none, .noneSkipFirst, .noneSkipLast].contains($0.alphaInfo) } ?? false
        let normalized: UIImage
        if image.imageOrientation == .up { normalized = image }
        else {
            let format = UIGraphicsImageRendererFormat.default(); format.scale = image.scale; format.opaque = !alpha
            normalized = UIGraphicsImageRenderer(size: image.size, format: format).image { _ in image.draw(in: CGRect(origin: .zero, size: image.size)) }
        }
        guard let bytes = alpha ? normalized.pngData() : normalized.jpegData(compressionQuality: 0.8) else { throw failure() }
        let metadata = T3QuestionAttachmentMetadata(kind: .image, name: alpha ? "pasted-image.png" : "pasted-image.jpeg", mimeType: alpha ? "image/png" : "image/jpeg")
        try metadata.validate(size: bytes.count, fileLimit: 0)
        return Self(metadata: metadata, bytes: bytes)
    }
}

/// Native caller retains this delegate. Its operation belongs to the store even if its completion is dropped.
final class T3QuestionAttachmentIntake: NSObject, PHPickerViewControllerDelegate, UIDocumentPickerDelegate, UIAdaptivePresentationControllerDelegate {
    struct Result { let operation: String; let fingerprint: String; let items: [T3QuestionAttachment]; let errors: [String] }
    private let store: T3QuestionAttachments
    private var active: T3QuestionOperation?
    private weak var picker: UIViewController?
    private var completion: ((Result) -> Void)?
    private let worker = DispatchQueue(label: "t3.question-attachment-intake", qos: .userInitiated)
    init(store: T3QuestionAttachments) { self.store = store }
    func pick(_ source: T3QuestionAttachmentConversion.Source, admission: T3QuestionAdmission, presenter: UIViewController, completion: @escaping (Result) -> Void) throws {
        precondition(Thread.isMainThread)
        guard active == nil, presenter.viewIfLoaded?.window != nil, presenter.presentedViewController == nil else { throw T3QuestionAttachmentFailure("An attachment picker cannot open here.") }
        let remaining = try store.remaining(admission)
        guard remaining > 0 else { throw T3QuestionAttachmentFailure("You can attach up to 100 attachments per question response.") }
        if source == .file && admission.fileLimit == 0 { throw T3QuestionAttachmentFailure("Files are unavailable for this question.") }
        let operation = try store.open(admission, kind: source == .file ? "files" : "photos")
        active = operation; self.completion = completion
        let controller: UIViewController
        if source == .file {
            let document = UIDocumentPickerViewController(forOpeningContentTypes: [.item], asCopy: true)
            document.allowsMultipleSelection = true; document.delegate = self; controller = document
        } else {
            var config = PHPickerConfiguration(photoLibrary: .shared())
            config.selectionLimit = remaining; config.selection = .ordered; config.preferredAssetRepresentationMode = .current
            config.filter = admission.fileLimit > 0 ? .any(of: [.images, .videos]) : .images
            let photo = PHPickerViewController(configuration: config); photo.delegate = self; controller = photo
        }
        picker = controller; presenter.present(controller, animated: true); controller.presentationController?.delegate = self
    }
    func cancel() {
        precondition(Thread.isMainThread)
        guard let operation = active else { return }
        active = nil; picker?.dismiss(animated: true); picker = nil
        let done = completion; completion = nil
        worker.async { [store] in
            var errors: [String] = []
            do { try store.cancel(operation) } catch { errors.append(error.localizedDescription) }
            DispatchQueue.main.async { done?(Result(operation: operation.id, fingerprint: operation.fingerprint, items: [], errors: errors)) }
        }
    }
    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) { cancel() }
    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) { cancel() }
    private func finish(_ operation: T3QuestionOperation, errors: [String]) {
        precondition(Thread.isMainThread)
        guard active === operation else { return }
        let done = completion; active = nil; completion = nil; picker = nil
        worker.async { [store] in
            var errors = errors, items: [T3QuestionAttachment] = []
            do { items = try store.adopt(operation) } catch { errors.append(error.localizedDescription); try? store.cancel(operation) }
            DispatchQueue.main.async { done?(Result(operation: operation.id, fingerprint: operation.fingerprint, items: items, errors: errors)) }
        }
    }
    private func ingest(_ url: URL, name: String?, source: T3QuestionAttachmentConversion.Source, operation: T3QuestionOperation, ordinal: Int) throws {
        let ticket = try store.reserve(operation, ordinal: ordinal)
        do {
            let value = try T3QuestionAttachmentConversion.file(url, name: name, source: source, fileLimit: operation.admission.fileLimit)
            try store.stage(ticket, metadata: value.metadata) { value.bytes }
        } catch { try? store.discard(ticket); throw error }
    }
    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let operation = active else { return }
        controller.dismiss(animated: true)
        worker.async { [self] in
            var errors: [String] = []
            for (index, url) in urls.enumerated() {
                do { try ingest(url, name: nil, source: .file, operation: operation, ordinal: index) }
                catch { errors.append(error.localizedDescription) }
            }
            DispatchQueue.main.async { self.finish(operation, errors: errors) }
        }
    }
    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        guard let operation = active else { return }
        picker.dismiss(animated: true)
        // Sequential provider callbacks keep each temporary URL valid while its owned copy is written.
        func next(_ index: Int, _ errors: [String]) {
            guard active === operation else { return }
            guard index < results.count else { finish(operation, errors: errors); return }
            let provider = results[index].itemProvider
            let movie = provider.hasItemConformingToTypeIdentifier(UTType.movie.identifier)
            provider.loadFileRepresentation(forTypeIdentifier: movie ? UTType.movie.identifier : UTType.image.identifier) { [self] url, error in
                var errors = errors
                do {
                    guard let url else { throw error ?? T3QuestionAttachmentFailure("The selected media is unreadable.") }
                    try ingest(url, name: provider.suggestedName, source: movie ? .file : .photo, operation: operation, ordinal: index)
                } catch { errors.append(error.localizedDescription) }
                DispatchQueue.main.async { next(index + 1, errors) }
            }
        }
        next(0, [])
    }
    /// Explicit image-paste action only. Plain text remains with the native input's normal editing path.
    func paste(providers: [NSItemProvider], admission: T3QuestionAdmission, completion: @escaping (Result) -> Void) throws {
        precondition(Thread.isMainThread)
        guard active == nil else { throw T3QuestionAttachmentFailure("An attachment operation is already active.") }
        let operation = try store.open(admission, kind: "paste"); active = operation; self.completion = completion
        let types = [UTType.gif.identifier, "public.gif", "image/gif", UTType.png.identifier, UTType.jpeg.identifier,
                     UTType.tiff.identifier, UTType.heic.identifier, "public.heif", UTType.webP.identifier, "public.webp", "image/webp", UTType.image.identifier]
        let ordered = providers.enumerated().sorted { left, right in
            let a = left.element.hasItemConformingToTypeIdentifier(UTType.gif.identifier), b = right.element.hasItemConformingToTypeIdentifier(UTType.gif.identifier)
            return a == b ? left.offset < right.offset : a
        }.map(\.element)
        var gifs = Set<String>()
        func next(_ index: Int, _ errors: [String]) {
            guard active === operation else { return }
            guard index < ordered.count else { finish(operation, errors: errors); return }
            let provider = ordered[index]
            guard let type = types.first(where: { provider.hasItemConformingToTypeIdentifier($0) }) else { next(index + 1, errors); return }
            let gifType = type.lowercased().contains("gif")
            func consume(_ result: Swift.Result<T3QuestionAttachmentConversion, Error>) {
                var errors = errors
                do {
                    let value = try result.get()
                    if value.metadata.mimeType != "image/gif" || gifs.insert(t3QuestionDigest(value.bytes)).inserted {
                        let ticket = try store.reserve(operation, ordinal: index)
                        do { try store.stage(ticket, metadata: value.metadata) { value.bytes } }
                        catch { try? store.discard(ticket); throw error }
                    }
                } catch { errors.append(error.localizedDescription) }
                DispatchQueue.main.async { next(index + 1, errors) }
            }
            provider.loadDataRepresentation(forTypeIdentifier: type) { data, error in
                if let data {
                    consume(Swift.Result { try T3QuestionAttachmentConversion.paste(data) })
                } else if !gifType, provider.canLoadObject(ofClass: UIImage.self) {
                    provider.loadObject(ofClass: UIImage.self) { object, objectError in
                        consume(Swift.Result {
                            guard let image = object as? UIImage else {
                                throw objectError ?? error ?? T3QuestionAttachmentFailure("The pasted image is unreadable.")
                            }
                            return try T3QuestionAttachmentConversion.paste(image)
                        })
                    }
                } else {
                    consume(.failure(error ?? T3QuestionAttachmentFailure("The pasted image is unreadable.")))
                }
            }
        }
        next(0, [])
    }
}

/// Dedicated question input interception. Text paste, selection, undo and IME remain UIKit's.
final class T3QuestionAttachmentTextView: UITextView {
    private static let supportedMediaTypes = [
        UTType.gif.identifier, "public.gif", "image/gif", UTType.png.identifier,
        UTType.jpeg.identifier, UTType.tiff.identifier, UTType.heic.identifier,
        "public.heif", UTType.webP.identifier, "public.webp", "image/webp", UTType.image.identifier,
    ]
    private let intake: T3QuestionAttachmentIntake
    private var admission: T3QuestionAdmission?
    var attachmentResult: ((T3QuestionAttachmentIntake.Result) -> Void)?

    init(intake: T3QuestionAttachmentIntake) {
        self.intake = intake
        super.init(frame: .zero, textContainer: nil)
    }
    required init?(coder: NSCoder) { nil }

    func bind(_ admission: T3QuestionAdmission?) {
        precondition(Thread.isMainThread)
        self.admission = admission
    }
    func close() {
        precondition(Thread.isMainThread)
        admission = nil
        intake.cancel()
    }

    override func paste(_ sender: Any?) {
        let providers = UIPasteboard.general.itemProviders.filter { provider in
            Self.supportedMediaTypes.contains { provider.hasItemConformingToTypeIdentifier($0) }
        }
        guard !providers.isEmpty else {
            super.paste(sender)
            return
        }
        // Never let UIKit insert recognized media into question text storage when
        // native ownership is absent or stale. Plain text still uses UIKit above.
        guard isEditable, let admission, admission.live else {
            UINotificationFeedbackGenerator().notificationOccurred(.error)
            return
        }
        do {
            try intake.paste(providers: providers, admission: admission) { [weak self] result in
                self?.attachmentResult?(result)
            }
        } catch {
            // A media paste does not become text merely because native intake is busy.
            UINotificationFeedbackGenerator().notificationOccurred(.error)
        }
    }
}
#endif
