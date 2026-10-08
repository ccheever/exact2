package com.exact.android

internal object ImageDecodePlanTest {
    fun run(): String {
        fun plan(w: Int, h: Int, boxW: Int, boxH: Int, fit: String) = ImageDecodePlan.create(w, h, boxW, boxH, fit)
        check(plan(1600, 900, 450, 450, "cover") == ImageDecodePlan(800, 450, 175, 0, 450, 450))
        check(plan(900, 1600, 450, 450, "cover") == ImageDecodePlan(450, 800, 0, 175, 450, 450))
        check(plan(1600, 900, 450, 450, "contain") == ImageDecodePlan(450, 254))
        check(plan(1600, 900, 450, 450, "fill") == ImageDecodePlan(800, 450))
        check(plan(900, 1600, 450, 450, "none") == ImageDecodePlan(900, 1600))
        check(plan(600, 400, 900, 400, "cover") == ImageDecodePlan(600, 400, 0, 66, 600, 268))
        check(plan(100, 50, 200, 200, "contain") == ImageDecodePlan(100, 50))
        check(plan(100, 50, 200, 200, "scale-down") == ImageDecodePlan(100, 50))
        // Real decoder constraints: source-aligned dimensions, safe crop
        // bounds, centered clipping and unchanged natural-resolution ceiling.
        var cases = 0
        for (nw in listOf(1, 2, 31, 600, 900, 1600, Int.MAX_VALUE))
            for (nh in listOf(1, 7, 400, 1600, Int.MAX_VALUE))
                for (w in listOf(1, 113, 450, 912, Int.MAX_VALUE))
                    for (h in listOf(1, 113, 394, 900, Int.MAX_VALUE))
                        for (fit in listOf("cover", "contain", "fill", "none", "scale-down")) {
                            val p = plan(nw, nh, w, h, fit)
                            check(p.width in 1..nw && p.height in 1..nh)
                            check(p.cropWidth in 1..p.width && p.cropHeight in 1..p.height)
                            check(p.left >= 0 && p.top >= 0 && p.left.toLong() + p.cropWidth <= p.width && p.top.toLong() + p.cropHeight <= p.height)
                            check(kotlin.math.abs((p.width.toLong() - p.cropWidth) - 2L * p.left) <= 1L)
                            check(kotlin.math.abs((p.height.toLong() - p.cropHeight) - 2L * p.top) <= 1L)
                            check(p.outputPixels > 0 && p.outputPixels <= nw.toLong() * nh)
                            if (fit == "cover") {
                                check(2L * p.left + p.cropWidth == p.width.toLong())
                                check(2L * p.top + p.cropHeight == p.height.toLong())
                                val originalScale = maxOf(w.toDouble() / p.width, h.toDouble() / p.height)
                                val croppedScale = maxOf(w.toDouble() / p.cropWidth, h.toDouble() / p.cropHeight)
                                check(kotlin.math.abs(originalScale - croppedScale) <= originalScale * 1e-9)
                            } else check(!p.cropped)
                            if (fit == "none") check(p.width == nw && p.height == nh)
                            cases++
                        }
        check(runCatching { plan(0, 40, 40, 40, "cover") }.isFailure)
        check(runCatching { plan(40, 40, 40, 0, "contain") }.isFailure)
        check(runCatching { plan(40, 40, 40, 40, "invalid") }.isFailure)
        return "ImageDecodePlanTest: PASS ($cases shape/fit/overflow cases; landscape/portrait crop, contain/fill/none, no source upscaling)"
    }
    @JvmStatic fun main(args: Array<String>) { println(run()) }
}
