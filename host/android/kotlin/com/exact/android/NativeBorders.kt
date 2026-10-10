package com.exact.android

import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path

/** CSS solid borders: one retained ring, partitioned at the outer/inner corner joins.
 * Geometry is rebuilt on a style or size change; colour changes reuse the paths.
 * Side order matches the kernel: top, right, bottom, left.
 */
internal class NativeBorders {
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val ring = Path()
    private var sides: Array<Path>? = null
    private var partitionsDirty = true
    private val inner = FloatArray(8)
    private var dirty = true
    private var occupied = false

    fun invalidate() { dirty = true }

    fun draw(canvas: Canvas, width: Float, height: Float, widths: FloatArray, colors: IntArray, radii: FloatArray) {
        if (dirty) {
            rebuild(width, height, widths, radii)
            dirty = false
        }
        if (!occupied) return
        val first = (0..3).first { widths[it] > 0f }
        if ((0..3).all { widths[it] <= 0f || colors[it] == colors[first] }) {
            paint.color = colors[first]
            canvas.drawPath(ring, paint)
        } else {
            val partitions = partitions(width, height, widths)
            for (side in 0..3) {
                if (widths[side] <= 0f) continue
                paint.color = colors[side]
                canvas.drawPath(partitions[side], paint)
            }
        }
    }

    private fun rebuild(width: Float, height: Float, widths: FloatArray, radii: FloatArray) {
        ring.reset()
        partitionsDirty = true
        occupied = width > 0f && height > 0f && widths.any { it > 0f }
        if (!occupied) return
        val top = widths[0].coerceIn(0f, height)
        val right = widths[1].coerceIn(0f, width)
        val bottom = widths[2].coerceIn(0f, height)
        val left = widths[3].coerceIn(0f, width)
        val x1 = left
        val y1 = top
        val x2 = (width - right).coerceAtLeast(x1)
        val y2 = (height - bottom).coerceAtLeast(y1)
        // A square single-sided border is a rectangle, not a ring with a hole.
        if (radii.all { it == 0f } && widths.count { it > 0f } == 1) {
            when (widths.indexOfFirst { it > 0f }) {
                0 -> ring.addRect(0f, 0f, width, top, Path.Direction.CW)
                1 -> ring.addRect(x2, 0f, width, height, Path.Direction.CW)
                2 -> ring.addRect(0f, y2, width, height, Path.Direction.CW)
                3 -> ring.addRect(0f, 0f, x1, height, Path.Direction.CW)
            }
            return
        }
        ring.fillType = Path.FillType.EVEN_ODD
        ring.addRoundRect(0f, 0f, width, height, radii, Path.Direction.CW)
        if (x2 > x1 && y2 > y1) {
            inner[0] = (radii[0] - left).coerceAtLeast(0f)
            inner[1] = (radii[1] - top).coerceAtLeast(0f)
            inner[2] = (radii[2] - right).coerceAtLeast(0f)
            inner[3] = (radii[3] - top).coerceAtLeast(0f)
            inner[4] = (radii[4] - right).coerceAtLeast(0f)
            inner[5] = (radii[5] - bottom).coerceAtLeast(0f)
            inner[6] = (radii[6] - left).coerceAtLeast(0f)
            inner[7] = (radii[7] - bottom).coerceAtLeast(0f)
            ring.addRoundRect(x1, y1, x2, y2, inner, Path.Direction.CW)
        }
    }

    private fun partitions(width: Float, height: Float, widths: FloatArray): Array<Path> {
        val paths = sides ?: Array(4) { Path() }.also { sides = it }
        if (!partitionsDirty) return paths
        for (path in paths) path.reset()
        val x1 = widths[3].coerceIn(0f, width)
        val y1 = widths[0].coerceIn(0f, height)
        val x2 = (width - widths[1].coerceIn(0f, width)).coerceAtLeast(x1)
        val y2 = (height - widths[2].coerceIn(0f, height)).coerceAtLeast(y1)
        polygon(paths[0], floatArrayOf(0f, 0f, width, 0f, x2, y1, x1, y1))
        polygon(paths[1], floatArrayOf(width, 0f, width, height, x2, y2, x2, y1))
        polygon(paths[2], floatArrayOf(width, height, 0f, height, x1, y2, x2, y2))
        polygon(paths[3], floatArrayOf(0f, height, 0f, 0f, x1, y1, x1, y2))
        for (path in paths) path.op(ring, Path.Op.INTERSECT)
        partitionsDirty = false
        return paths
    }

    private fun polygon(path: Path, points: FloatArray) {
        path.moveTo(points[0], points[1])
        for (i in 2 until points.size step 2) path.lineTo(points[i], points[i + 1])
        path.close()
    }
}
