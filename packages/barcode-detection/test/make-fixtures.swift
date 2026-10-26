// Makes the vision packages' fixture images (LLP 1133.027 §7, stage 1), so
// what each holds is reproducible: the barcodes are drawn module by module
// from their symbologies' tables (GS1 General Specifications §5.2.1 for
// EAN-13 and UPC-A), the QR code by Core Image's generator, the quote in the
// system's serif. The rotated JPEGs store the upright picture turned 90°
// anticlockwise with EXIF orientation 6, which a reader turns back.
//
// usage: swift packages/barcode-detection/test/make-fixtures.swift
//   writes packages/barcode-detection/fixture/*.{png,jpg} and
//   packages/text-detection/fixture/*.{png,jpg}; run from the checkout's root.
import CoreGraphics
import CoreImage
import CoreText
import Foundation
import ImageIO
import UniformTypeIdentifiers

let L = ["0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011", "0110111", "0001011"]
let R = L.map { String($0.map { $0 == "0" ? "1" : "0" }) }
let G = R.map { String($0.reversed()) }
let parity = ["LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL", "LGGLGL"]

/// The 95 modules of an EAN-13 (a UPC-A is one whose first digit is 0).
func ean13(_ code: String) -> String {
    let d = code.compactMap { $0.wholeNumberValue }
    precondition(d.count == 13)
    let sum = (0..<12).reduce(0) { $0 + d[$1] * ($1 % 2 == 0 ? 1 : 3) }
    precondition((10 - sum % 10) % 10 == d[12], "\(code): the check digit is wrong")
    var bits = "101"
    for (i, p) in parity[d[0]].enumerated() { bits += p == "L" ? L[d[i + 1]] : G[d[i + 1]] }
    bits += "01010"
    for i in 7..<13 { bits += R[d[i]] }
    return bits + "101"
}

func context(_ width: Int, _ height: Int) -> CGContext {
    let c = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                      space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
    c.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
    c.fill(CGRect(x: 0, y: 0, width: width, height: height))
    return c
}

/// A linear barcode, 4 px a module, 11 modules of quiet zone each side, 200 px tall.
func linear(_ bits: String) -> CGImage {
    let module = 4, quiet = 11, height = 280
    let c = context((bits.count + 2 * quiet) * module, height)
    c.setFillColor(CGColor(red: 0, green: 0, blue: 0, alpha: 1))
    for (i, b) in bits.enumerated() where b == "1" { c.fill(CGRect(x: (quiet + i) * module, y: 40, width: module, height: 200)) }
    return c.makeImage()!
}

func qr(_ text: String) -> CGImage {
    let filter = CIFilter(name: "CIQRCodeGenerator")!
    filter.setValue(Data(text.utf8), forKey: "inputMessage")
    filter.setValue("M", forKey: "inputCorrectionLevel")
    let scaled = filter.outputImage!.transformed(by: CGAffineTransform(scaleX: 10, y: 10))
    let code = CIContext().createCGImage(scaled, from: scaled.extent)!
    let c = context(code.width + 80, code.height + 80)
    c.draw(code, in: CGRect(x: 40, y: 40, width: code.width, height: code.height))
    return c.makeImage()!
}

/// No code: a grey panel and a rule, so a detector has pixels to look at.
func none() -> CGImage {
    let c = context(400, 300)
    c.setFillColor(CGColor(red: 0.85, green: 0.85, blue: 0.85, alpha: 1))
    c.fill(CGRect(x: 40, y: 40, width: 320, height: 220))
    c.setFillColor(CGColor(red: 0.3, green: 0.3, blue: 0.3, alpha: 1))
    c.fill(CGRect(x: 60, y: 140, width: 280, height: 6))
    return c.makeImage()!
}

/// Printed lines, top to bottom, in a 44 pt serif.
func printed(_ lines: [String]) -> CGImage {
    let width = 1200, lineHeight = 70, height = lines.count * lineHeight + 80
    let c = context(width, height)
    let font = CTFontCreateWithName("Times New Roman" as CFString, 44, nil)
    for (i, line) in lines.enumerated() {
        let attributed = NSAttributedString(string: line, attributes: [NSAttributedString.Key(kCTFontAttributeName as String): font,
                                                                         NSAttributedString.Key(kCTForegroundColorAttributeName as String): CGColor(red: 0, green: 0, blue: 0, alpha: 1)])
        c.textPosition = CGPoint(x: 50, y: height - 40 - (i + 1) * lineHeight + 18)
        CTLineDraw(CTLineCreateWithAttributedString(attributed), c)
    }
    return c.makeImage()!
}

/// `image` turned 90° anticlockwise: what a camera held sideways stores for EXIF orientation 6.
func turned(_ image: CGImage) -> CGImage {
    let c = context(image.height, image.width)
    c.translateBy(x: CGFloat(image.height), y: 0)
    c.rotate(by: .pi / 2)
    c.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
    return c.makeImage()!
}

func write(_ image: CGImage, _ path: String, orientation: Int? = nil) {
    let jpeg = path.hasSuffix(".jpg")
    let url = URL(fileURLWithPath: path)
    let dest = CGImageDestinationCreateWithURL(url as CFURL, (jpeg ? UTType.jpeg : UTType.png).identifier as CFString, 1, nil)!
    var props: [CFString: Any] = [:]
    if jpeg { props[kCGImageDestinationLossyCompressionQuality] = 0.95 }
    if let orientation { props[kCGImagePropertyOrientation] = orientation }
    CGImageDestinationAddImage(dest, image, props as CFDictionary)
    precondition(CGImageDestinationFinalize(dest), "\(path) not written")
    print("wrote \(path) (\(image.width)×\(image.height)\(orientation.map { ", orientation \($0)" } ?? ""))")
}

let barcodes = "packages/barcode-detection/fixture", text = "packages/text-detection/fixture"
let isbn = linear(ean13("9780747532743"))
write(isbn, "\(barcodes)/isbn-9780747532743.png")
write(linear(ean13("0036000291452")), "\(barcodes)/upc-a.png")
write(qr("https://example.com/exact/qr"), "\(barcodes)/qr-url.png")
write(turned(isbn), "\(barcodes)/rotated-6.jpg", orientation: 6)
write(none(), "\(barcodes)/none.png")
let quote = printed(["It is a truth universally acknowledged,", "that a single man in possession", "of a good fortune, must be", "in want of a wife."])
write(quote, "\(text)/quote.png")
write(turned(quote), "\(text)/rotated-quote.jpg", orientation: 6)
