package com.exact.android

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.BitmapDrawable
import android.widget.ImageView

/** Keep SDK image drawing, including software snapshots of a hardware window. */
internal class NativeImageView(context: Context) : ImageView(context) {
    private var swappingForSnapshot = false
    override fun invalidate() { if (!swappingForSnapshot) super.invalidate() }
    override fun requestLayout() { if (!swappingForSnapshot) super.requestLayout() }
    override fun onDraw(canvas: Canvas) {
        // getDrawable() gives up ImageView's reusable BitmapDrawable. Keep
        // ordinary hardware draws on the unchanged SDK path before that read.
        if (canvas.isHardwareAccelerated) { super.onDraw(canvas); return }
        val originalDrawable = drawable as? BitmapDrawable
        val original = originalDrawable?.bitmap
        if (original?.config != Bitmap.Config.HARDWARE) {
            super.onDraw(canvas)
            return
        }
        // A hardware window can still be drawn into a software Canvas by its
        // embedder. Read back only for that draw; retain no second pixel cache.
        val format = if (original.colorSpace?.isWideGamut == true) Bitmap.Config.RGBA_F16 else Bitmap.Config.ARGB_8888
        val software = checkNotNull(original.copy(format, false)) { "Cannot snapshot hardware image" }
        swappingForSnapshot = true
        try {
            super.setImageBitmap(software)
            super.onDraw(canvas)
        } finally {
            try {
                // ImageView reuses and mutates its BitmapDrawable. Restoring
                // only the saved Drawable would leave it holding the copy.
                super.setImageBitmap(original)
                if (drawable !== originalDrawable) super.setImageDrawable(originalDrawable)
            } finally { swappingForSnapshot = false; software.recycle() }
        }
    }
}
