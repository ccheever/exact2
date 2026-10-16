package dev.exact.canvas

import android.graphics.fonts.Font
import java.io.File
import java.nio.IntBuffer

/** Where FONT_AXES (41: key index weight n (tag value)×n len path) at [i] ends. */
internal fun fontAxesEnd(b: IntBuffer, i: Int): Int { val p = i + 5 + 2 * b.get(i + 4); return p + 1 + (b.get(p) + 3) / 4 }

/** Where FONT (10: key index weight len path) or FONT_AXES (41) at [i] ends. */
internal fun fontEnd(b: IntBuffer, i: Int): Int = if (b.get(i) == 41) fontAxesEnd(b, i) else i + 5 + (b.get(i + 4) + 3) / 4

/** The face FONT (10) or FONT_AXES (41) at [i] announces, made from its file. 41 is 10 with the variation settings
 * fontique chose (ital 1 for Roboto's italic, wght, slnt, wdth) before the path; 'wght' is the weight unless it
 * sets one. The page's stream and a surface's ([Surfaces]) both announce faces this way. */
internal fun fontAt(b: IntBuffer, i: Int): Font {
    val index = b.get(i + 2); val weight = b.get(i + 3)
    val axes = if (b.get(i) == 41) b.get(i + 4) else 0
    val at = if (b.get(i) == 41) i + 5 + 2 * axes else i + 4
    val len = b.get(at)
    val bytes = ByteArray(len)
    for (k in 0 until len) bytes[k] = (b.get(at + 1 + k / 4) ushr (8 * (k % 4))).toByte()
    val settings = StringBuilder()
    var wght = false
    for (a in 0 until axes) {
        val t = b.get(i + 5 + 2 * a)
        val tag = String(charArrayOf((t ushr 24).toChar(), (t ushr 16 and 255).toChar(), (t ushr 8 and 255).toChar(), (t and 255).toChar()))
        if (tag == "wght") wght = true
        if (settings.isNotEmpty()) settings.append(", ")
        settings.append('\'').append(tag).append("' ").append(java.lang.Float.intBitsToFloat(b.get(i + 6 + 2 * a)))
    }
    if (!wght) settings.insert(0, if (axes > 0) "'wght' $weight, " else "'wght' $weight")
    val builder = Font.Builder(File(String(bytes))).setTtcIndex(index)
    try { builder.setFontVariationSettings(settings.toString()) } catch (e: Exception) {}
    return builder.setWeight(weight.coerceIn(1, 1000)).build()
}

/**
 * A stream's structure: what the window's own drawing (the view's display list) and each scroller's group node record,
 * with kept rows named by their node (a row recorded again in place keeps its predecessor's node), definitions left
 * out. When a new stream's top level is the last one's, the view is not drawn again: only the groups whose rows
 * changed are recorded again, and HWUI's damage is the nodes that changed (a row, a group), not the window.
 */
internal class Structure {
    private var words = IntArray(1024)
    private var size = 0
    /** Each group's id and its rows' range in the stream (after GROUP_BEGIN, up to GROUP_END). */
    val groups = ArrayList<IntArray>()
    /** Each group's signature, by id. */
    val groupSigs = HashMap<Int, IntArray>()
    /** Something the structure cannot name stably (a platform element): never retained. */
    var opaque = false

    private fun put(v: Int) {
        if (size == words.size) words = words.copyOf(size * 2)
        words[size++] = v
    }

    private fun copy(b: IntBuffer, from: Int, to: Int) { for (k in from until to) put(b.get(k)) }

    private fun skipPath(b: IntBuffer, at: Int): Int {
        var i = at + 1
        repeat(b.get(at)) { i += when (b.get(i)) { 0, 1 -> 3; 2 -> 7; else -> 1 } }
        return i
    }

    /** Where the op at [i] ends. */
    fun end(b: IntBuffer, i: Int): Int = when (b.get(i)) {
        1 -> i + 7; 2 -> i + 14; 3 -> skipPath(b, i + 3); 4 -> i + 13; 5 -> skipPath(b, i + 2); 6 -> i + 1; 7 -> i + 2
        8 -> i + 6; 9 -> i + 6 + 3 * b.get(i + 5); 10 -> i + 5 + (b.get(i + 4) + 3) / 4; 11 -> i + 4; 12 -> i + 2
        13 -> skipPath(b, i + 5); 14 -> i + 18; 15 -> i + 6 + b.get(i + 5); 16 -> i + 1; 17 -> i + 4; 18 -> i + 2
        19 -> i + 26; 20 -> i + 2; 21 -> i + 1; 23 -> i + 2; 24, 30, 31 -> i + 3 + b.get(i + 2); 25 -> i + 3 + b.get(i + 2); 43 -> i + 2 + b.get(i + 1); 44 -> i + 2
        26 -> i + 3 + (b.get(i + 2) + 3) / 4; 27 -> i + 14; 42 -> i + 14 + 2 * b.get(i + 1); 28 -> i + 16 + (b.get(i + 15) + 3) / 4; 29 -> i + 2
        32 -> i + 2; 33 -> i + 3; 40 -> i + 27; 41 -> fontAxesEnd(b, i)
        else -> throw IllegalStateException("op ${b.get(i)} at $i")
    }

    /** [b]'s top-level signature (returned) and its groups' ([groups], [groupSigs]); [token] names a row's node. */
    fun read(b: IntBuffer, token: (Int) -> Int): IntArray {
        groups.clear(); groupSigs.clear(); opaque = false
        size = 0
        var i = 0
        val n = b.limit()
        var group = -1; var groupFrom = 0; var outer = 0
        while (i < n) {
            val op = b.get(i)
            val j = end(b, i)
            when (op) {
                // Definitions: the reader's kept things, not this drawing.
                10, 11, 12, 15, 18, 24, 26, 30, 31, 32, 33, 41 -> {}
                17 -> { put(17); put(token(b.get(i + 1))); put(b.get(i + 2)); put(b.get(i + 3)) }
                20 -> {
                    put(20); put(b.get(i + 1))
                    group = b.get(i + 1); groupFrom = j; outer = size
                }
                21 -> {
                    if (group >= 0) {
                        groupSigs[group] = words.copyOfRange(outer, size)
                        groups.add(intArrayOf(group, groupFrom, i))
                        size = outer
                        group = -1
                    }
                    put(21)
                }
                28 -> { opaque = true; copy(b, i, j) }
                else -> copy(b, i, j)
            }
            i = j
        }
        return words.copyOf(size)
    }
}
