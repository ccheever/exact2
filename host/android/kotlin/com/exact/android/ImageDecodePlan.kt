package com.exact.android

import kotlin.math.ceil
import kotlin.math.max
import kotlin.math.min

/** Pixels the system decoder must keep, distinct from an image's natural size.
 * Cover may discard only the centered pixels its ImageView would clip anyway.
 * Small sources are never enlarged merely to fill the decoded-image cache.
 */
internal data class ImageDecodePlan(
    val width: Int, val height: Int,
    val left: Int = 0, val top: Int = 0, val cropWidth: Int = width, val cropHeight: Int = height
) {
    val cropped: Boolean get() = left != 0 || top != 0 || cropWidth != width || cropHeight != height
    val outputPixels: Long get() = cropWidth.toLong() * cropHeight

    companion object {
        fun create(naturalWidth: Int, naturalHeight: Int, offeredWidth: Int, offeredHeight: Int, fit: String): ImageDecodePlan {
            require(naturalWidth > 0 && naturalHeight > 0 && offeredWidth > 0 && offeredHeight > 0) { "invalid Android image dimensions" }
            val x = offeredWidth.toDouble() / naturalWidth
            val y = offeredHeight.toDouble() / naturalHeight
            val scale = when (fit) {
                "cover", "fill" -> max(x, y)
                "contain", "scale-down" -> min(x, y)
                "none" -> 1.0
                else -> error("Android object-fit '$fit' is invalid")
            }
            val decodeScale = min(1.0, scale)
            fun pixels(axis: Int) = ceil(axis * decodeScale).toLong().coerceIn(1L, axis.toLong()).toInt()
            val width = pixels(naturalWidth)
            val height = pixels(naturalHeight)
            if (fit != "cover") return ImageDecodePlan(width, height)
            // Use ImageView's scale of the actual rounded decode. One axis
            // stays whole, so its final CENTER_CROP scale cannot change. Keep
            // the crop's parity equal to the source: at most one extra pixel
            // makes its center exact even for odd widths and enlarged sources.
            val displayScale = max(offeredWidth.toDouble() / width, offeredHeight.toDouble() / height)
            fun centered(axis: Int, offer: Int): Int {
                val cropped = ceil(offer / displayScale).toLong().coerceIn(1L, axis.toLong()).toInt()
                return cropped + (axis - cropped) % 2
            }
            val cropWidth = centered(width, offeredWidth)
            val cropHeight = centered(height, offeredHeight)
            return ImageDecodePlan(width, height, (width - cropWidth) / 2, (height - cropHeight) / 2, cropWidth, cropHeight)
        }
    }
}
