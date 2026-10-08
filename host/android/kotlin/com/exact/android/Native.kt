package com.exact.android

import java.nio.ByteBuffer

/** One JNI call per committed turn. Returned buffers are borrowed until the next call. */
internal object Native {
    init { System.loadLibrary("exact_app") }
    @JvmStatic external fun create(text: TextEngine, reuseRows: Boolean): Long
    @JvmStatic external fun rowReuse(handle: Long, enabled: Boolean)
    @JvmStatic external fun boot(handle: Long, width: Float, height: Float, initialPress: ByteArray? = null): ByteBuffer
    @JvmStatic external fun dispatch(handle: Long, view: Int, kind: Int, payload: ByteArray?, now: Double): ByteBuffer
    @JvmStatic external fun resize(handle: Long, width: Float, height: Float): ByteBuffer
    @JvmStatic external fun frame(handle: Long, now: Double): ByteBuffer
    @JvmStatic external fun advance(handle: Long, now: Double): ByteBuffer
    @JvmStatic external fun tick(handle: Long, now: Double): ByteBuffer
    @JvmStatic external fun pump(handle: Long, now: Double): ByteBuffer
    @JvmStatic external fun painted(handle: Long): ByteBuffer
    @JvmStatic external fun preferences(handle: Long, bits: Int): ByteBuffer
    @JvmStatic external fun insets(handle: Long, top: Float, right: Float, bottom: Float, left: Float): ByteBuffer
    @JvmStatic external fun intrinsic(handle: Long, view: Int, width: Float, height: Float): ByteBuffer
    @JvmStatic external fun intrinsics(handle: Long, sizes: ByteArray): ByteBuffer
    @JvmStatic external fun controlQuery(handle: Long, view: Int, kind: Int): ByteBuffer
    @JvmStatic external fun scrolled(handle: Long, positions: ByteArray)
    @JvmStatic external fun collectionFeedback(handle: Long, facts: ByteArray, now: Double): ByteBuffer
    @JvmStatic external fun agent(handle: Long, request: ByteArray): ByteBuffer
    @JvmStatic external fun bridgeStats(handle: Long): LongArray
    @JvmStatic external fun close(handle: Long)
}
