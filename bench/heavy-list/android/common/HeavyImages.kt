package dev.exact.heavybench

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.graphics.Rect
import android.os.Handler
import android.os.Looper
import android.util.LruCache
import org.json.JSONObject
import java.nio.ByteBuffer
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlin.math.ceil
import kotlin.math.max

/** Same image executor and byte-bounded cache in both references; no full-resolution UI-thread decode. */
class HeavyImageLoader(context: Context) {
    private val assets = context.applicationContext.assets
    private val main = Handler(Looper.getMainLooper())
    private val lock = Any()
    private val cache = object : LruCache<String, Bitmap>(32 * 1024 * 1024) {
        override fun sizeOf(key: String, bitmap: Bitmap): Int = bitmap.allocationByteCount
    }
    private val executor = ThreadPoolExecutor(2, 2, 0, TimeUnit.MILLISECONDS, ArrayBlockingQueue(128))
    private val pending = HashMap<String, Work>()
    private val decoded = AtomicInteger()
    private val failures = AtomicInteger()
    private var closed = false
    private class Listener(val callback: (Bitmap) -> Unit) { @Volatile var active = true }

    private inner class Work(val key: String, val file: String, val width: Int, val height: Int) : Runnable {
        val listeners = LinkedHashSet<Listener>()
        var running = false
        val retry = Runnable { synchronized(lock) { if (!closed && listeners.isNotEmpty() && pending[key] === this) submit(this) } }
        override fun run() {
            synchronized(lock) { if (closed || listeners.isEmpty()) return; running = true }
            val bitmap = try {
                val bytes = assets.open("images/$file").use { it.readBytes() }
                ImageDecoder.decodeBitmap(ImageDecoder.createSource(ByteBuffer.wrap(bytes))) { decoder, info, _ ->
                    val scale = max(width.toDouble() / info.size.width, height.toDouble() / info.size.height)
                    val decodedWidth = ceil(info.size.width * scale).toInt().coerceAtLeast(width)
                    val decodedHeight = ceil(info.size.height * scale).toInt().coerceAtLeast(height)
                    decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                    decoder.setTargetSize(decodedWidth, decodedHeight)
                    val left = (decodedWidth - width) / 2
                    val top = (decodedHeight - height) / 2
                    decoder.setCrop(Rect(left, top, left + width, top + height))
                }.also { decoded.incrementAndGet() }
            } catch (_: Exception) { failures.incrementAndGet(); null }
            val delivery = synchronized(lock) {
                if (bitmap != null && !closed) cache.put(key, bitmap)
                if (pending[key] === this) pending.remove(key)
                listeners.toList().also { listeners.clear() }
            }
            if (bitmap != null) main.post { delivery.forEach { if (it.active && !closed) it.callback(bitmap) } }
        }
    }

    private fun submit(work: Work) {
        try { executor.execute(work) }
        catch (_: RejectedExecutionException) {
            // A visible request is retried; it is never permanently replaced with a placeholder.
            if (!closed && work.listeners.isNotEmpty()) main.postDelayed(work.retry, 16)
        }
    }
    fun request(file: String, widthPx: Int, heightPx: Int, onReady: (Bitmap) -> Unit): () -> Unit {
        require(widthPx > 0 && heightPx > 0)
        val key = "$file|$widthPx|$heightPx"
        val listener = Listener(onReady)
        synchronized(lock) {
            if (closed) return { }
            val hit = cache.get(key)
            if (hit != null) { onReady(hit); return { listener.active = false } }
            val work = pending[key]
            if (work != null) work.listeners.add(listener)
            else Work(key, file, widthPx, heightPx).let { it.listeners.add(listener); pending[key] = it; submit(it) }
        }
        return {
            listener.active = false
            synchronized(lock) {
                pending[key]?.let { work ->
                    work.listeners.remove(listener)
                    if (work.listeners.isEmpty() && !work.running) {
                        main.removeCallbacks(work.retry); executor.remove(work); pending.remove(key)
                    }
                }
            }
        }
    }
    fun diagnosticSnapshot(): JSONObject = synchronized(lock) {
        JSONObject().put("image_decodes", decoded.get()).put("image_failures", failures.get())
            .put("image_cache_bytes", cache.size()).put("image_pending", pending.size)
            .put("image_cache_budget_bytes", 32 * 1024 * 1024).put("image_workers", 2)
    }
    fun close() {
        synchronized(lock) {
            closed = true
            pending.values.forEach { work -> work.listeners.forEach { it.active = false }; main.removeCallbacks(work.retry) }
            pending.clear(); cache.evictAll(); executor.shutdownNow()
        }
    }
}
