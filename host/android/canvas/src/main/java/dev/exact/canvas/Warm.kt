package dev.exact.canvas

import android.graphics.HardwareRenderer
import android.graphics.PixelFormat
import android.graphics.RenderNode
import android.hardware.HardwareBuffer
import android.media.ImageReader
import android.os.Handler

/**
 * A press wakes the GPU, as a platform press's ripple does: one pixel drawn offscreen (a renderer of its own over a 1x1
 * image, on the RenderThread), so the release's response, ~50-100 ms on, finds the GPU awake. After ~50 ms idle the
 * first frame's GPU work takes 7-10 ms more on the Nothing Phone (4a) Pro (the GPU wakes and clocks up), and a
 * response frame that pays that misses its deadline. Nothing of the window is drawn: no frame of the app's, no damage.
 */
internal class Warm {
    private var renderer: HardwareRenderer? = null
    private var reader: ImageReader? = null
    private val node = RenderNode("exact warm").apply { setPosition(0, 0, 1, 1) }
    private var shade = 0

    /** Its images are let go here: taking one waits for its fence, the GPU's work (not on the main thread). */
    private var drain: android.os.HandlerThread? = null

    /** Made before the first press (after first content): the renderer and its first frame cost ~15 ms. */
    fun prepare() { if (renderer == null) poke() }

    fun poke() {
        try {
            val r = renderer ?: run {
                val ir = ImageReader.newInstance(1, 1, PixelFormat.RGBA_8888, 2,
                    HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_GPU_SAMPLED_IMAGE)
                val d = android.os.HandlerThread("exact warm").apply { start() }.also { drain = it }
                ir.setOnImageAvailableListener({ it.acquireLatestImage()?.close() }, Handler(d.looper))
                reader = ir
                HardwareRenderer().apply { setSurface(ir.surface); setContentRoot(node) }.also { renderer = it }
            }
            // A new drawing each time: an unchanged one is no frame.
            val c = node.beginRecording()
            c.drawColor(if (++shade % 2 == 0) 0xff000000.toInt() else 0xffffffff.toInt())
            node.endRecording()
            android.os.Trace.beginSection("exact warm")
            r.createRenderRequest().syncAndDraw()
            android.os.Trace.endSection()
        } catch (e: Exception) {
            android.util.Log.w("exact", "warm: $e")
        }
    }

    fun destroy() {
        renderer?.destroy(); renderer = null
        reader?.close(); reader = null
        drain?.quitSafely(); drain = null
    }
}
