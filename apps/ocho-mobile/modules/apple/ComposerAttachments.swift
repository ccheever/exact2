// The composer's attachments: photos (the library or the camera) and files,
// each uploaded as soon as it is picked to fleet serve's upload route
// (`POST …/sessions/{id}/upload`, JSON name and base64 data, the bearer on
// the header). Fleet puts it in the paste folder of the machine the session
// reads input on and answers its path there; the message carries the paths
// after its text, as the desktop's paste does. Photos travel as JPEG at most
// 2048 px on a side, under what Fleet sends to another machine uncompressed.
import Foundation

#if os(iOS)
import PhotosUI
import UIKit
import UniformTypeIdentifiers

/// One picked file and where its upload stands.
final class Attachment {
    enum State { case uploading, done(String), failed(String) }
    let id = UUID()
    let name: String
    let thumbnail: UIImage?
    var state: State = .uploading

    init(name: String, thumbnail: UIImage?) {
        self.name = name
        self.thumbnail = thumbnail
    }

    var path: String? { if case .done(let path) = state { return path } else { return nil } }
    var uploading: Bool { if case .uploading = state { return true } else { return false } }
}

/// An upload's outcome: its path on the session's machine, or why not.
enum Uploaded {
    case path(String)
    case failed(String)
}

/// Sends one file to the upload route and answers its path there.
enum Uploader {
    static let maxBytes = 20 << 20

    static func upload(_ data: Data, name: String, to url: URL, auth: String,
                       done: @escaping (Uploaded) -> Void) {
        guard data.count <= maxBytes else { return done(.failed("Larger than 20 MB")) }
        // Named apart here too, so an older server (which keeps the name as
        // given) never lets two "image.jpg"s overwrite each other.
        let stamp = Int(Date().timeIntervalSince1970 * 1000)
        let body: [String: String] = ["name": "\(stamp)-\(name)", "data": data.base64EncodedString()]
        post(try? JSONSerialization.data(withJSONObject: body), to: url, auth: auth) { status, json, error in
            // A server older than the session route has only the machine's
            // (`…/machines/{m}/upload`): the file lands in that machine's
            // paste folder rather than the session's input machine's.
            if status == 404, let machine = machineRoute(url) {
                return upload(data, name: name, to: machine, auth: auth, done: done)
            }
            if let path = json?["path"] as? String, status == 200, !path.isEmpty {
                done(.path(path))
            } else {
                done(.failed((json?["error"] as? String) ?? error?.localizedDescription ?? "Upload failed (\(status))"))
            }
        }
    }

    private static func post(_ body: Data?, to url: URL, auth: String,
                             done: @escaping (Int, [String: Any]?, Error?) -> Void) {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.timeoutInterval = 120
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        if !auth.isEmpty { request.setValue(auth, forHTTPHeaderField: "Authorization") }
        request.httpBody = body
        URLSession.shared.dataTask(with: request) { data, response, error in
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            let json = data.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
            DispatchQueue.main.async { done(status, json, error) }
        }.resume()
    }

    /// `…/machines/{m}/sessions/{s}/upload` → `…/machines/{m}/upload`.
    static func machineRoute(_ url: URL) -> URL? {
        let parts = url.absoluteString.components(separatedBy: "/")
        guard parts.count > 3, parts[parts.count - 1] == "upload", parts[parts.count - 3] == "sessions" else { return nil }
        return URL(string: (parts.dropLast(3) + ["upload"]).joined(separator: "/"))
    }

    /// A photo as JPEG, its longer side at most 2048 px.
    static func jpeg(_ image: UIImage) -> Data? {
        let longest = max(image.size.width, image.size.height) * image.scale
        let factor = min(1, 2048 / max(longest, 1))
        let size = CGSize(width: image.size.width * image.scale * factor, height: image.size.height * image.scale * factor)
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        let resized = UIGraphicsImageRenderer(size: size, format: format).image { _ in
            image.draw(in: CGRect(origin: .zero, size: size))
        }
        // Under what an older server can pass to a machine it reaches over
        // SSH in one request (about 750 KB), when the photo allows.
        var quality: CGFloat = 0.8
        var data = resized.jpegData(compressionQuality: quality)
        while let bytes = data?.count, bytes > 700_000, quality > 0.35 {
            quality -= 0.15
            data = resized.jpegData(compressionQuality: quality)
        }
        return data
    }
}

