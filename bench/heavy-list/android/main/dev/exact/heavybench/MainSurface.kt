package dev.exact.heavybench

import android.content.Context
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import java.io.File

/** Thin carrier of the public main Handle. Rust owns choreographer, input and painting. */
internal class MainSurface(context: Context, private val live: Boolean) : SurfaceView(context), SurfaceHolder.Callback {
    private var handle = 0L
    private var size = 0 to 0
    private val assetsDirectory by lazy { installAssets() }
    init { holder.addCallback(this); isFocusable = true }
    val ready: Boolean get() = handle != 0L && MainNative.ready(handle)
    val submittedFrames: Long get() = if (handle == 0L) 0 else MainNative.frames(handle)
    fun scrollByPixels(dy: Float) { if (handle != 0L) MainNative.scroll(handle, dy) }
    override fun surfaceCreated(holder: SurfaceHolder) = Unit
    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (width <= 0 || height <= 0 || size == (width to height) && handle != 0L) return
        // Public Handle has no resize command: a real surface size change owns a new session.
        close()
        size = width to height
        handle = MainNative.start(holder.surface, width, height, resources.displayMetrics.density,
            assetsDirectory.absolutePath, File(context.cacheDir, "heavy-main").apply { mkdirs() }.absolutePath, live)
    }
    override fun surfaceDestroyed(holder: SurfaceHolder) { close() }
    override fun onTouchEvent(event: MotionEvent): Boolean {
        val action = when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> 0; MotionEvent.ACTION_UP -> 1
            MotionEvent.ACTION_MOVE -> 2; MotionEvent.ACTION_CANCEL -> 3; else -> return false
        }
        if (handle != 0L) MainNative.touch(handle, action, event.x, event.y)
        return true
    }
    fun close() {
        if (handle != 0L) { val previous = handle; handle = 0; MainNative.close(previous) }
        size = 0 to 0
    }
    private fun installAssets(): File {
        val directory = File(context.filesDir, "heavy-main-assets").apply { mkdirs() }
        val stamp = context.assets.open("benchmark-assets.sha256").bufferedReader().use { it.readText() }
        val prior = File(directory, ".dataset")
        if (!prior.isFile || prior.readText() != stamp) {
            directory.listFiles()?.forEach { it.deleteRecursively() }
            val images = File(directory, "assets").apply { mkdirs() }
            for (name in checkNotNull(context.assets.list("assets"))) if (name.endsWith(".jpg")) {
                context.assets.open("assets/$name").use { input -> File(images, name).outputStream().use { input.copyTo(it) } }
            }
            prior.writeText(stamp)
        }
        return directory
    }
}
