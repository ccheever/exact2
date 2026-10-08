package com.exact.android

internal object ImageRequestsTest {
    private class Listener(val value: String)
    fun run(): String {
        val q = ImageRequests<String, Listener>(2)
        val a = Listener("old owner")
        val b = Listener("other live owner")
        val c = Listener("replacement owner")
        q.add("same image", a)
        val work = checkNotNull(q.next())
        q.add("same image", b)
        check(q.pending == 1 && q.active == 1 && q.next() == null)
        q.cancel("same image", a)
        check(q.complete(work) == listOf(b) && work.listeners.isEmpty())
        check(q.active == 0 && q.pending == 0)

        q.add("same image", a)
        val cancelled = checkNotNull(q.next())
        q.cancel("same image", a)
        q.add("second image", b)
        val second = checkNotNull(q.next())
        q.add("third image", c)
        check(q.active == 2 && q.next() == null)
        // A replacement lease revives the exact running decode. It cannot
        // bypass the workers bound while the cancelled owner's pixels finish.
        q.add("same image", c)
        check(q.next() == null && q.pending == 3)
        check(q.complete(cancelled) == listOf(c))
        val third = checkNotNull(q.next())
        check(third.key == "third image" && q.active == 2)
        check(q.complete(second) == listOf(b))
        q.cancel("third image", c)
        check(q.complete(third).isEmpty() && q.active == 0 && q.pending == 0)

        q.add("waiting", a); q.add("waiting", b)
        q.cancel("waiting", a)
        check(q.pending == 1)
        q.cancel("waiting", b)
        check(q.pending == 0 && q.next() == null)
        q.add("new waiting owner", c)
        check(q.complete(checkNotNull(q.next())) == listOf(c))

        q.add("active", a)
        val active = checkNotNull(q.next())
        q.add("queued", b)
        check(q.close().toSet() == setOf(a, b))
        check(q.closed && q.pending == 1 && q.active == 1 && q.next() == null)
        check(q.complete(active).isEmpty() && q.pending == 0 && q.active == 0)
        check(q.close().isEmpty())
        check(runCatching { q.add("closed", c) }.isFailure)
        check(runCatching { q.complete(active) }.isFailure)
        return "ImageRequestsTest: PASS (shared decode, independent cancel, same-key replacement, worker ceiling, queued retirement, close/late completion)"
    }
    @JvmStatic fun main(args: Array<String>) { println(run()) }
}
