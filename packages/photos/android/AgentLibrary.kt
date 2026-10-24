// The agent's photo library on Android (LLP 1133.019 §10), as Apple's `AgentLibrary.swift` is: what @exact/photos
// reads and adds to under the agent, where it asks MediaStore for nothing. Its photos are `fixture/agent-1.jpg` to
// `agent-6.jpg`, which an APK carries as the app's assets under `photos-fixture/` (the fixture app's `assets/`; the
// reader puts an app's assets at `app/assets/` in the APK), with fixed creation dates, agent-1 the newest; agent-3
// is stored turned (EXIF orientation 6) and agent-5 is in Display P3. An add joins it, its file kept under the
// session's `app:/tmp/photos-library/`.
// What the app may see of it is the access the runner's status gives (`full`, `limited`, `none`) and, under
// `limited`, the selection (the agent's `selection`, else `agent-1` to `agent-3`), with the app's own adds, which
// Android lets an app read whatever its read permission (D6).
package exact.photos

import android.content.res.AssetManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.media.ExifInterface
import java.io.ByteArrayInputStream
import java.io.File
import java.security.MessageDigest
import java.time.ZoneOffset
import java.time.ZonedDateTime
import kotlin.math.ceil
import kotlin.math.max
import kotlin.math.min

internal class AgentLibrary(assets: AssetManager) {
    class Asset(
        val id: String,
        val bytes: ByteArray,
        /** Its size as the gallery shows it, orientation applied. */
        val width: Int,
        val height: Int,
        /** Ms since the epoch. */
        val created: Double,
        /** Added by the app in this session. */
        val own: Boolean,
        val type: String,
        val name: String,
    )

    companion object {
        val defaultSelection = listOf("agent-1", "agent-2", "agent-3")

        fun sha256(bytes: ByteArray): String = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

        /** [bytes]' upright size and media type, or null for bytes that are no image. */
        fun size(bytes: ByteArray): Triple<Int, Int, String>? {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
            val type = bounds.outMimeType ?: return null
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
            val turned = orientation(bytes) >= 5
            return Triple(if (turned) bounds.outHeight else bounds.outWidth, if (turned) bounds.outWidth else bounds.outHeight, type)
        }

        private fun orientation(bytes: ByteArray): Int = try {
            ExifInterface(ByteArrayInputStream(bytes)).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL)
        } catch (e: Exception) { ExifInterface.ORIENTATION_NORMAL }

        private fun extension(type: String) = when (type) { "image/png" -> "png"; "image/webp" -> "webp"; "image/heic" -> "heic"; else -> "jpg" }
    }

    val assets = ArrayList<Asset>()
    var access = "none"
    var selection = defaultSelection
    /** Why the fixtures are missing, when they are. */
    var missing: String? = null

    init {
        // 2026-0(7-n)-1n 10:(7n):00 UTC, as the fixtures' EXIF dates say.
        for (n in 1..6) {
            val bytes = try { assets.open("app/assets/photos-fixture/agent-$n.jpg").use { it.readBytes() } } catch (e: java.io.IOException) {
                missing = "the APK carries no assets/photos-fixture/agent-$n.jpg"
                continue
            }
            val created = ZonedDateTime.of(2026, 7 - n, 10 + n, 10, 7 * n, 0, 0, ZoneOffset.UTC).toInstant().toEpochMilli().toDouble()
            val (w, h, type) = size(bytes) ?: continue
            this.assets += Asset("agent-$n", bytes, w, h, created, false, type, "agent-$n.jpg")
        }
    }

    /** The ids the app may read now. */
    val visible: Set<String>
        get() = when (access) {
            "full" -> assets.map { it.id }.toSet()
            "limited" -> selection.toSet() + assets.filter { it.own }.map { it.id }
            else -> assets.filter { it.own }.map { it.id }.toSet()
        }

    fun asset(id: String): Asset? = if (id in visible) assets.firstOrNull { it.id == id } else null

    /** The newest [limit] the app may read. */
    fun newest(limit: Int): List<Asset> {
        val seen = visible
        return assets.filter { it.id in seen }.sortedByDescending { it.created }.take(limit)
    }

    /** An add: [bytes], kept in [directory], join the library at the session's [now]. Null for bytes that are no image. */
    fun add(bytes: ByteArray, directory: File, now: Double): Asset? {
        val (w, h, type) = size(bytes) ?: return null
        val id = "agent-${assets.size + 1}"
        val name = "$id.${extension(type)}"
        try {
            directory.mkdirs()
            File(directory, name).writeBytes(bytes)
        } catch (e: java.io.IOException) { return null }
        // The session's clock counts from 0 under the agent, so an add is dated a day after the newest fixture, plus
        // that clock: newer than all of them, as a photo saved today is.
        val newest = assets.filter { !it.own }.maxOfOrNull { it.created } ?: 0.0
        val asset = Asset(id, bytes, w, h, newest + 86_400_000 + now, true, type, name)
        assets += asset
        return asset
    }

    /** [asset] drawn upright for a box of [width] by [height] device pixels at [fit]: no larger than it is. */
    fun picture(asset: Asset, width: Int, height: Int, fit: String): Bitmap? {
        val (w, h) = asset.width.toDouble() to asset.height.toDouble()
        val (x, y) = width / w to height / h
        val factor = if (fit == "contain" || fit == "scale-down") min(x, y) else max(x, y)
        val scale = min(1.0, ceil(max(w, h) * factor) / max(w, h))
        // BitmapFactory never applies the EXIF orientation, so the turn below is always the file's own.
        val options = BitmapFactory.Options().apply {
            inSampleSize = Integer.highestOneBit(max(1, (1 / scale).toInt()))
            inPreferredConfig = Bitmap.Config.ARGB_8888
        }
        val stored = BitmapFactory.decodeByteArray(asset.bytes, 0, asset.bytes.size, options) ?: return null
        val o = orientation(asset.bytes)
        val turned = o >= 5
        val (uw, uh) = if (turned) stored.height to stored.width else stored.width to stored.height
        val matrix = Matrix()
        when (o) {
            ExifInterface.ORIENTATION_FLIP_HORIZONTAL -> matrix.setScale(-1f, 1f)
            ExifInterface.ORIENTATION_ROTATE_180 -> matrix.setRotate(180f)
            ExifInterface.ORIENTATION_FLIP_VERTICAL -> matrix.setScale(1f, -1f)
            ExifInterface.ORIENTATION_TRANSPOSE -> { matrix.setRotate(90f); matrix.postScale(-1f, 1f) }
            ExifInterface.ORIENTATION_ROTATE_90 -> matrix.setRotate(90f)
            ExifInterface.ORIENTATION_TRANSVERSE -> { matrix.setRotate(-90f); matrix.postScale(-1f, 1f) }
            ExifInterface.ORIENTATION_ROTATE_270 -> matrix.setRotate(-90f)
        }
        val target = max(1.0, ceil(max(w, h) * scale))
        val s = (target / max(uw, uh)).toFloat()
        matrix.postScale(s, s)
        return Bitmap.createBitmap(stored, 0, 0, stored.width, stored.height, matrix, true)
    }
}
