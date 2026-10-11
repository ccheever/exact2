#if os(macOS)
import AppKit
import Foundation
import ImageIO
import SceneKit

/// Lane r7-device: the device models of the reference's 3D view (MIT reference, see LICENSE-T3:
/// apps/web/src/components/device/deviceModels.ts, packages/client-runtime/src/device/model.ts,
/// modelScene.ts). The models are Apple's and carry no redistribution license, so this client
/// never bundles them: it finds the connected server's own copies (`assets/<id>-<hash>.glb`,
/// referenced from the web client's chunks) and reads them as the reference's GLTFLoader does.
/// The converted assets use triangles, float positions / normals / UVs, normalized byte vertex
/// colours, 16-bit indices, WebP textures (EXT_texture_webp) and metallic-roughness materials
/// with clearcoat; that subset is what this reader accepts.
enum R7GLB {
    struct Failure: Error { let message: String }

    static func parse(_ data: Data) throws -> SCNNode {
        let bytes = [UInt8](data)
        guard bytes.count >= 20, le32(bytes, 0) == 0x46546C67, le32(bytes, 4) == 2 else { throw Failure(message: "Not a binary glTF file") }
        var offset = 12, json: [String: Any]?, binary = Data()
        while offset + 8 <= bytes.count {
            let length = Int(le32(bytes, offset)), type = le32(bytes, offset + 4)
            let body = offset + 8
            guard body + length <= bytes.count else { throw Failure(message: "Truncated glTF chunk") }
            if type == 0x4E4F534A { json = try JSONSerialization.jsonObject(with: Data(bytes[body..<body + length])) as? [String: Any] }
            if type == 0x004E4942 { binary = Data(bytes[body..<body + length]) }
            offset = body + length + (4 - length % 4) % 4
        }
        guard let json else { throw Failure(message: "glTF has no JSON chunk") }
        return try Reader(json: json, binary: binary).scene()
    }

    static func le32(_ bytes: [UInt8], _ at: Int) -> UInt32 {
        UInt32(bytes[at]) | UInt32(bytes[at + 1]) << 8 | UInt32(bytes[at + 2]) << 16 | UInt32(bytes[at + 3]) << 24
    }

    /// glTF base colours are linear; SceneKit reads NSColor components in their colour space.
    static func color(linear rgba: [Double]) -> NSColor {
        func encode(_ c: Double) -> Double { c <= 0.0031308 ? 12.92 * c : 1.055 * pow(c, 1 / 2.4) - 0.055 }
        return NSColor(srgbRed: encode(rgba[r7: 0] ?? 1), green: encode(rgba[r7: 1] ?? 1), blue: encode(rgba[r7: 2] ?? 1), alpha: rgba[r7: 3] ?? 1)
    }

    private struct Reader {
        let json: [String: Any], binary: Data
        var accessors: [[String: Any]] { json["accessors"] as? [[String: Any]] ?? [] }
        var views: [[String: Any]] { json["bufferViews"] as? [[String: Any]] ?? [] }

        func scene() throws -> SCNNode {
            let nodes = json["nodes"] as? [[String: Any]] ?? []
            let scenes = json["scenes"] as? [[String: Any]] ?? []
            let roots = (scenes[r7: json["scene"] as? Int ?? 0]?["nodes"] as? [Int]) ?? Array(nodes.indices)
            let materials = try (json["materials"] as? [[String: Any]] ?? []).map(material)
            let root = SCNNode()
            func build(_ index: Int) throws -> SCNNode {
                guard let spec = nodes[r7: index] else { throw Failure(message: "Missing node \(index)") }
                let node = SCNNode()
                node.name = spec["name"] as? String
                if let t = spec["translation"] as? [Double], t.count == 3 { node.simdPosition = SIMD3(Float(t[0]), Float(t[1]), Float(t[2])) }
                if let r = spec["rotation"] as? [Double], r.count == 4 { node.simdOrientation = simd_quatf(ix: Float(r[0]), iy: Float(r[1]), iz: Float(r[2]), r: Float(r[3])) }
                if let s = spec["scale"] as? [Double], s.count == 3 { node.simdScale = SIMD3(Float(s[0]), Float(s[1]), Float(s[2])) }
                if let m = spec["matrix"] as? [Double], m.count == 16 { node.simdTransform = simd_float4x4(columns: (SIMD4(m[0..<4].map(Float.init)), SIMD4(m[4..<8].map(Float.init)), SIMD4(m[8..<12].map(Float.init)), SIMD4(m[12..<16].map(Float.init)))) }
                if let meshIndex = spec["mesh"] as? Int, let mesh = (json["meshes"] as? [[String: Any]])?[r7: meshIndex] {
                    let primitives = mesh["primitives"] as? [[String: Any]] ?? []
                    // GLTFLoader names a one-primitive mesh's object after the mesh ("device-screen").
                    if primitives.count == 1 { node.geometry = try geometry(primitives[0], materials); node.name = mesh["name"] as? String ?? node.name }
                    else { for primitive in primitives { node.addChildNode(SCNNode(geometry: try geometry(primitive, materials))) } }
                }
                for child in spec["children"] as? [Int] ?? [] { node.addChildNode(try build(child)) }
                return node
            }
            for index in roots { root.addChildNode(try build(index)) }
            return root
        }