/// The chips over the text: a thumbnail (or the file's icon and name), a
/// spinner while it uploads, a mark if it failed, and a remove button.
final class AttachmentTray: UIScrollView {
    static let height: CGFloat = 60
    var removed: ((UUID) -> Void)?

    init() {
        super.init(frame: .zero)
        showsHorizontalScrollIndicator = false
        alwaysBounceHorizontal = true
    }

    required init?(coder: NSCoder) { fatalError() }

    func show(_ attachments: [Attachment]) {
        subviews.forEach { $0.removeFromSuperview() }
        var x: CGFloat = 0
        for attachment in attachments {
            let chip = chip(for: attachment)
            chip.frame.origin = CGPoint(x: x, y: 4)
            addSubview(chip)
            x += chip.frame.width + 8
        }
        contentSize = CGSize(width: max(0, x - 8), height: Self.height)
    }

    private func chip(for attachment: Attachment) -> UIView {
        let side: CGFloat = 52
        let photo = attachment.thumbnail != nil
        let chip = UIView(frame: CGRect(x: 0, y: 0, width: photo ? side : 150, height: side))
        chip.backgroundColor = .tertiarySystemFill
        chip.layer.cornerRadius = 12
        chip.clipsToBounds = true
        if let thumbnail = attachment.thumbnail {
            let image = UIImageView(image: thumbnail)
            image.contentMode = .scaleAspectFill
            image.frame = chip.bounds
            chip.addSubview(image)
        } else {
            let icon = UIImageView(image: UIImage(systemName: "doc"))
            icon.tintColor = .secondaryLabel
            icon.frame = CGRect(x: 10, y: 15, width: 20, height: 22)
            icon.contentMode = .scaleAspectFit
            chip.addSubview(icon)
            let label = UILabel(frame: CGRect(x: 36, y: 0, width: chip.bounds.width - 60, height: side))
            label.text = attachment.name
            label.font = .preferredFont(forTextStyle: .footnote)
            label.textColor = .label
            label.lineBreakMode = .byTruncatingMiddle
            chip.addSubview(label)
        }
        switch attachment.state {
        case .uploading:
            let dim = UIView(frame: chip.bounds)
            dim.backgroundColor = UIColor.black.withAlphaComponent(0.25)
            chip.addSubview(dim)
            let spinner = UIActivityIndicatorView(style: .medium)
            spinner.color = .white
            spinner.center = CGPoint(x: chip.bounds.midX, y: chip.bounds.midY)
            spinner.startAnimating()
            chip.addSubview(spinner)
        case .failed(let why):
            let mark = UIImageView(image: UIImage(systemName: "exclamationmark.circle.fill"))
            mark.tintColor = .systemRed
            mark.backgroundColor = .systemBackground
            mark.layer.cornerRadius = 9
            mark.frame = CGRect(x: 4, y: chip.bounds.height - 22, width: 18, height: 18)
            chip.addSubview(mark)
            chip.accessibilityHint = why
        case .done:
            break
        }
        var remove = UIButton.Configuration.plain()
        remove.image = UIImage(systemName: "xmark.circle.fill")
        remove.baseForegroundColor = .white
        remove.contentInsets = .zero
        let button = UIButton(configuration: remove)
        button.frame = CGRect(x: chip.bounds.width - 22, y: 2, width: 20, height: 20)
        button.layer.shadowOpacity = 0.4
        button.layer.shadowRadius = 2
        button.layer.shadowOffset = .zero
        button.accessibilityLabel = "Remove \(attachment.name)"
        let id = attachment.id
        button.addAction(UIAction { [weak self] _ in self?.removed?(id) }, for: .primaryActionTriggered)
        chip.addSubview(button)
        chip.isAccessibilityElement = false
        return chip
    }
}

