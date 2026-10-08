package com.exact.android

import java.util.ArrayDeque

/** Owner-thread work admission: one pixel decode per exact demand, even if its
 * last observer cancels and another observes it before the decoder returns.
 * Running work occupies its worker slot until complete; cancelling cannot
 * accidentally admit another decode into that slot.
 */
internal class ImageRequests<K : Any, L : Any>(private val workers: Int) {
    internal class Work<K, L>(val key: K) {
        val listeners = LinkedHashSet<L>()
        var running = false
            internal set
        var completed = false
            internal set
    }
    private val indexed = LinkedHashMap<K, Work<K, L>>()
    private val waiting = ArrayDeque<Work<K, L>>()
    var active = 0
        private set
    var closed = false
        private set
    val pending: Int get() = indexed.size

    init { require(workers > 0) }
    fun add(key: K, listener: L) {
        check(!closed)
        val work = indexed.getOrPut(key) { Work<K, L>(key).also(waiting::addLast) }
        check(!work.completed && work.listeners.add(listener))
    }
    fun cancel(key: K, listener: L) {
        val work = indexed[key] ?: return
        work.listeners.remove(listener)
        if (work.listeners.isEmpty() && !work.running) {
            waiting.remove(work)
            indexed.remove(key)
            work.completed = true
        }
    }
    fun next(): Work<K, L>? {
        if (closed || active >= workers || waiting.isEmpty()) return null
        val work = waiting.removeFirst()
        check(indexed[work.key] === work && !work.running && !work.completed && work.listeners.isNotEmpty())
        work.running = true
        active++
        return work
    }
    fun complete(work: Work<K, L>): List<L> {
        check(work.running && !work.completed && indexed[work.key] === work)
        indexed.remove(work.key)
        work.running = false
        work.completed = true
        active--
        return work.listeners.toList().also { work.listeners.clear() }
    }
    /** Return observers for closure retirement, keeping only running slot
     * identities until the workers acknowledge their completion.
     */
    fun close(): List<L> {
        if (closed) return emptyList()
        closed = true
        val listeners = indexed.values.flatMap { it.listeners }
        for (work in indexed.values) { work.listeners.clear(); if (!work.running) work.completed = true }
        indexed.entries.removeAll { !it.value.running }
        waiting.clear()
        return listeners
    }
}
