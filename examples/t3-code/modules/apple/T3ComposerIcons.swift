#if os(macOS)
import AppKit

// The composer's chip icons: Pierre's file icons (@pierre/trees, used by PierreEntryIcon, Apache-2.0) with
// their light and dark tints, and Lucide glyphs (ISC); see LICENSE-T3. The data is generated
// (assets/composer-icons.json, tools/generate/gen-icons.mjs) and read from the app's assets as
// T3TerminalAssets reads the terminal page: `EXACT_ASSETS` in a development run, else the bundle's
// `assets/`. Each icon renders through AppKit's SVG image rep.
enum T3ComposerIcons {
    /// The generated icons: Pierre's as SVG by token, their tints, and Lucide's glyphs by chip name.
    private struct Icons: Decodable {
        var pierre: [String: String]
        var colors: [String: [String]]
        var lucide: [String: Glyph]
        /// A Lucide glyph: its elements on the 24px grid and its stroke width.
        struct Glyph: Decodable {
            var body: String
            var width: Double
            init(from decoder: Decoder) throws {
                var pair = try decoder.unkeyedContainer()
                body = try pair.decode(String.self)
                width = try pair.decode(Double.self)
            }
        }
    }
    private static let icons: Icons = {
        guard let url = T3TerminalAssets.fileURL("composer-icons.json"), let data = try? Data(contentsOf: url),
              let icons = try? JSONDecoder().decode(Icons.self, from: data) else {
            FileHandle.standardError.write(Data("t3.composer-icons: assets/composer-icons.json is missing (tools/generate/generate.mjs writes it)\n".utf8))
            return Icons(pierre: [:], colors: [:], lucide: [:])
        }
        return icons
    }()
    static let pierre: [String: String] = icons.pierre
    static let pierreColors: [String: [String]] = icons.colors
    /// Lucide glyphs on the 24px grid (stroke icons), keyed by chip name.
    static let lucide: [String: (String, Double)] = icons.lucide.mapValues { ($0.body, $0.width) }

    /// The Pierre token for a path (createFileTreeIconResolver's common cases).
    static func token(for path: String) -> String {
        let name = T3ComposerText.basename(path).lowercased()
        let byName: [String: String] = ["package.json": "npm", "package-lock.json": "npm", "dockerfile": "docker", ".gitignore": "git",
            ".gitattributes": "git", ".gitmodules": "git", "claude.md": "claude", "bun.lock": "bun", "bun.lockb": "bun"]
        if let token = byName[name], pierre[token] != nil { return token }
        if name.hasPrefix("vite.config.") { return "vite" }
        if name.hasPrefix("tailwind.config.") { return "tailwind" }
        let ext = name.contains(".") ? String(name.split(separator: ".").last ?? "") : ""
        let byExtension: [String: String] = ["md": "markdown", "mdx": "markdown", "markdown": "markdown", "ts": "typescript", "mts": "typescript",
            "cts": "typescript", "tsx": "react", "jsx": "react", "js": "javascript", "mjs": "javascript", "cjs": "javascript", "json": "json",
            "jsonc": "json", "css": "css", "scss": "sass", "sass": "sass", "html": "html", "htm": "html", "py": "python", "rb": "ruby", "rs": "rust",
            "swift": "swift", "go": "go", "c": "c", "h": "c", "cpp": "cpp", "cc": "cpp", "cxx": "cpp", "hpp": "cpp", "sh": "bash", "bash": "bash",
            "zsh": "bash", "yml": "yml", "yaml": "yml", "png": "image", "jpg": "image", "jpeg": "image", "gif": "image", "webp": "image",
            "bmp": "image", "ico": "image", "avif": "image", "svg": "svg", "txt": "text", "log": "text", "csv": "table", "tsv": "table", "zip": "zip",
            "tar": "zip", "gz": "zip", "tgz": "zip", "7z": "zip", "rar": "zip", "sql": "database", "sqlite": "database", "db": "database",
            "ttf": "font", "otf": "font", "woff": "font", "woff2": "font", "graphql": "graphql", "gql": "graphql", "vue": "vue",
            "svelte": "svelte", "zig": "zig", "astro": "astro", "wasm": "wasm", "tf": "terraform"]
        return byExtension[ext].flatMap { pierre[$0] != nil ? $0 : nil } ?? "default"
    }

    /// inferEntryKindFromPath: a name with no dot (or only a leading one) is a folder.
    static func isFolder(_ path: String) -> Bool {
        if path.hasSuffix("/") { return true }
        let base = T3ComposerText.basename(path)
        if base.hasPrefix(".") && !base.dropFirst().contains(".") { return true }
        return !base.contains(".")
    }

    static func name(for chip: T3ComposerChip, kind: String) -> String {
        switch chip.kind {
        case "mention": return isFolder(chip.detail) ? "folder" : "pierre:" + token(for: chip.detail)
        case "skill": return "skill"
        case "citation": return "quote"
        default:
            switch kind {
            case "file": return "pierre:" + token(for: chip.label)
            case "thread": return "messages-square"
            case "preview-annotation": return "mouse-pointer-click" // browser-surface part 3: an Annotate chip
            case "image": return "image"
            case "video": return "film" // r4-composer: FileChipContent's FilmIcon
            case "pr-open", "pull-request": return "git-pull-request"
            case "pr-draft": return "git-pull-request-draft"
            case "pr-merged": return "git-merge"
            case "pr-closed": return "git-pull-request-closed"
            default: return "message-square-text"
            }
        }
    }

    private static var cache: [String: NSImage] = [:]

    static func svg(_ name: String, color: String) -> String? {
        if name.hasPrefix("pierre:") {
            guard let body = pierre[String(name.dropFirst(7))] else { return nil }
            return body.replacingOccurrences(of: "currentColor", with: color)
        }
        guard let (body, width) = lucide[name] else { return nil }
        return "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"\(color)\" stroke-width=\"\(width)\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\(body)</svg>"
    }

    /// Pierre icons keep their own colour per theme; others take `color`.
    static func draw(_ name: String, in rect: NSRect, color: NSColor, dark: Bool, path: String = "") {
        var hex = color.usingColorSpace(.sRGB).map { String(format: "#%02x%02x%02x", Int(($0.redComponent * 255).rounded()), Int(($0.greenComponent * 255).rounded()), Int(($0.blueComponent * 255).rounded())) } ?? "#71717a"
        var opacity = color.usingColorSpace(.sRGB)?.alphaComponent ?? 1
        if name.hasPrefix("pierre:"), let pair = pierreColors[String(name.dropFirst(7))] { hex = dark ? pair[1] : pair[0]; opacity = 1 }
        let key = "\(name)|\(hex)"
        let image: NSImage? = cache[key] ?? svg(name, color: hex).flatMap { NSImage(data: Data($0.utf8)) }
        // The composer's underlay is flipped; the SVG keeps its own up.
        if let image { cache[key] = image; image.draw(in: rect, from: .zero, operation: .sourceOver, fraction: opacity, respectFlipped: true, hints: nil) }
    }
}
#endif
