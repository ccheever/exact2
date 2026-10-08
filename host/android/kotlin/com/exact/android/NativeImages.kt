package com.exact.android

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Handler
import android.os.Looper
import android.util.LruCache
import java.util.ArrayDeque
import java.util.concurrent.Executors
import kotlin.math.ceil
import kotlin.math.max
import kotlin.math.min

/** Asset images decoded off the owner thread at their displayed pixel size.
 * Two active decodes and cancellable waiting requests bound work during a fling.
 * The cache is bounded by decoded bytes; callers hold no bitmap after unmount.
 */
internal class NativeImages(private val context: Context) : AutoCloseable {
    data class Image(val bitmap: Bitmap, val width: Int, val height: Int)
    internal class Request(val source: String, val width: Int, val height: Int, val fit: String,
        val deliver: (Image) -> Unit) { var cancelled = false }
    private data class Key(val source: String, val width: Int, val height: Int, val fit: String)
    private val handler = Handler(Looper.getMainLooper())
    private val workers = Executors.newFixedThreadPool(2)
    private val waiting = ArrayDeque<Request>()
    private val cache = object : LruCache<Key, Image>(32 * 1024 * 1024) {
        override fun sizeOf(key: Key, value: Image) = value.bitmap.allocationByteCount
    }
    private var active = 0
    private var closed = false

    fun request(source: String, width: Int, height: Int, fit: String, deliver: (Image) -> Unit): Request {
        check(!closed && Looper.myLooper() == Looper.getMainLooper())
        val request = Request(source, width.coerceAtLeast(1), height.coerceAtLeast(1), fit, deliver)
        val key = Key(request.source, request.width, request.height, request.fit)
        val cached = cache.get(key)
        if (cached != null) handler.post { if (!closed && !request.cancelled) deliver(cached) }
        else { waiting.addLast(request); pump() }
        return request
    }
    fun cancel(request: Request?) {
        if (request == null) return
        request.cancelled = true
        waiting.remove(request)
    }
    private fun pump() {
        while (!closed && active < 2 && waiting.isNotEmpty()) {
            val request = waiting.removeFirst()
            if (request.cancelled) continue
            active++
            workers.execute {
                val result = runCatching { decode(request) }
                handler.post {
                    active--
                    if (!closed && !request.cancelled) {
                        val image = result.getOrThrow()
                        cache.put(Key(request.source, request.width, request.height, request.fit), image)
                        request.deliver(image)
                    }
                    pump()
                }
            }
        }
    }
    private fun decode(request: Request): Image {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true; inScaled = false }
        context.assets.open(request.source).use { BitmapFactory.decodeStream(it, null, bounds) }
        require(bounds.outWidth > 0 && bounds.outHeight > 0) { "Android asset image '${request.source}' is invalid" }
        val rw = request.width.toDouble() / bounds.outWidth
        val rh = request.height.toDouble() / bounds.outHeight
        val factor = when (request.fit) {
            "cover", "fill" -> max(rw, rh)
            "contain", "scale-down" -> min(rw, rh)
            "none" -> 1.0
            else -> error("Android object-fit '${request.fit}' is invalid")
        }.coerceAtMost(1.0)
        val width = ceil(bounds.outWidth * factor).toInt().coerceAtLeast(1)
        val height = ceil(bounds.outHeight * factor).toInt().coerceAtLeast(1)
        var sample = 1
        while (bounds.outWidth / (sample * 2) >= width && bounds.outHeight / (sample * 2) >= height) sample *= 2
        val options = BitmapFactory.Options().apply { inSampleSize = sample; inScaled = false }
        val bitmap = context.assets.open(request.source).use { BitmapFactory.decodeStream(it, null, options) }
            ?: error("Android asset image '${request.source}' could not be decoded")
        val sized = if (bitmap.width == width && bitmap.height == height) bitmap else {
            Bitmap.createScaledBitmap(bitmap, width, height, true).also { if (it !== bitmap) bitmap.recycle() }
        }
        return Image(sized, bounds.outWidth, bounds.outHeight)
    }
    override fun close() {
        if (closed) return
        closed = true
        for (request in waiting) request.cancelled = true
        waiting.clear()
        workers.shutdown()
        cache.evictAll()
    }
}
