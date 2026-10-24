package dev.exact.canvas

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.ColorSpace
import android.graphics.ImageDecoder
import android.graphics.Matrix
import android.graphics.Paint
import android.media.ExifInterface
import android.os.Build
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.File
import java.nio.ByteBuffer
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

/**
 * Android's image codec (LLP 1133.019 D16), called from exact2's storage workers through JNI
 * (`host/linux/src/canvas/codec.rs`), which drives these steps in the order and with the search every codec shares:
 * a decode upright, an edit (turn, flip, crop, size) as `Bitmap` transforms, an encode, and the metadata. A decoded
 * picture is held under a handle until it is closed. Steps run on the calling thread and never on the main one.
 *
 * `Bitmap.createBitmap` keeps a gain map through a quarter turn, a flip, a crop and a scale on API 34 and later
 * ([Ultra HDR editing](https://developer.android.com/media/grow/ultra-hdr/edit)); a free turn is drawn into its
 * bounding box on a canvas, which keeps none, so its result says `hdr` false.
 */
object ImageCodec {
    private val held = ConcurrentHashMap<Long, Bitmap>()
    private val next = AtomicLong()

    private fun error(code: String, message: String) = JSONObject().put("error", code).put("message", message).toString()

    private fun colorSpace(b: Bitmap): String = when (b.colorSpace) {
        ColorSpace.get(ColorSpace.Named.SRGB) -> "srgb"
        ColorSpace.get(ColorSpace.Named.DISPLAY_P3) -> "display-p3"
        else -> "other"
    }

    private fun hdr(b: Bitmap) = Build.VERSION.SDK_INT >= 34 && b.hasGainmap()

    private fun answer(b: Bitmap, type: String): String {
        val handle = next.incrementAndGet()
        held[handle] = b
        return JSONObject().put("handle", handle).put("width", b.width).put("height", b.height).put("type", type)
            .put("colorSpace", colorSpace(b)).put("hdr", hdr(b)).toString()
    }

    private class TooLarge(message: String) : Exception(message)

    /**
     * [source] upright, as a handle, or `{"error", "message"}`. Refused `too-large` from its header past [maxPixels];
     * in sRGB when [srgb] (an upload's). `ImageDecoder` applies EXIF orientation; whether it did is checked where the
     * size tells (a quarter turn transposes it), and the turn applied here where it did not.
     */
    @JvmStatic
    fun decode(source: ByteArray, srgb: Boolean, maxPixels: Long): String = try {
        var type = "image/jpeg"
        val decoded = ImageDecoder.decodeBitmap(ImageDecoder.createSource(ByteBuffer.wrap(source))) { decoder, info, _ ->
            val size = info.size
            if (size.width.toLong() * size.height > maxPixels) throw TooLarge("${size.width}×${size.height} pixels is over $maxPixels")
            type = info.mimeType
            decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
            if (srgb) decoder.setTargetColorSpace(ColorSpace.get(ColorSpace.Named.SRGB))
        }
        answer(upright(source, decoded), type)
    } catch (e: TooLarge) {
        error("too-large", e.message ?: "")
    } catch (e: ImageDecoder.DecodeException) {
        error("undecodable", "not an image this host decodes")
    } catch (e: java.io.IOException) {
        error("undecodable", "not an image this host decodes")
    } catch (e: OutOfMemoryError) {
        error("too-large", "the decode did not fit in memory")
    }

    /** [decoded] with the EXIF quarter turn applied, where the decoder left the stored size of a turned picture. */
    private fun upright(source: ByteArray, decoded: Bitmap): Bitmap {
        val orientation = try {
            ExifInterface(ByteArrayInputStream(source)).getAttributeInt(ExifInterface.TAG_ORIENTATION, 1)
        } catch (e: Exception) { 1 }
        // Orientations 5 to 8 transpose the picture: a decode still at the stored size (BitmapFactory's bounds, which
        // never apply EXIF) was not turned. 2 to 4 keep the size and cannot tell; the decoder is taken at its word.
        if (orientation !in 5..8 || decoded.width == decoded.height) return decoded
        val options = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
        android.graphics.BitmapFactory.decodeByteArray(source, 0, source.size, options)
        if (options.outWidth != decoded.width || options.outHeight != decoded.height) return decoded
        val m = Matrix()
        when (orientation) {
            5 -> { m.postRotate(90f); m.postScale(-1f, 1f) }
            6 -> m.postRotate(90f)
            7 -> { m.postRotate(270f); m.postScale(-1f, 1f) }
            8 -> m.postRotate(270f)
        }
        return Bitmap.createBitmap(decoded, 0, 0, decoded.width, decoded.height, m, true)
    }

