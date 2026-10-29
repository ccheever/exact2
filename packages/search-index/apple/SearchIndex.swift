// @exact/search-index on Apple (LLP 1133.023 D4, D14): a thin layer over the
// host's mirror, `context.contentIndex`, which keeps the entries in the
// app's container and tells Core Spotlight's named index the difference
// (ExactKit's ContentIndex.swift). The host, not this module, answers the
// reindex delegate, a search result's continuation and the generated entity
// queries, since each can come with no package loaded.
import ExactPackage
import Foundation

public final class SearchIndexModule: ExactPackage {
    private let context: ExactPackageContext

    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            let index = context.contentIndex
            let refused: String?
            switch name {
            case "setContentIndex":
                refused = index.set(kind: args["kind"] as? String, entries: args["entries"] as? [[String: Any]] ?? [])
            case "addContentIndexEntry":
                refused = index.add(args["description"] as? [String: Any] ?? [:])
            case "deleteContentIndexEntry":
                index.delete(id: args["id"] as? String ?? "")
                refused = nil
            case "contentIndexEntries":
                return reply(["exact:answer": index.entries])
            default:
                return reply(["error": "NotSupportedError"])
            }
            if let refused { return reply(["error": refused]) }
            context.changed("search-index.contentIndexEntries")
            reply([:])
        }
    }
}
