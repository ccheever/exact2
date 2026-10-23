#if os(iOS) || os(tvOS)
import UIKit
import CoreImage

/// A face uses the same decoded lease and natural dimensions as an image node.
enum ButtonRasterIOS {
    static func image(_ face: ButtonFace, lease: NativeRasterLease?) -> UIImage? {
        guard let declared = face.image else { return nil }
        if let pixels = lease?.image {
            let size = pixels.naturalSize, cg = pixels.image
            let scale = CGFloat(cg.width) / size.width
            if abs(CGFloat(cg.height) / scale - size.height) < 0.001 {
                return UIImage(cgImage: cg, scale: scale, orientation: .up).withRenderingMode(.alwaysOriginal)
            }
            let ci = CIImage(cgImage: cg).transformed(by: CGAffineTransform(
                scaleX: size.width / CGFloat(cg.width), y: size.height / CGFloat(cg.height)))
            return UIImage(ciImage: ci).withRenderingMode(.alwaysOriginal)
        }
        guard let size = declared.size, size.width > 0, size.height > 0 else { return nil }
        // Measurement needs the dimensions, with no bitmap allocation or decode on main.
        return UIImage(ciImage: CIImage(color: .clear).cropped(to: CGRect(origin: .zero, size: size)))
            .withRenderingMode(.alwaysOriginal)
    }
}

extension ControlHost {
    func syncRaster(_ face: ButtonFace, _ owner: NodeView) {
        guard let image = face.image else {
            guard owner.buttonRasterView != nil else { return }
            owner.buttonRasterView = nil
            owner.imageSource = nil
            owner.loadGeneration += 1
            presenter.session?.rasters.cancel(owner.id)
            owner.raster = nil
            return
        }
        let replaced = owner.buttonRasterView != image.view
        owner.buttonRasterView = image.view
        if owner.imageSource != image.source { owner.loadImage(image.source) }
        else if replaced, let size = owner.raster?.image.naturalSize {
            DispatchQueue.main.async { [weak owner, weak presenter] in
                guard let owner, let presenter, presenter.views[owner.id] === owner,
                      owner.buttonRasterView == image.view, owner.imageSource == image.source else { return }
                presenter.onIntrinsic?([(image.view, size)])
            }
        }
    }

    func refreshRaster(_ owner: NodeView) {
        guard owner.buttonRasterView != nil, presenter.views[owner.id] === owner, let button = controls[owner.id] as? NativeButtonIOS else { return }
        button.written = nil
        configureNative(button, owner, accent: owner.channels("accent_color").map { TextEngine.color($0) })
    }
}
#endif
