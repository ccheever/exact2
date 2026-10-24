// @exact/photos on Android (LLP 1133.019): MediaStore's images read and added to. Under the agent it asks MediaStore
// for nothing and answers from the agent's library (AgentLibrary.kt, §10); the runner holds the permission tickets and
// lays the status it keeps over each answer.
//
// - A photo reaches the app as an id (its MediaStore content URI), a picture (`photos:<id>`, which an `img` draws
//   through the reader's image door, `ContentResolver.loadThumbnail`, D2) or a copy in `app:/tmp/photos/` (emptied
//   when the module is first made in a launch).
// - The read permissions are by API level (D4): `READ_EXTERNAL_STORAGE` to 32, `READ_MEDIA_IMAGES` from 33, with
//   `READ_MEDIA_VISUAL_USER_SELECTED` from 34 (limited access), each asked only where the manifest declares it, with
//   `ACCESS_MEDIA_LOCATION` in the same request. Adding asks nothing (API 29 and later).
// - The app's own photos are read under the add line (D6): the runner lets a picture or a copy through as `owned`,
//   and MediaStore decides by opening the id.
// - Android has no event for a change in Settings: each resume reads the status again and, when it changed,
//   announces it, and revokes the pictures it drew that the app may no longer read (D6).
// - Tier 3 (D9): `queryArgsHatch` sees each `photoAssets` query's `Bundle` after the module's arguments, on the
//   module's thread; `insertValuesHatch` each add's `ContentValues`, likewise.
package exact.photos

import android.Manifest
import android.content.ContentUris
import android.content.ContentValues
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.MediaStore
import android.util.Size
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.Executors
import kotlin.math.ceil
import kotlin.math.max
import kotlin.math.min

class PhotosModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's access hatches (D9), on the module's thread. */
        @JvmStatic var queryArgsHatch: ((Bundle) -> Unit)? = null
        @JvmStatic var insertValuesHatch: ((ContentValues) -> Unit)? = null
        private const val SELECTED = "android.permission.READ_MEDIA_VISUAL_USER_SELECTED"
        private val IMAGES = MediaStore.Images.Media.EXTERNAL_CONTENT_URI
        /** Emptied once a launch. */
        private var emptied = false
    }

    private val app get() = context.application
    private val main = Handler(Looper.getMainLooper())
    /** The module's thread: queries, thumbnails, copies and adds. */
    private val work = Executors.newSingleThreadExecutor()
    private val library by lazy { AgentLibrary(app.assets) }
    private var copies = 0
    /** The status last answered or read, which a resume compares with. */
    private var said: String? = null
    /** The ids whose pictures were drawn, which a narrowed access may revoke. */
    private val drawn = LinkedHashSet<String>()

    init {
        if (!emptied) {
            emptied = true
            context.file("app:/tmp/photos/x")?.parentFile?.deleteRecursively()
        }
        if (context.agent) library.missing?.let { say(it) }
    }

    private fun say(line: String) = context.log("photos: $line")
    private fun error(code: String, message: String? = null) = JSONObject().put("error", code).apply { if (message != null) put("message", message) }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        if (context.agent) return agent(name, args, reply)
        when (name) {
            "photoLibraryStatus" -> reply(status())
            "photoAssets" -> readable(reply) { work.execute { reply(assets(args)) } }
            "copyPhotoAsset" -> copy(args, reply)
            "addToPhotoLibrary" -> work.execute { reply(add(args)) }
            "managePhotoSelection" -> manage(reply)
            else -> reply(error("NotSupportedError"))
        }
    }

    override fun resumed() {
        if (context.agent) return
        val before = said ?: return
        val now = status()
        if (now.toString() == before) return
        context.changed("photos.photoLibraryStatus")
        context.changed("photos.photoAssets")
        if (now.getString("access") == "full") return
        // What the app may read narrowed: a picture of a photo it can no longer open is withdrawn (D6).
        val ids = drawn.toList()
        work.execute {
            val gone = ids.filter { !opens(Uri.parse(it)) }
            main.post { gone.forEach { drawn.remove(it); context.revokePicture("photos:$it") } }
        }
    }

    // The fact (D5)

    private fun has(permission: String) = app.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED

    private val declared: Set<String> by lazy {
        app.packageManager.getPackageInfo(app.packageName, PackageManager.GET_PERMISSIONS).requestedPermissions.orEmpty().toSet()
    }

    /** The read permissions this API level has, the full one first, as the manifest declares them. */
    private val reads: List<String>
        get() = when {
            Build.VERSION.SDK_INT >= 34 -> listOf(Manifest.permission.READ_MEDIA_IMAGES, SELECTED)
            Build.VERSION.SDK_INT >= 33 -> listOf(Manifest.permission.READ_MEDIA_IMAGES)
            else -> listOf(Manifest.permission.READ_EXTERNAL_STORAGE)
        }.filter { it in declared }

    private fun status(): JSONObject {
        val full = reads.firstOrNull()?.let { it != SELECTED && has(it) } == true
        val limited = !full && SELECTED in reads && has(SELECTED)
        val granted = full || limited
        // Neither: the state is the host's history of the request (LLP 1133.008 D8 item 7).
        val history = reads.firstOrNull()?.let { context.permission(it) }
        val state = if (granted) "granted" else history?.state ?: "prompt"
        val out = JSONObject()
            .put("available", true)
            .put("state", state)
            .put("access", if (full) "full" else if (limited) "limited" else "none")
            .put("addState", "granted")
            .put("apple", JSONObject.NULL)
            .put("android", JSONObject()
                .put("shouldShowRationale", history?.shouldShowRationale == true)
                .put("permanentlyDenied", !granted && history?.permanentlyDenied == true)
                .put("generation", MediaStore.getVersion(app)))
        said = out.toString()
        return out
    }

    /** [then] once the app may read: asked in one request while `prompt`; refused while `denied` or undeclared. */
    private fun readable(reply: (JSONObject) -> Unit, then: () -> Unit) {
        val state = status()
        when {
            reads.isEmpty() -> reply(error("NotAllowedError", "the manifest declares no read permission"))
            state.getString("state") == "granted" -> then()
            state.getString("state") == "denied" -> reply(error("NotAllowedError"))
            else -> ask { if (status().getString("state") == "granted") then() else reply(error("NotAllowedError")) }
        }
    }

    /** The platform's dialog for the read permissions and the location one, then [done]. */
    private fun ask(done: () -> Unit) {
        val location = listOf(Manifest.permission.ACCESS_MEDIA_LOCATION).filter { it in declared }
        val asked = (reads + location).toTypedArray()
        val first = reads.first()
        context.requestPermissions(asked) {
            // A refusal with no rationale owed after it is permanent: the history `permission` reads, as
            // `askPermission` keeps it for one permission.
            val activity = context.activity
            if (status().getString("state") != "granted" && activity != null && !activity.shouldShowRequestPermissionRationale(first)) {
                app.getSharedPreferences("exact.permissions", android.content.Context.MODE_PRIVATE).edit().putBoolean(first, true).apply()
            }
            context.changed("photos.photoLibraryStatus")
            done()
        }
    }

    /** API 34's selection dialog: the read request again, while access is limited (D6). Answers the status. */
    private fun manage(reply: (JSONObject) -> Unit) {
        if (Build.VERSION.SDK_INT < 34 || status().getString("access") != "limited") return reply(status())
        context.requestPermissions(reads.toTypedArray()) {
            context.changed("photos.photoLibraryStatus")
            context.changed("photos.photoAssets")
            reply(status())
        }
    }

    // Reads

    private val columns = arrayOf(
        MediaStore.Images.Media._ID, MediaStore.Images.Media.WIDTH, MediaStore.Images.Media.HEIGHT,
        MediaStore.Images.Media.ORIENTATION, MediaStore.Images.Media.DATE_TAKEN, MediaStore.Images.Media.DATE_ADDED,
        MediaStore.Images.Media.RELATIVE_PATH, MediaStore.Images.Media.BUCKET_DISPLAY_NAME,
        MediaStore.Images.Media.OWNER_PACKAGE_NAME, MediaStore.Images.Media.MIME_TYPE, MediaStore.Images.Media.DISPLAY_NAME,
    ) + if (Build.VERSION.SDK_INT >= 30) arrayOf(MediaStore.MediaColumns.IS_FAVORITE) else emptyArray()

    private fun assets(args: JSONObject): JSONObject {
        val limit = args.optInt("limit", 60)
        val query = Bundle().apply {
            // Newest first: the date taken, else the date added (seconds, so many share one), then the row.
            putString(android.content.ContentResolver.QUERY_ARG_SQL_SORT_ORDER,
                "${MediaStore.Images.Media.DATE_TAKEN} DESC, ${MediaStore.Images.Media.DATE_ADDED} DESC, ${MediaStore.Images.Media._ID} DESC")
            putInt(android.content.ContentResolver.QUERY_ARG_LIMIT, limit)
        }
        queryArgsHatch?.invoke(query)
        val out = JSONArray()
        try {
            app.contentResolver.query(IMAGES, columns, query, null)?.use { c ->
                while (c.moveToNext()) out.put(asset(c))
            }
        } catch (e: SecurityException) { return error("NotAllowedError", e.message) }
        return JSONObject().put("assets", out)
    }

    private fun asset(c: android.database.Cursor): JSONObject {
        val id = ContentUris.withAppendedId(IMAGES, c.getLong(0)).toString()
        val turned = c.getInt(3) % 180 != 0
        val (w, h) = c.getInt(1) to c.getInt(2)
        val taken = if (c.isNull(4)) null else c.getLong(4).toDouble()
        val text = { i: Int -> if (c.isNull(i)) JSONObject.NULL else c.getString(i) }
        return JSONObject()
            .put("id", id)
            .put("mediaType", "image")
            .put("width", if (turned) h else w)
            .put("height", if (turned) w else h)
            .put("creationDate", taken ?: (c.getLong(5) * 1000.0))
            .put("favorite", c.columnCount > 11 && c.getInt(11) == 1)
            .put("picture", "photos:$id")
            .put("apple", JSONObject.NULL)
            .put("android", JSONObject().put("relativePath", text(6)).put("bucketDisplayName", text(7)).put("ownerPackageName", text(8)))
    }

    /** Whether the app may open [uri] now. */
    private fun opens(uri: Uri): Boolean = try {
        app.contentResolver.query(uri, arrayOf(MediaStore.Images.Media._ID), null, null, null)?.use { it.moveToFirst() } == true
    } catch (e: SecurityException) { false }

    override fun picture(source: String, width: Int, height: Int, fit: String, owned: Boolean, reply: (Bitmap?, String?) -> Unit) {
        val id = source.removePrefix("photos:")
        if (context.agent) return agentPicture(id, width, height, fit, owned, reply)
        val uri = Uri.parse(id)
        work.execute {
            try {
                val size = app.contentResolver.query(uri, arrayOf(MediaStore.Images.Media.WIDTH, MediaStore.Images.Media.HEIGHT), null, null, null)
                    ?.use { if (it.moveToFirst()) it.getInt(0) to it.getInt(1) else null }
                    ?: return@execute reply(null, "NotFoundError")
                val longest = longest(size, width, height, fit)
                val bitmap = app.contentResolver.loadThumbnail(uri, Size(longest, longest), null)
                main.post { drawn += id }
                reply(bitmap, null)
            } catch (e: SecurityException) {
                // Another app's photo under the add line alone (D6): an unresolved image.
                reply(null, if (owned) "NotFoundError" else "NotAllowedError")
            } catch (e: java.io.IOException) {
                reply(null, "NotFoundError")
            }
        }
    }

    /** The longest side a picture of [size] needs for a box of [width] by [height] at [fit]: no larger than it is. */
    private fun longest(size: Pair<Int, Int>, width: Int, height: Int, fit: String): Int {
        val (w, h) = size.first.toDouble().coerceAtLeast(1.0) to size.second.toDouble().coerceAtLeast(1.0)
        val factor = if (fit == "contain" || fit == "scale-down") min(width / w, height / h) else max(width / w, height / h)
        return max(1, min(max(w, h), ceil(max(w, h) * factor)).toInt())
    }

    // Copies and adds

    private fun copy(args: JSONObject, reply: (JSONObject) -> Unit) {
        val uri = Uri.parse(args.optString("asset"))
        // Opened as the app's own when it is (D6); else a read of the library, asked while `prompt`.
        work.execute {
            if (opens(uri)) return@execute reply(copied(uri, args))
            main.post {
                if (reads.isEmpty()) return@post reply(error("NotFoundError"))
                readable(reply) { work.execute { reply(if (opens(uri)) copied(uri, args) else error("NotFoundError")) } }
            }
        }
    }

    private fun copied(uri: Uri, args: JSONObject): JSONObject {
        val row = app.contentResolver.query(uri, columns, null, null, null)?.use { if (it.moveToFirst()) asset(it).put("name", it.getString(10)).put("type", it.getString(9)) else null }
            ?: return error("NotFoundError")
        val name = row.optString("name")
        val ext = name.substringAfterLast('.', "jpg")
        val path = args.optString("path").ifEmpty { null }?.let { if (it.endsWith(".$ext")) it else "$it.$ext" } ?: "app:/tmp/photos/${++copies}.$ext"
        val file = context.file(path) ?: return error("NotAllowedError")
        // The original bytes, location kept, where the app holds ACCESS_MEDIA_LOCATION (D4); the owner's are never
        // redacted, so the flag describes another app's photo opened without it.
        val original = has(Manifest.permission.ACCESS_MEDIA_LOCATION)
        val own = row.getJSONObject("android").opt("ownerPackageName") == app.packageName
        val size = try {
            file.parentFile?.mkdirs()
            app.contentResolver.openInputStream(if (original) MediaStore.setRequireOriginal(uri) else uri)?.use { input ->
                file.outputStream().use { input.copyTo(it) }
            } ?: return error("NotFoundError")
        } catch (e: SecurityException) { return error("NotAllowedError") } catch (e: java.io.IOException) { return error("UnknownError") }
        return JSONObject()
            .put("path", path).put("name", name).put("type", row.optString("type")).put("size", size)
            .put("width", row.getInt("width")).put("height", row.getInt("height")).put("duration", JSONObject.NULL)
            .put("android", JSONObject().put("locationRedacted", !original && !own))
    }

    private fun add(args: JSONObject): JSONObject {
        val path = args.optString("path")
        val file = context.file(path)?.takeIf { it.isFile } ?: return error("NotFoundError")
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.path, bounds)
        val type = bounds.outMimeType ?: return error("DataError")
        val values = ContentValues().apply {
            put(MediaStore.Images.Media.DISPLAY_NAME, file.name)
            put(MediaStore.Images.Media.MIME_TYPE, type)
            put(MediaStore.Images.Media.IS_PENDING, 1)
            args.optJSONObject("android")?.optString("relativePath")?.ifEmpty { null }?.let { put(MediaStore.Images.Media.RELATIVE_PATH, it) }
        }
        insertValuesHatch?.invoke(values)
        val resolver = app.contentResolver
        val uri = try { resolver.insert(IMAGES, values) } catch (e: IllegalArgumentException) { return error("DataError", e.message) } ?: return error("UnknownError")
        try {
            resolver.openOutputStream(uri)?.use { out -> file.inputStream().use { it.copyTo(out) } } ?: throw java.io.IOException("no stream")
            resolver.update(uri, ContentValues().apply { put(MediaStore.Images.Media.IS_PENDING, 0) }, null, null)
        } catch (e: java.io.IOException) {
            resolver.delete(uri, null, null)
            return error("UnknownError")
        }
        main.post { context.changed("photos.photoAssets") }
        // The app reads its own photo whatever its read permission (D6).
        return JSONObject().put("asset", uri.toString())
    }

    // The agent's answers (§10)

    private fun agent(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "photoLibraryStatus", "managePhotoSelection" -> reply(agentStatus())
            "photoAssets" -> reply(JSONObject().put("assets", JSONArray(library.newest(args.optInt("limit", 60)).map(::agentAsset))))
            "copyPhotoAsset" -> reply(agentCopy(args))
            "addToPhotoLibrary" -> reply(agentAdd(args))
            else -> reply(error("NotSupportedError"))
        }
    }

    /** Under the agent: the access the runner's status gives and the selection; a photo that left what the app may
     * read is revoked. */
    override fun preferred(values: JSONObject) {
        if (!context.agent) return
        val before = library.visible
        values.optJSONObject("photoLibraryStatus")?.optString("access")?.ifEmpty { null }?.let { library.access = it }
        library.selection = values.optJSONArray("selection")?.let { a -> (0 until a.length()).map { a.getString(it) } } ?: AgentLibrary.defaultSelection
        val after = library.visible
        for (id in (before - after).sorted()) {
            say("revoked photos:$id")
            context.revokePicture("photos:$id")
        }
        if (before != after) context.changed("photos.photoAssets")
    }

    override fun permissions(states: JSONObject) {
        // Denied reading: nothing is readable, whatever the status said before.
        if (context.agent && states.optString("photos") == "denied") preferred(JSONObject().put("photoLibraryStatus", JSONObject().put("access", "none")))
    }

    private fun agentStatus() = JSONObject()
        .put("available", true).put("state", "prompt").put("access", library.access).put("addState", "prompt")
        .put("apple", JSONObject.NULL).put("android", JSONObject.NULL)

    private fun agentAsset(a: AgentLibrary.Asset) = JSONObject()
        .put("id", a.id).put("mediaType", "image").put("width", a.width).put("height", a.height).put("creationDate", a.created)
        .put("favorite", false).put("picture", "photos:${a.id}").put("apple", JSONObject.NULL)
        .put("android", JSONObject().put("relativePath", JSONObject.NULL).put("bucketDisplayName", JSONObject.NULL)
            .put("ownerPackageName", if (a.own) app.packageName else JSONObject.NULL))

    private fun agentPicture(id: String, width: Int, height: Int, fit: String, owned: Boolean, reply: (Bitmap?, String?) -> Unit) {
        val asset = library.asset(id) ?: run {
            say("picture $id at ${width}x$height: NotFoundError")
            return reply(null, "NotFoundError")
        }
        say("picture $id at ${width}x$height $fit${if (owned) " owned" else ""}")
        work.execute {
            val bitmap = library.picture(asset, width, height, fit)
            reply(bitmap, if (bitmap == null) "UnknownError" else null)
        }
    }

    private fun agentCopy(args: JSONObject): JSONObject {
        val id = args.optString("asset")
        val asset = library.asset(id) ?: return error("NotFoundError")
        val ext = asset.name.substringAfterLast('.')
        val path = args.optString("path").ifEmpty { null }?.let { if (it.endsWith(".$ext")) it else "$it.$ext" } ?: "app:/tmp/photos/${++copies}.$ext"
        val file = context.file(path) ?: return error("UnknownError")
        try {
            file.parentFile?.mkdirs()
            file.writeBytes(asset.bytes)
        } catch (e: java.io.IOException) { return error("UnknownError") }
        say("copied $id to $path sha256 ${AgentLibrary.sha256(asset.bytes)}")
        return JSONObject().put("path", path).put("name", asset.name).put("type", asset.type).put("size", asset.bytes.size)
            .put("width", asset.width).put("height", asset.height).put("duration", JSONObject.NULL)
            .put("android", JSONObject().put("locationRedacted", false))
    }

    private fun agentAdd(args: JSONObject): JSONObject {
        val path = args.optString("path")
        val bytes = context.file(path)?.takeIf { it.isFile }?.readBytes() ?: return error("NotFoundError")
        val directory = context.file("app:/tmp/photos-library/x")?.parentFile ?: return error("DataError")
        val asset = library.add(bytes, directory, context.now()) ?: return error("DataError")
        say("added ${asset.id} from $path sha256 ${AgentLibrary.sha256(bytes)} ${asset.width}x${asset.height} ${asset.type}")
        context.changed("photos.photoAssets")
        // The app reads its own photo whatever its read permission (D6).
        return JSONObject().put("asset", asset.id)
    }
}
