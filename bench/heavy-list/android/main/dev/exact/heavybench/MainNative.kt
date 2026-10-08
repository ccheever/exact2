package dev.exact.heavybench

import android.view.Surface

/** Public main native-window launcher, not the previous private Canvas command reader. */
internal object MainNative {
    init {
        android.system.Os.setenv("MIMALLOC_PURGE_DELAY", "0", true)
        android.system.Os.setenv("EXACT_FONTS", "/system/fonts", true)
        android.system.Os.setenv("EXACT_FONT", "Roboto", true)
        System.loadLibrary("exact_heavylist_main")
    }
    @JvmStatic external fun start(surface: Surface, width: Int, height: Int, scale: Float,
        assets: String, cache: String, live: Boolean): Long
    @JvmStatic external fun close(handle: Long)
    @JvmStatic external fun ready(handle: Long): Boolean
    @JvmStatic external fun frames(handle: Long): Long
    @JvmStatic external fun scroll(handle: Long, dy: Float)
    @JvmStatic external fun touch(handle: Long, action: Int, x: Float, y: Float)
}
