package com.exact.android

import android.content.res.AssetManager
import android.graphics.Bitmap
import android.graphics.ColorSpace
import android.hardware.HardwareBuffer
import android.os.Build

/** Optional public NDK decode destination; ImageView still paints the SDK Bitmap. */
internal object NativeImageDecode {
    init { System.loadLibrary("exact_image_decode") }
    @JvmStatic private external fun decode(assets: AssetManager, source: String, width: Int, height: Int,
        fit: String, cropAllowed: Boolean, natural: IntArray): HardwareBuffer?
    @JvmStatic fun plan(width: Int, height: Int, offeredWidth: Int, offeredHeight: Int,
        fit: String, cropAllowed: Boolean): IntArray {
        val p = ImageDecodePlan.create(width, height, offeredWidth, offeredHeight, fit)
        return if (cropAllowed) intArrayOf(p.width, p.height, p.left, p.top, p.cropWidth, p.cropHeight)
            else intArrayOf(p.width, p.height, 0, 0, p.width, p.height)
    }
    fun decode(assets: AssetManager, source: String, width: Int, height: Int, fit: String,
        cropAllowed: Boolean, density: Int): NativeImages.Image? {
        if (Build.VERSION.SDK_INT < 30) return null
        val natural = IntArray(2)
        val buffer = decode(assets, source, width, height, fit, cropAllowed, natural) ?: return null
        val bitmap = try { Bitmap.wrapHardwareBuffer(buffer, ColorSpace.get(ColorSpace.Named.SRGB)) }
            finally { buffer.close() }
        if (bitmap == null) return null
        bitmap.density = density
        return NativeImages.Image(bitmap, natural[0], natural[1])
    }
}