    /**
     * Picture [handle] edited by [plan] (`{"rotate", "flip", "turned", "crop", "size", "smooth"}`, in D14's order): a
     * new handle, or `{"error", "message"}`.
     */
    @JvmStatic
    fun edit(handle: Long, plan: String): String = try {
        val source = held[handle] ?: return error("UnknownError", "no picture $handle")
        val p = JSONObject(plan)
        val rotate = p.getDouble("rotate").toFloat()
        val flip = p.optString("flip").takeIf { !p.isNull("flip") }
        val turned = p.getJSONArray("turned").ints()
        val crop = p.getJSONArray("crop").ints()
        val size = p.getJSONArray("size").ints()
        val smooth = p.optBoolean("smooth", true)
        var b = source
        if (rotate != 0f || flip != null) {
            val m = Matrix()
            m.postRotate(rotate)
            when (flip) {
                "horizontal" -> m.postScale(-1f, 1f)
                "vertical" -> m.postScale(1f, -1f)
            }
            b = if (rotate % 90f == 0f) {
                // A quarter turn's bounds are the frame: createBitmap moves them to the origin.
                Bitmap.createBitmap(b, 0, 0, b.width, b.height, m, true)
            } else {
                // About the centre into the frame of the turned bounding box; what it does not cover is clear.
                val (tw, th) = turned[0] to turned[1]
                val out = Bitmap.createBitmap(tw, th, Bitmap.Config.ARGB_8888, true, b.colorSpace ?: ColorSpace.get(ColorSpace.Named.SRGB))
                val centred = Matrix().apply { postTranslate(-b.width / 2f, -b.height / 2f); postConcat(m); postTranslate(tw / 2f, th / 2f) }
                Canvas(out).drawBitmap(b, centred, Paint(Paint.FILTER_BITMAP_FLAG or Paint.ANTI_ALIAS_FLAG))
                out
            }
        }
        if (crop[0] != 0 || crop[1] != 0 || crop[2] != b.width || crop[3] != b.height) {
            val x = crop[0].coerceIn(0, b.width - 1)
            val y = crop[1].coerceIn(0, b.height - 1)
            b = Bitmap.createBitmap(b, x, y, crop[2].coerceAtMost(b.width - x), crop[3].coerceAtMost(b.height - y))
        }
        if (size[0] != b.width || size[1] != b.height) b = Bitmap.createScaledBitmap(b, size[0], size[1], smooth)
        answer(b, "image/jpeg")
    } catch (e: OutOfMemoryError) {
        error("too-large", "the edit did not fit in memory")
    } catch (e: IllegalArgumentException) {
        error("UnknownError", e.message ?: "the platform refused the edit")
    }

    private fun JSONArray.ints() = IntArray(length()) { getInt(it) }

    /**
     * Picture [handle] at [width]×[height] encoded as [type] at [quality] (0 to 1): its gain map kept only where
     * [hdr], a JPEG's clear pixels on white. Null for a type this platform does not encode.
     */
    @JvmStatic
    fun encode(handle: Long, width: Int, height: Int, type: String, quality: Double, hdr: Boolean): ByteArray? {
        var b = held[handle] ?: return null
        val format = format(type) ?: return null
        if (b.width != width || b.height != height) b = Bitmap.createScaledBitmap(b, width, height, true)
        if (!hdr && hdr(b)) b = b.copy(b.config ?: Bitmap.Config.ARGB_8888, false).also { it.gainmap = null }
        if (format == Bitmap.CompressFormat.JPEG && b.hasAlpha()) {
            val white = Bitmap.createBitmap(b.width, b.height, Bitmap.Config.ARGB_8888, false, b.colorSpace ?: ColorSpace.get(ColorSpace.Named.SRGB))
            Canvas(white).apply { drawColor(Color.WHITE); drawBitmap(b, 0f, 0f, null) }
            b = white
        }
        val out = ByteArrayOutputStream()
        return if (b.compress(format, (quality * 100).toInt().coerceIn(0, 100), out)) out.toByteArray() else null
    }

