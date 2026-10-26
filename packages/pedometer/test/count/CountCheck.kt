// The Android pedometer's sums (LLP 1133.026 §15 stage 3), fed recorded reads and counter events, since an emulator
// need not have a step counter or current Play services. `count.mjs` compiles this with `android/Count.kt` and runs it.
import exact.pedometer.Count
import kotlin.system.exitProcess

var failed = 0

fun check(name: String, answer: Count.Answer, steps: Double, since: Double) {
    val ok = answer.steps == steps && answer.since == since
    if (!ok) failed++
    println("${if (ok) "PASS" else "FAIL"} count $name${if (ok) "" else ": ${answer.steps} since ${answer.since}, not $steps since $since"}")
}

fun checkThat(name: String, ok: Boolean) {
    if (!ok) failed++
    println("${if (ok) "PASS" else "FAIL"} count $name")
}

fun main() {
    // The sensor path: the counter's increase from its first event.
    Count(0.0).run {
        counter(1000.0, 10.0)
        check("sensor: the first event is the zero", sensor(10.0), 0.0, 0.0)
        counter(1050.0, 20.0)
        check("sensor: steps since the first event", sensor(20.0), 50.0, 0.0)
        // A hidden stretch: no event arrives meanwhile, so the return takes a new zero and moves `since`.
        hidden()
        counter(1200.0, 100.0)
        check("sensor: after a hidden stretch, a new zero and since", sensor(100.0), 0.0, 100.0)
        counter(1230.0, 110.0)
        check("sensor: counting on after the return", sensor(110.0), 30.0, 100.0)
        // A reboot: the counter falls and counts on from its new value.
        counter(5.0, 200.0)
        check("sensor: a reboot's fall is a new zero", sensor(200.0), 0.0, 200.0)
        counter(25.0, 210.0)
        check("sensor: counting on after the reboot", sensor(210.0), 20.0, 200.0)
    }
    // The Recording path: a total, then the counter's increase until the next read.
    Count(0.0).run {
        counter(500.0, 1.0)
        checkThat("recording: the first answer reads the API", due(1.0))
        check("recording: the total read", recorded(300.0, 0.0, 1.0), 300.0, 0.0)
        counter(520.0, 2.0)
        checkThat("recording: a fresh total is not read again", !due(2.0))
        check("recording: the total and the counter's increase", between(2.0), 320.0, 0.0)
        checkThat("recording: a total a minute old is read again", due(1.0 + Count.READ_EVERY_MS))
        // A hidden stretch: the counter's last value is not trusted, so the return reads the API again.
        hidden()
        checkThat("recording: the return reads the API", due(3.0))
        check("recording: the return's total", recorded(410.0, 0.0, 4.0), 410.0, 0.0)
        check("recording: no counter event yet adds nothing", between(5.0), 410.0, 0.0)
        counter(900.0, 6.0)
        counter(915.0, 7.0)
        check("recording: the counter counts from its first event after the read", between(7.0), 425.0, 0.0)
        // A reboot between reads: the counter falls; the total stands and the counter counts on from its new value.
        counter(3.0, 8.0)
        counter(13.0, 9.0)
        check("recording: a reboot's fall adds no negative steps", between(9.0), 435.0, 0.0)
    }
    println(if (failed > 0) "$failed failed" else "all passed")
    exitProcess(if (failed > 0) 1 else 0)
}
