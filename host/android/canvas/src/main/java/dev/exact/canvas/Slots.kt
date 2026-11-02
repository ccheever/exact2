package dev.exact.canvas

import android.graphics.RenderNode
import java.nio.IntBuffer

/**
 * Each picture's slot (ops 23 and 24): a node the rows drawing an image view draw by reference, so a picture arriving
 * records the slot and not the rows. exact2 sends a slot's drawing only when it differs from the last it sent, while HWUI
 * discards the drawing of a node that leaves its tree: a row taking over a slot from rows freed before it is drawn
 * would draw it empty. So each drawing is kept here, and a slot that lost its own is recorded again as a row drawing
 * it is drawn ([refill]). A slot's owners are the rows and layers whose drawings draw it: a layer outlives the rows
 * drawing it, and changes apart from them.
 */
internal class Slots(private val again: Player, private val alive: (Int) -> Boolean) {
    /** The nodes by image view; a slot nothing draws has none, until something does. */
    val nodes = HashMap<Int, RenderNode>()
    /** Called when [nodes] changes (the worker's copy of it is then made again). */
    var changed: () -> Unit = {}
    private val drawings = HashMap<Int, IntArray>()
    /** The slots each picture is drawn in: freeing it empties them. */
    private val drawnIn = HashMap<Int, HashSet<Int>>()
    /** Each owner's slots (a row by its id, a layer by [layer]), and how many owners each slot has. */
    private val byOwner = HashMap<Int, HashSet<Int>>()
    private val owners = HashMap<Int, Int>()

    /** Slot [id]'s node, its drawing recorded in it when it is made. */
    fun node(id: Int): RenderNode = nodes[id] ?: RenderNode("exact slot").also { node ->
        node.setPosition(0, 0, 1 shl 14, 1 shl 14); node.setClipToBounds(false)
        nodes[id] = node
        changed()
        record(id)
    }

    /** [owner]'s drawing draws slot [id]. */
    fun own(owner: Int, id: Int) {
        if (byOwner.getOrPut(owner) { HashSet(4) }.add(id)) owners[id] = (owners[id] ?: 0) + 1
    }

    /** [owner]'s drawing is made again by [walk], which names each slot it draws ([own]): those it no longer draws
     * are let go after, so a slot it still draws keeps its node. */
    fun again(owner: Int, walk: () -> Unit) {
        val before = byOwner.remove(owner)
        walk()
        before?.let { letGo(it) }
    }

    /** [owner] is gone: a slot nothing draws lets its node go, and keeps its drawing. */
    fun gone(owner: Int) { byOwner.remove(owner)?.let { letGo(it) } }

    private fun letGo(ids: Set<Int>) {
        for (id in ids) {
            val n = (owners[id] ?: 1) - 1
            if (n > 0) { owners[id] = n; continue }
            owners.remove(id)
            nodes.remove(id)?.let { it.discardDisplayList(); changed() }
        }
    }

    /** Slot [id]'s drawing, [b] from [from] to [to], recorded with [player] into its node, if anything draws it. A
     * drawing of a picture already freed (a waiting row's, kept after the free) is not kept. */
    fun set(id: Int, b: IntBuffer, from: Int, to: Int, player: Player) {
        nodes[id]?.let { node ->
            val c = node.beginRecording()
            player.replay(c, b, from, to)
            node.endRecording()
        }
        drawings.remove(id)?.let { old -> pictures(IntBuffer.wrap(old), 0, old.size) { unlist(it, id) } }
        var live = to > from
        pictures(b, from, to) { if (!alive(it)) live = false }
        if (!live) return
        pictures(b, from, to) { drawnIn.getOrPut(it) { HashSet(2) }.add(id) }
        drawings[id] = IntArray(to - from) { b.get(from + it) }
    }

    /** Picture [pic] is freed: the slots drawing it draw nothing, in the same nodes (a kept row draws those). */
    fun pictureFreed(pic: Int) {
        for (id in drawnIn.remove(pic) ?: return) {
            drawings.remove(id)?.let { old -> pictures(IntBuffer.wrap(old), 0, old.size) { if (it != pic) unlist(it, id) } }
            nodes[id]?.discardDisplayList()
        }
    }

    /** Picture [pic] has new pixels (a hardware bitmap): the slots drawing it are recorded again. */
    fun redraw(pic: Int) {
        for (id in drawnIn[pic] ?: return) record(id)
    }

    /** As [owner] is drawn (a row) or recorded (a layer): each of its slots that lost its drawing is recorded again. */
    fun refill(owner: Int) {
        for (id in byOwner[owner] ?: return) if (nodes[id]?.hasDisplayList() == false) record(id)
    }

    private fun unlist(pic: Int, id: Int) {
        val ids = drawnIn[pic] ?: return
        ids.remove(id)
        if (ids.isEmpty()) drawnIn.remove(pic)
    }

    private fun record(id: Int) {
        val node = nodes[id] ?: return
        val drawing = drawings[id] ?: return
        val c = node.beginRecording()
        again.replay(c, IntBuffer.wrap(drawing), 0, drawing.size)
        node.endRecording()
    }

    /** Each picture a slot's drawing ([from] to [to]) draws. */
    private inline fun pictures(b: IntBuffer, from: Int, to: Int, each: (Int) -> Unit) {
        var k = from
        while (k < to) {
            val op = b.get(k)
            if (op == 8 || op == 14) each(b.get(k + 1))
            k = when (op) {
                1 -> k + 7; 4 -> k + 13; 6 -> k + 1; 8 -> k + 6; 11 -> k + 4; 14 -> k + 18
                26 -> k + 3 + (b.get(k + 2) + 3) / 4
                else -> to
            }
        }
    }

    companion object {
        /** Layer [id]'s owner key: below every row's. */
        fun layer(id: Int) = -1 - id
    }
}