        /// One accessor's bytes, tightly packed, with its component count and type.
        func accessor(_ index: Int) throws -> (data: Data, count: Int, components: Int, type: Int, normalized: Bool) {
            guard let spec = accessors[r7: index], let viewIndex = spec["bufferView"] as? Int, let view = views[r7: viewIndex] else { throw Failure(message: "Unsupported accessor \(index)") }
            let count = spec["count"] as? Int ?? 0, type = spec["componentType"] as? Int ?? 0
            let components = ["SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4][spec["type"] as? String ?? ""] ?? 0
            let size = [5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4][type] ?? 0
            guard components > 0, size > 0 else { throw Failure(message: "Unsupported accessor type") }
            let element = components * size, stride = view["byteStride"] as? Int ?? element
            let start = (view["byteOffset"] as? Int ?? 0) + (spec["byteOffset"] as? Int ?? 0)
            guard count > 0, start + stride * (count - 1) + element <= binary.count else { throw Failure(message: "Accessor out of range") }
            if stride == element { return (binary.subdata(in: start..<start + element * count), count, components, type, spec["normalized"] as? Bool ?? false) }
            var packed = Data(capacity: element * count)
            for item in 0..<count { packed.append(binary.subdata(in: start + item * stride..<start + item * stride + element)) }
            return (packed, count, components, type, spec["normalized"] as? Bool ?? false)
        }

        func source(_ index: Int, _ semantic: SCNGeometrySource.Semantic) throws -> SCNGeometrySource {
            var (data, count, components, type, normalized) = try accessor(index)
            if type != 5126 {
                // Normalized bytes / shorts (vertex colours) become floats.
                let bytes = [UInt8](data), size = type == 5121 || type == 5120 ? 1 : 2
                let scale = type == 5121 ? 255.0 : type == 5123 ? 65535.0 : type == 5120 ? 127.0 : 32767.0
                var floats = [Float](repeating: 0, count: count * components)
                for i in 0..<floats.count {
                    let raw = size == 1 ? Double(bytes[i]) : Double(UInt16(bytes[i * 2]) | UInt16(bytes[i * 2 + 1]) << 8)
                    floats[i] = Float(normalized ? raw / scale : raw)
                }
                data = floats.withUnsafeBufferPointer { Data(buffer: $0) }
            }
            return SCNGeometrySource(data: data, semantic: semantic, vectorCount: count, usesFloatComponents: true, componentsPerVector: components, bytesPerComponent: 4, dataOffset: 0, dataStride: components * 4)
        }

        func geometry(_ primitive: [String: Any], _ materials: [SCNMaterial]) throws -> SCNGeometry {
            guard (primitive["mode"] as? Int ?? 4) == 4, let attributes = primitive["attributes"] as? [String: Int], let position = attributes["POSITION"] else { throw Failure(message: "Unsupported primitive") }
            var sources = [try source(position, .vertex)]
            if let normal = attributes["NORMAL"] { sources.append(try source(normal, .normal)) }
            if let uv = attributes["TEXCOORD_0"] { sources.append(try source(uv, .texcoord)) }
            if let uv = attributes["TEXCOORD_1"] { sources.append(try source(uv, .texcoord)) }
            if let color = attributes["COLOR_0"] { sources.append(try source(color, .color)) }
            let element: SCNGeometryElement
            if let indices = primitive["indices"] as? Int {
                let (data, count, _, type, _) = try accessor(indices)
                element = SCNGeometryElement(data: data, primitiveType: .triangles, primitiveCount: count / 3, bytesPerIndex: type == 5125 ? 4 : type == 5121 ? 1 : 2)
            } else {
                let count = sources[0].vectorCount
                let indices = (0..<UInt32(count)).map { $0 }
                element = SCNGeometryElement(indices: indices, primitiveType: .triangles)
            }
            let geometry = SCNGeometry(sources: sources, elements: [element])
            if let index = primitive["material"] as? Int, let material = materials[r7: index] { geometry.materials = [material] }
            return geometry
        }

        func image(texture index: Int, linear: Bool) -> CGImage? {
            guard let texture = (json["textures"] as? [[String: Any]])?[r7: index] else { return nil }
            let source = ((texture["extensions"] as? [String: Any])?["EXT_texture_webp"] as? [String: Any])?["source"] as? Int ?? texture["source"] as? Int
            guard let source, let spec = (json["images"] as? [[String: Any]])?[r7: source], let viewIndex = spec["bufferView"] as? Int, let view = views[r7: viewIndex] else { return nil }
            let start = view["byteOffset"] as? Int ?? 0, length = view["byteLength"] as? Int ?? 0
            guard start + length <= binary.count, let decoded = CGImageSourceCreateWithData(binary.subdata(in: start..<start + length) as CFData, nil), let image = CGImageSourceCreateImageAtIndex(decoded, 0, nil) else { return nil }
            // Data textures (normal, metal / rough) are linear; colour textures are sRGB.
            guard linear, let space = CGColorSpace(name: CGColorSpace.linearSRGB) else { return image }
            return image.copy(colorSpace: space) ?? image
        }

        func material(_ spec: [String: Any]) throws -> SCNMaterial {
            let material = SCNMaterial()
            material.name = spec["name"] as? String
            material.lightingModel = .physicallyBased
            material.isDoubleSided = spec["doubleSided"] as? Bool ?? false
            let pbr = spec["pbrMetallicRoughness"] as? [String: Any] ?? [:]
            let factor = (pbr["baseColorFactor"] as? [Double]) ?? [1, 1, 1, 1]
            if let base = (pbr["baseColorTexture"] as? [String: Any])?["index"] as? Int, let image = image(texture: base, linear: false) {
                material.diffuse.contents = factor.prefix(3).allSatisfy({ $0 == 1 }) ? image : R7GLB.tint(image, factor)
                material.diffuse.mappingChannel = (pbr["baseColorTexture"] as? [String: Any])?["texCoord"] as? Int ?? 0
            } else { material.diffuse.contents = R7GLB.color(linear: factor) }
            let metallic = pbr["metallicFactor"] as? Double ?? 1, roughness = pbr["roughnessFactor"] as? Double ?? 1
            if let mr = (pbr["metallicRoughnessTexture"] as? [String: Any])?["index"] as? Int, let image = image(texture: mr, linear: true) {
                material.metalness.contents = image; material.metalness.textureComponents = .blue; material.metalness.intensity = metallic
                material.roughness.contents = image; material.roughness.textureComponents = .green; material.roughness.intensity = roughness
            } else { material.metalness.contents = NSNumber(value: metallic); material.roughness.contents = NSNumber(value: roughness) }
            if let normal = (spec["normalTexture"] as? [String: Any])?["index"] as? Int, let image = image(texture: normal, linear: true) { material.normal.contents = image }
            if let occlusion = (spec["occlusionTexture"] as? [String: Any])?["index"] as? Int, let image = image(texture: occlusion, linear: true) { material.ambientOcclusion.contents = image; material.ambientOcclusion.textureComponents = .red }
            if let emissive = spec["emissiveFactor"] as? [Double], emissive.contains(where: { $0 > 0 }) {
                if let index = (spec["emissiveTexture"] as? [String: Any])?["index"] as? Int, let image = image(texture: index, linear: false) { material.emission.contents = image }
                else { material.emission.contents = R7GLB.color(linear: emissive + [1]) }
            }
            if let clearcoat = (spec["extensions"] as? [String: Any])?["KHR_materials_clearcoat"] as? [String: Any] {
                material.clearCoat.contents = NSNumber(value: clearcoat["clearcoatFactor"] as? Double ?? 0)
                material.clearCoatRoughness.contents = NSNumber(value: clearcoat["clearcoatRoughnessFactor"] as? Double ?? 0)
            }
            switch spec["alphaMode"] as? String {
            case "BLEND": material.blendMode = .alpha; material.transparencyMode = .dualLayer; material.writesToDepthBuffer = false
            case "MASK": material.transparencyMode = .aOne
            default: material.transparencyMode = .default; material.transparency = 1
            }
            if spec["alphaMode"] as? String != "BLEND", factor.count == 4, factor[3] < 1 { material.transparency = 1 }
            else if factor.count == 4 { material.transparency = CGFloat(factor[3]) }
            return material
        }
    }

    /// A base colour texture multiplied by its factor (SceneKit has no per-property tint).
    static func tint(_ image: CGImage, _ factor: [Double]) -> CGImage {
        let width = image.width, height = image.height
        guard let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: image.colorSpace ?? CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return image }
        let rect = CGRect(x: 0, y: 0, width: width, height: height)
        context.draw(image, in: rect)
        context.setBlendMode(.multiply)
        context.setFillColor(color(linear: factor).cgColor)
        context.fill(rect)
        return context.makeImage() ?? image
    }
}

extension Array {
    subscript(r7 index: Int) -> Element? { indices.contains(index) ? self[index] : nil }
}
#endif