    @Suppress("DEPRECATION")
    private fun format(type: String): Bitmap.CompressFormat? = when (type) {
        "image/jpeg" -> Bitmap.CompressFormat.JPEG
        "image/png" -> Bitmap.CompressFormat.PNG
        "image/webp" -> if (Build.VERSION.SDK_INT >= 30) Bitmap.CompressFormat.WEBP_LOSSY else Bitmap.CompressFormat.WEBP
        else -> null
    }

    @JvmStatic
    fun encodes(type: String) = format(type) != null

    @JvmStatic
    fun close(handle: Long) { held.remove(handle) }

    /** The standard tags that describe the capture: `no-location`'s whole set (D15), as Apple's codec keeps it. */
    private val described = listOf(
        "DateTime", "DateTimeOriginal", "DateTimeDigitized", "OffsetTime", "OffsetTimeOriginal", "OffsetTimeDigitized",
        "SubSecTime", "SubSecTimeOriginal", "SubSecTimeDigitized", "ExposureTime", "FNumber", "ExposureProgram",
        "ISOSpeedRatings", "PhotographicSensitivity", "ExposureBiasValue", "ApertureValue", "ShutterSpeedValue",
        "BrightnessValue", "MeteringMode", "Flash", "FocalLength", "FocalLengthIn35mmFilm", "LensMake", "LensModel",
        "LensSpecification", "ColorSpace", "WhiteBalance", "ExposureMode", "SceneCaptureType", "DigitalZoomRatio",
        "Make", "Model", "Software", "Artist", "Copyright",
    )

    /** What the encoder writes of the picture's own structure, and the offsets of the file it came from. */
    private fun structural(tag: String) = listOf("Pointer", "Offset", "ByteCount", "Interchange", "Thumbnail").any { it in tag } ||
        tag in setOf("Orientation", "ImageWidth", "ImageLength", "PixelXDimension", "PixelYDimension", "BitsPerSample",
            "Compression", "PhotometricInterpretation", "SamplesPerPixel", "PlanarConfiguration", "RowsPerStrip",
            "YCbCrSubSampling", "YCbCrPositioning", "NewSubfileType", "SubfileType")

    /** Every tag `ExifInterface` names. */
    private val every: List<String> by lazy {
        ExifInterface::class.java.fields.filter { it.name.startsWith("TAG_") && it.type == String::class.java }
            .mapNotNull { it.get(null) as? String }.distinct()
    }

    /**
     * [encoded] with [mode]'s set of [source]'s metadata (`keep`: every tag, location, maker note and XMP included;
     * `no-location`: [described]), its orientation reset and its size [width]×[height]. Null where `ExifInterface`
     * writes none for [type] (JPEG always; PNG and WebP from API 30).
     */
    @JvmStatic
    fun metadata(source: ByteArray, encoded: ByteArray, type: String, mode: String, width: Int, height: Int): ByteArray? {
        if (type != "image/jpeg" && Build.VERSION.SDK_INT < 30) return null
        if (type !in setOf("image/jpeg", "image/png", "image/webp")) return null
        val file = File.createTempFile("exact-edit", ".img")
        return try {
            val from = ExifInterface(ByteArrayInputStream(source))
            file.writeBytes(encoded)
            val to = ExifInterface(file.path)
            val tags = if (mode == "keep") every.filter { !structural(it) } else described
            for (tag in tags) from.getAttribute(tag)?.let { to.setAttribute(tag, it) }
            to.setAttribute(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL.toString())
            to.setAttribute(ExifInterface.TAG_PIXEL_X_DIMENSION, width.toString())
            to.setAttribute(ExifInterface.TAG_PIXEL_Y_DIMENSION, height.toString())
            to.saveAttributes()
            file.readBytes()
        } catch (e: Exception) {
            null
        } finally {
            file.delete()
        }
    }
}
