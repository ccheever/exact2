package com.exact.android

import java.nio.ByteBuffer
import java.nio.ByteOrder
import android.util.SparseArray
import org.json.JSONObject
import org.json.JSONTokener

/** EXA1: fixed binary hot operations, length-delimited existing JSON cold operations. */
internal object BatchReader {
    private const val STYLE_SLOTS = 128
    private const val STYLE_BYTES = 64 * 1024
    data class Schedule(val flags: Int, val clock: Double, val due: Double, val metadata: JSONObject?)
    private fun ByteBuffer.utf8(length: Int): String {
        require(length in 0..remaining()) { "truncated Android batch" }
        val bytes = ByteArray(length)
        get(bytes)
        return bytes.toString(Charsets.UTF_8)
    }

    fun apply(buffer: ByteBuffer, target: Presenter): Schedule {
        buffer.order(ByteOrder.LITTLE_ENDIAN).position(0)
        require(buffer.remaining() >= 32 && buffer.int == 0x31415845) { "invalid Android batch magic" }
        require(buffer.short.toInt() == 1) { "unsupported Android batch version" }
        val flags = buffer.short.toInt() and 0xffff
        val count = buffer.int
        val clock = buffer.double; val due = buffer.double; val metadataLength = buffer.int
        require(count in 0..1000000 && metadataLength >= 0)
        // Validate all record bounds before touching the retained native tree.
        val start = buffer.position()
        var styles: Array<JSONObject?>? = null
        var cold: SparseArray<JSONObject>? = null
        var styleBytes = 0
        repeat(count) {
            require(buffer.remaining() >= 5)
            val opcode = buffer.get().toInt() and 255
            val length = buffer.int
            require(length in 0..buffer.remaining())
            val payload = buffer.position()
            val end = payload + length
            when (opcode) {
                1 -> {
                    val parsed = JSONObject(buffer.utf8(length))
                    val records = cold ?: SparseArray<JSONObject>().also { cold = it }
                    records.put(payload, parsed)
                }
                2 -> require(length == 20)
                3 -> require(length == 12)
                4 -> {
                    require(length == 22 || length == 38)
                    val property = buffer.get(buffer.position() + 4).toInt()
                    val arity = buffer.get(buffer.position() + 5).toInt()
                    require(property in 1..5 && ((arity == 2 && length == 22) || (arity == 4 && length == 38)))
                }
                5 -> {
                    require(length >= 8)
                    val mask = buffer.getInt(buffer.position() + 4)
                    require(mask and 63 == mask && length == 8 + Integer.bitCount(mask) * 8)
                }
                6 -> require(length == 4)
                7 -> {
                    require(length >= 6)
                    val slot = buffer.int
                    val pool = styles ?: arrayOfNulls<JSONObject>(STYLE_SLOTS).also { styles = it }
                    require(slot in pool.indices && pool[slot] == null) { "invalid or duplicate Android style slot" }
                    val bytes = length - 4
                    require(bytes <= STYLE_BYTES - styleBytes) { "Android style pool exceeds its byte budget" }
                    val parser = JSONTokener(buffer.utf8(bytes))
                    val style = parser.nextValue()
                    require(style is JSONObject && parser.nextClean() == '\u0000') { "invalid Android style definition" }
                    pool[slot] = style
                    styleBytes += bytes
                }
                8 -> {
                    require(length >= 6)
                    val slot = buffer.int
                    val pool = styles
                    require(pool != null && slot in pool.indices && pool[slot] != null) { "unknown or forward Android style reference" }
                    val op = JSONObject(buffer.utf8(length - 4))
                    val name = op.optString("op")
                    require((name == "create" || name == "style") &&
                        op.getJSONObject("style").length() == 0) { "invalid Android style reference operation" }
                    val records = cold ?: SparseArray<JSONObject>().also { cold = it }
                    records.put(payload, op)
                }
                else -> error("unsupported Android record $opcode")
            }
            buffer.position(end)
        }
        require(buffer.remaining() == metadataLength)
        val metadata = if (metadataLength == 0) null else JSONObject(buffer.utf8(metadataLength))
        if (metadata != null && !metadata.isNull("error")) error(metadata.getString("error"))
        require(flags and (8 or 32) == 0) { "Android canvas executor is not implemented" }
        buffer.position(start)
        try {
            repeat(count) {
                val opcode = buffer.get().toInt() and 255
                val length = buffer.int
                val end = buffer.position() + length
                when (opcode) {
                    1 -> { target.cold(checkNotNull(cold?.get(buffer.position()))); buffer.position(end) }
                    2 -> target.frame(buffer.int, buffer.float, buffer.float, buffer.float, buffer.float)
                    3 -> target.content(buffer.int, buffer.float, buffer.float)
                    4 -> {
                        val id = buffer.int; val property = buffer.get().toInt(); val arity = buffer.get().toInt()
                        val x = buffer.double; val y = buffer.double
                        val w = if (arity == 4) buffer.double else 1.0
                        val h = if (arity == 4) buffer.double else 1.0
                        target.present(id, property, x, y, w, h)
                    }
                    5 -> target.paint(buffer.int, buffer.int, buffer)
                    6 -> target.invalidate(buffer.int)
                    7 -> buffer.position(end)
                    8 -> {
                        val payload = buffer.position()
                        val slot = buffer.int
                        target.cold(checkNotNull(cold?.get(payload)), checkNotNull(styles?.get(slot)))
                        buffer.position(end)
                    }
                }
                check(buffer.position() == end)
            }
            target.finish()
        } finally { target.discardStylePool() }
        return Schedule(flags, clock, due, metadata)
    }
}
