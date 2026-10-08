package com.exact.android

/** Paint-only values: font metrics remain the kernel's measured run protocol. */
internal data class InlinePaintColor(val light: Int, val dark: Int = light) {
    fun resolve(darkScheme: Boolean) = if (darkScheme) dark else light
}

internal data class InlinePaintPiece(
    val text: String, val color: InlinePaintColor?, val backgrounds: List<InlinePaintColor>,
    val decoration: Int, val hidden: Boolean
)

internal data class InlinePaintRange(
    val start: Int, val end: Int, val color: Int?, val background: Int?, val decoration: Int
)

internal data class InlinePaintRanges(val text: String, val ranges: List<InlinePaintRange>)

internal data class InlinePaintModel(val pieces: List<InlinePaintPiece>) {
    fun ranges(whiteSpace: Int, dark: Boolean, ownerDecoration: Int): InlinePaintRanges {
        val texts = collapse(pieces.map { it.text }, whiteSpace)
        val joined = StringBuilder()
        val ranges = ArrayList<InlinePaintRange>(pieces.size)
        for ((index, piece) in pieces.withIndex()) {
            val start = joined.length
            joined.append(texts[index])
            if (start == joined.length) continue
            var background: Int? = null
            if (!piece.hidden) for (value in piece.backgrounds) {
                val next = value.resolve(dark)
                if (next ushr 24 != 0) background = over(next, background ?: 0)
            }
            ranges.add(InlinePaintRange(start, joined.length,
                if (piece.hidden) 0 else piece.color?.resolve(dark),
                if (piece.hidden) 0 else background,
                if (piece.hidden) 0 else piece.decoration or ownerDecoration))
        }
        return InlinePaintRanges(joined.toString(), ranges)
    }

    companion object {
        fun decoration(value: String): Int = when (value.trim()) {
            "", "none" -> 0
            "underline" -> 1
            "line-through" -> 2
            "underline line-through", "line-through underline" -> 3
            else -> error("Android text decoration '$value' is not implemented")
        }

        // CSS Text 3 preparation matches exact_textflow::collapse: only ASCII
        // space/tab/CR/LF collapse. Keep the first space in its original run;
        // UTF-16 span offsets are then computed from the exact shaped strings.
        fun collapse(runs: List<String>, whiteSpace: Int): List<String> {
            require(whiteSpace in 0..4)
            if (whiteSpace == 1 || whiteSpace == 4) return runs
            val output = runs.map { StringBuilder(it.length) }
            val pending = ArrayList<Pair<Int, Char>>()
            var previous: Char? = null
            fun flush(next: Char?) {
                val breaks = pending.any { it.second == '\n' }
                if (whiteSpace == 3 && breaks) {
                    for ((run, ch) in pending) if (ch == '\n') output[run].append(ch)
                } else if (next != null && previous != null &&
                    !(breaks && (previous == '\u200b' || next == '\u200b'))) {
                    output[pending.first().first].append(' ')
                }
                pending.clear()
            }
            for ((index, run) in runs.withIndex()) for (ch in run) {
                if (ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r') pending.add(index to ch)
                else {
                    if (pending.isNotEmpty()) flush(ch)
                    output[index].append(ch)
                    previous = ch
                }
            }
            if (pending.isNotEmpty()) flush(null)
            return output.map { it.toString() }
        }

        // Inline ancestor backgrounds paint behind descendant fragments. A
        // transparent child cannot erase its parent's fill; partial alpha
        // composes over it before the platform BackgroundColorSpan is installed.
        private fun over(front: Int, back: Int): Int {
            val fa = front ushr 24
            val ba = back ushr 24
            val alpha = fa * 255 + ba * (255 - fa)
            if (alpha == 0) return 0
            fun channel(shift: Int): Int = (((front ushr shift and 255) * fa * 255 +
                (back ushr shift and 255) * ba * (255 - fa) + alpha / 2) / alpha)
            return ((alpha + 127) / 255 shl 24) or (channel(16) shl 16) or
                (channel(8) shl 8) or channel(0)
        }
    }
}
