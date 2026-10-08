package com.exact.android

import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Reproduce callback enqueue -> native unmount/replace -> owner batch drain. */
internal object PendingIntrinsicsTest {
    private data class Owner(val name: String)
    private data class Size(val id: Int, val width: Float, val height: Float)
    private fun sizes(bytes: ByteArray?): List<Size> {
        if (bytes == null) return emptyList()
        check(bytes.size % 12 == 0)
        val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        return List(bytes.size / 12) { Size(buffer.int, buffer.float, buffer.float) }
    }
    fun run(): String {
        val queue = PendingIntrinsics<Owner>()
        val oldImage = Owner("image")
        val liveImage = Owner("live image")
        val control = Owner("control")
        val native = Owner("native component")
        val mounted = mutableMapOf(1 to oldImage, 2 to liveImage, 3 to control, 4 to native)
        queue.put(1, oldImage, 320f, 180f)
        queue.put(2, liveImage, 100f, 50f)
        queue.put(3, control, 88f, 44f)
        queue.put(4, native, 200f, 80f)
        mounted.remove(1) // The next native turn virtualizes the first image out.
        check(sizes(queue.drain(mounted::get)) == listOf(Size(2, 100f, 50f), Size(3, 88f, 44f), Size(4, 200f, 80f)))
        check(queue.drain(mounted::get) == null) // No empty native intrinsic turn.

        mounted[1] = oldImage
        queue.put(1, oldImage, 320f, 180f)
        mounted[1] = Owner("image") // Equal value and same id, different owner.
        check(queue.drain(mounted::get) == null) // Replacement must not inherit it.
        val replacement = checkNotNull(mounted[1])
        queue.put(1, replacement, 240f, 90f)
        queue.put(1, replacement, 300f, 120f) // New live reports still coalesce.
        check(sizes(queue.drain(mounted::get)) == listOf(Size(1, 300f, 120f)))

        queue.put(3, control, -1f, -1f) // Live clear records retain their ABI meaning.
        check(sizes(queue.drain(mounted::get)) == listOf(Size(3, -1f, -1f)))
        queue.put(2, liveImage, 100f, 50f)
        mounted.clear()
        check(queue.drain(mounted::get) == null)
        queue.put(1, replacement, 1f, 1f)
        queue.clear()
        check(queue.drain { replacement } == null)
        // Exercise the same source transition helper used by Presenter.loadImage.
        val imageQueue = PendingIntrinsics<Any>()
        val imageNode = Any()
        var imageOwner = PendingIntrinsics.sourceOwner(imageNode, null, "a.jpg")
        check(imageOwner === imageNode) // Initial mount allocates no lease token.
        imageQueue.put(7, imageOwner, 100f, 50f)
        imageOwner = PendingIntrinsics.sourceOwner(imageOwner, "a.jpg", "a.jpg")
        check(sizes(imageQueue.drain { imageOwner }) == listOf(Size(7, 100f, 50f)))
        imageQueue.put(7, imageOwner, 100f, 50f)
        imageOwner = PendingIntrinsics.sourceOwner(imageOwner, "a.jpg", "b.jpg")
        check(imageOwner !== imageNode && imageQueue.drain { imageOwner } == null)
        imageQueue.put(7, imageOwner, 200f, 80f)
        imageOwner = PendingIntrinsics.sourceOwner(imageOwner, "b.jpg", "")
        check(imageQueue.drain { imageOwner } == null)
        imageQueue.put(7, imageOwner, -1f, -1f)
        check(sizes(imageQueue.drain { imageOwner }) == listOf(Size(7, -1f, -1f)))
        return "PendingIntrinsicsTest: PASS (unmount, same-id replacement, live image/control/native sizes, coalescing, clear, empty batch, image source generation)"
    }
    @JvmStatic fun main(args: Array<String>) { println(run()) }
}