/// Presents the pickers and hands back what was picked: an image's JPEG and
/// thumbnail, or a file's bytes and name.
final class AttachmentPicker: NSObject, PHPickerViewControllerDelegate, UIDocumentPickerDelegate,
    UIImagePickerControllerDelegate, UINavigationControllerDelegate {
    var picked: ((_ data: Data, _ name: String, _ thumbnail: UIImage?) -> Void)?
    var failed: ((String) -> Void)?

    func menu(from view: UIView) -> UIMenu {
        var actions = [
            UIAction(title: "Photo Library", image: UIImage(systemName: "photo.on.rectangle")) { [weak self, weak view] _ in
                guard let view else { return }
                self?.photos(from: view)
            },
        ]
        if UIImagePickerController.isSourceTypeAvailable(.camera) {
            actions.append(UIAction(title: "Take Photo", image: UIImage(systemName: "camera")) { [weak self, weak view] _ in
                guard let view else { return }
                self?.camera(from: view)
            })
        }
        actions.append(UIAction(title: "Choose File", image: UIImage(systemName: "folder")) { [weak self, weak view] _ in
            guard let view else { return }
            self?.files(from: view)
        })
        return UIMenu(children: actions)
    }

    private func present(_ controller: UIViewController, from view: UIView) {
        var top = view.window?.rootViewController
        while let next = top?.presentedViewController { top = next }
        top?.present(controller, animated: true)
    }

    private func photos(from view: UIView) {
        var config = PHPickerConfiguration()
        config.filter = .images
        config.selectionLimit = 6
        let picker = PHPickerViewController(configuration: config)
        picker.delegate = self
        present(picker, from: view)
    }

    private func camera(from view: UIView) {
        let picker = UIImagePickerController()
        picker.sourceType = .camera
        picker.delegate = self
        present(picker, from: view)
    }

    private func files(from view: UIView) {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.item], asCopy: true)
        picker.allowsMultipleSelection = true
        picker.delegate = self
        present(picker, from: view)
    }

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        picker.dismiss(animated: true)
        for (index, result) in results.enumerated() {
            let provider = result.itemProvider
            guard provider.canLoadObject(ofClass: UIImage.self) else { continue }
            let base = (provider.suggestedName ?? "photo-\(index + 1)")
            provider.loadObject(ofClass: UIImage.self) { [weak self] object, _ in
                DispatchQueue.main.async { self?.image(object as? UIImage, name: base) }
            }
        }
    }

    func imagePickerController(_ picker: UIImagePickerController, didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) {
        picker.dismiss(animated: true)
        image(info[.originalImage] as? UIImage, name: "photo")
    }

    func imagePickerControllerDidCancel(_ picker: UIImagePickerController) { picker.dismiss(animated: true) }

    private func image(_ image: UIImage?, name: String) {
        guard let image, let data = Uploader.jpeg(image) else { return failed?("That photo couldn't be read.") ?? () }
        let stem = (name as NSString).deletingPathExtension
        picked?(data, (stem.isEmpty ? "photo" : stem) + ".jpg", image.preparingThumbnail(of: CGSize(width: 104, height: 104)) ?? image)
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        for url in urls {
            let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
            guard size <= Uploader.maxBytes, let data = try? Data(contentsOf: url) else {
                failed?("\(url.lastPathComponent) is larger than 20 MB.")
                continue
            }
            let isImage = UTType(filenameExtension: url.pathExtension)?.conforms(to: .image) == true
            let thumbnail = isImage ? UIImage(data: data)?.preparingThumbnail(of: CGSize(width: 104, height: 104)) : nil
            picked?(data, url.lastPathComponent, thumbnail)
        }
    }
}
#endif
