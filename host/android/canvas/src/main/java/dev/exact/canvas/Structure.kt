package dev.exact.canvas

import java.nio.IntBuffer

/** Where FONT_AXES (41: key index weight n (tag value)×n len path) at [i] ends. */
internal fun fontAxesEnd(b: IntBuffer, i: Int): Int { val p = i + 5 + 2 * b.get(i + 4); return p + 1 + (b.get(p) + 3) / 4 }

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
        19 -> i + 26; 20 -> i + 2; 21 -> i + 1; 23 -> i + 2; 24, 30, 31 -> i + 3 + b.get(i + 2); 25 -> i + 3 + b.get(i + 2)
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
