package com.exact.android

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.graphics.Rect
import android.os.Handler
import android.os.Looper
import android.util.LruCache
import java.util.concurrent.Executors

/** Asset images decoded off the owner thread at their displayed pixel size.
 * Two pixel decodes and cancellable shared waiting demands bound fling work.
 * The byte-bounded cache keeps no invisible portion of a center-cover image.
 */
internal class NativeImages(private val context: Context) : AutoCloseable {
    class Image(val bitmap: Bitmap, val width: Int, val height: Int) {
        // The decode result, cache, queued delivery and each ImageView hold a
        // lease. Pixel retirement must not depend on Java text allocation/GC.
        private var owners = 1
        fun retain() {
            check(Looper.myLooper() == Looper.getMainLooper() && owners > 0)
            owners++
        }
        fun release() {
            check(Looper.myLooper() == Looper.getMainLooper() && owners > 0)
            if (--owners == 0) bitmap.recycle()
        }
    }
    internal class Request(val source: String, val width: Int, val height: Int, val fit: String,
        var deliver: ((Image) -> Unit)?) { var cancelled = false }
    private data class Key(val source: String, val width: Int, val height: Int, val fit: String)
    private val handler = Handler(Looper.getMainLooper())
    private val workers = Executors.newFixedThreadPool(2)
    private val requests = ImageRequests<Key, Request>(2)
    private val cache = object : LruCache<Key, Image>(32 * 1024 * 1024) {
        override fun sizeOf(key: Key, value: Image) = value.bitmap.allocationByteCount
        override fun entryRemoved(evicted: Boolean, key: Key, oldValue: Image, newValue: Image?) {
            oldValue.release()
        }
    }
    private var closed = false

    private fun Request.key() = Key(source, width, height, fit)
    fun request(source: String, width: Int, height: Int, fit: String, deliver: (Image) -> Unit): Request {
        check(!closed && Looper.myLooper() == Looper.getMainLooper())
        val request = Request(source, width.coerceAtLeast(1), height.coerceAtLeast(1), fit, deliver)
        val key = request.key()
        val cached = cache.get(key)
        if (cached != null) {
            cached.retain()
            handler.post { try { deliverImage(request, cached) } finally { cached.release() } }
        }
        else { requests.add(key, request); pump() }
        return request
    }
    private fun deliverImage(request: Request, image: Image) {
        val callback = request.deliver
        request.deliver = null
        if (!closed && !request.cancelled) callback?.invoke(image)
    }
    fun cancel(request: Request?) {
        if (request == null) return
        check(Looper.myLooper() == Looper.getMainLooper())
        request.cancelled = true
        request.deliver = null
        requests.cancel(request.key(), request)
    }
    private fun pump() {
        while (!closed) {
            val work = requests.next() ?: return
            workers.execute {
                // Only the immutable demand key is read by the decoder.
                // Observer sets, cancellation, completion and cache leases
                // are owned exclusively by the main Looper.
                val result = runCatching { decode(work.key) }
                handler.post {
                    val listeners = requests.complete(work)
                    val image = result.getOrNull()
                    try {
                        if (!closed && listeners.isNotEmpty()) {
                            val decoded = result.getOrThrow()
                            decoded.retain()
                            cache.put(work.key, decoded)
                            listeners.forEach { deliverImage(it, decoded) }
                        }
                    } finally { image?.release(); pump() }
                }
            }
        }
    }
    private fun decode(key: Key): Image {
        // BitmapFactory asset decoding uses the process default, which need
        // not equal an override/display Context's density. Read it through a
        // public Bitmap constructor so configuration/compat changes stay exact.
        val density = Bitmap.createBitmap(1, 1, Bitmap.Config.ALPHA_8).let { probe ->
            probe.density.also { probe.recycle() }
        }
        var naturalWidth = 0
        var naturalHeight = 0
        val bitmap = ImageDecoder.decodeBitmap(ImageDecoder.createSource(context.assets, key.source)) { decoder, info, _ ->
            naturalWidth = info.size.width
            naturalHeight = info.size.height
            val plan = ImageDecodePlan.create(naturalWidth, naturalHeight, key.width, key.height, key.fit)
            decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
            decoder.setTargetSize(plan.width, plan.height)
            // BitmapDrawable rounds density-scaled intrinsic dimensions.
            // Cropping before that rounding may change CENTER_CROP's scale;
            // the uncut decode preserves it on an overridden-density Context.
            if (plan.cropped && context.resources.displayMetrics.densityDpi == density) decoder.setCrop(Rect(plan.left, plan.top, plan.left + plan.cropWidth, plan.top + plan.cropHeight))
        }
        bitmap.density = density
        return Image(bitmap, naturalWidth, naturalHeight)
    }
    override fun close() {
        check(Looper.myLooper() == Looper.getMainLooper())
        if (closed) return
        closed = true
        requests.close().forEach { it.cancelled = true; it.deliver = null }
        workers.shutdown()
        cache.evictAll()
    }
}
