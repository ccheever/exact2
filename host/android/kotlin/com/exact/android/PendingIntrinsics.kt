package com.exact.android

import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Coalesced natural sizes keep the logical owner that produced each value.
 * A native turn may unmount or replace it before the batch crosses JNI.
 */
internal class PendingIntrinsics<Owner : Any> {
    companion object {
        // Initial mount keeps the Node itself; only a later source change needs
        // a new identity. Target-size/object-fit changes keep natural dimensions.
        fun sourceOwner(owner: Any, previous: String?, next: String): Any =
            if (previous != null && previous != next) Any() else owner
    }
    private data class Size<Owner>(val owner: Owner, val width: Float, val height: Float)
    private val values = LinkedHashMap<Int, Size<Owner>>()

    fun put(id: Int, owner: Owner, width: Float, height: Float) {
        values[id] = Size(owner, width, height)
    }

    fun drain(currentOwner: (Int) -> Owner?): ByteArray? {
        values.entries.removeAll { it.value.owner !== currentOwner(it.key) }
        if (values.isEmpty()) return null
        val bytes = ByteBuffer.allocate(values.size * 12).order(ByteOrder.LITTLE_ENDIAN)
        for ((id, size) in values) bytes.putInt(id).putFloat(size.width).putFloat(size.height)
        values.clear()
        return bytes.array()
    }

    fun clear() = values.clear()
}
