package com.exact.android

import android.content.Context
import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import android.util.LruCache

/** Real cached requests/leases; pixel/decoder parity stays in the existing SDK corpus. */
internal object NativeImageReuseTest {
    private const val SOURCE = "image-reuse-seeded-owner"
    private data class Case(val name: String, val w: Int = 160, val h: Int = 120,
        val naturalW: Int = 320, val naturalH: Int = 240, val offerW: Int = w, val offerH: Int = h,
        val source: String = SOURCE, val fit: String = "cover", val hardware: Boolean = false,
        val densityDelta: Int = 0, val requestW: Int = 101, val requestH: Int = 73)
    private val cases = listOf(Case("least/MRU"), Case("source", source = "another-source"),
        Case("fit", fit = "contain"), Case("capability", hardware = true), Case("density", densityDelta = 1),
        Case("insufficient", w = 80, h = 60), Case("actual-full-plan", offerW = 80, offerH = 60),
        Case("prior-crop", offerW = 160, offerH = 100), Case("actual-crop", h = 100),
        Case("rounded-aspect", w = 401, h = 301, naturalW = 1000, naturalH = 750,
            offerW = 401, offerH = 300, requestW = 200, requestH = 150))
    private val keyType = Class.forName("com.exact.android.NativeImages\$Key")
    private val constructor = keyType.getDeclaredConstructor(String::class.java, Integer.TYPE,
        Integer.TYPE, String::class.java, java.lang.Boolean.TYPE).apply { isAccessible = true }
    private fun key(c: Case): Any = constructor.newInstance(c.source, c.offerW, c.offerH, c.fit, c.hardware)
    @Suppress("UNCHECKED_CAST")
    private fun cache(images: NativeImages): LruCache<Any, NativeImages.Image> =
        NativeImages::class.java.getDeclaredField("cache").apply { isAccessible = true }.get(images) as LruCache<Any, NativeImages.Image>
    private fun seed(context: Context, images: NativeImages, c: Case): NativeImages.Image {
        val bitmap = Bitmap.createBitmap(c.w, c.h, Bitmap.Config.ARGB_8888)
        bitmap.density = context.resources.displayMetrics.densityDpi + c.densityDelta
        return NativeImages.Image(bitmap, c.naturalW, c.naturalH).also { cache(images).put(key(c), it) }
    }
    fun runAsync(context: Context, done: (String) -> Unit) {
        check(Looper.myLooper() == Looper.getMainLooper())
        val main = Handler(Looper.getMainLooper())
        var active: NativeImages? = null; var retained: NativeImages.Image? = null
        var ended = false; var stage = 0; var calls = 0; var leases = 0
        fun finish(error: Throwable?) {
            if (ended) return
            ended = true; main.removeCallbacksAndMessages(null)
            var failure = error
            try { active?.close(); retained?.release() }
            catch (caught: Throwable) { if (failure == null) failure = caught else failure.addSuppressed(caught) }
            done(if (failure == null) "NativeImageReuseTest: PASS (9 rejected smaller owners;least allocation;actual MRU promotion;10 shared real requests;no aliases;12 lease checks)"
                else "NativeImageReuseTest: FAIL (stage=$stage ${failure.stackTraceToString()})")
        }
        fun guard(body: () -> Unit) { if (!ended) try { body() } catch (error: Throwable) { finish(error) } }
        lateinit var next: () -> Unit
        fun cancelled(close: Boolean, complete: () -> Unit) {
            val images = NativeImages(context); active = images
            val image = seed(context, images, Case("queued"))
            var called = false
            val request = images.request(SOURCE, 101, 73, "cover") { called = true }
            if (close) images.close() else { images.cancel(request); images.trim() }
            check(!image.bitmap.isRecycled) { "Queued lease retired before dispatch" }
            main.post { guard {
                check(!called && image.bitmap.isRecycled) { "Cancelled/closed cache hit delivered or leaked" }
                images.close(); active = null; leases++; complete()
            } }
        }
        next = { guard {
            if (stage == cases.size) {
                check(calls == 10 && leases == 12)
                finish(null)
            } else {
                val c = cases[stage]
                val images = NativeImages(context); active = images
                val largeCase = Case("valid", w = if (c.naturalW == 1000) 500 else 320,
                    h = if (c.naturalW == 1000) 375 else 240, naturalW = c.naturalW, naturalH = c.naturalH)
                val large = seed(context, images, largeCase)
                val small = seed(context, images, c)
                val mediumCase = Case("medium", w = 240, h = 180)
                val medium = if (stage == 0) seed(context, images, mediumCase) else null
                val expected = if (stage == 0) small else large
                val before = cache(images).snapshot().keys.toSet()
                check(before.size == (if (medium == null) 2 else 3))
                check(cache(images).snapshot().keys.last() == key(if (medium == null) c else mediumCase))
                images.request(SOURCE, c.requestW, c.requestH, "cover") { delivered -> guard {
                    check(delivered === expected) { "Wrong shared source owner: ${c.name}" }; calls++
                    check(cache(images).snapshot().keys.toSet() == before) { "Hit added an alias/double charge" }
                    if (stage == 0) check(cache(images).snapshot().keys.last() == key(c)) { "Owner key was not promoted MRU" }
                    delivered.retain(); retained = delivered
                    images.trim(); images.close(); active = null
                    check(!delivered.bitmap.isRecycled) { "Eviction retired a retained delivery owner" }
                    check(listOfNotNull(small, large, medium).filter { it !== expected }.all { it.bitmap.isRecycled })
                    // This executes AFTER NativeImages releases its queued-delivery lease in finally.
                    main.post { guard {
                        check(!delivered.bitmap.isRecycled)
                        delivered.release(); retained = null
                        check(delivered.bitmap.isRecycled) { "Final delivery owner leaked" }
                        leases++; stage++; next()
                    } }
                } }
            }
        } }
        main.postDelayed({ finish(IllegalStateException("Image reuse callback timed out")) }, 5000)
        guard { cancelled(false) { cancelled(true) { next() } } }
    }
}
