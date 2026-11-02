# speed.py — the machine's CPU speed right now, in one line: loops/s of a fixed integer loop for one process
# alone, then per process for four at once (0.7 s each). A healthy Mac holds the per-process rate at four.
# (M1 Max healthy: ~560 alone, ~530 at four; bones M4: ~850, ~825. 2026-09-30 09:23 the M1 read 270-348 and 146.)
import time, multiprocessing as mp
def work(q, secs):
    end = time.perf_counter() + secs; n = 0; x = 0
    while time.perf_counter() < end:
        for i in range(20000): x = (x * 1103515245 + 12345) & 0x7fffffff
        n += 1
    q.put(n / secs)
def rate(procs, secs=0.7):
    q = mp.Queue(); ps = [mp.Process(target=work, args=(q, secs)) for _ in range(procs)]
    [p.start() for p in ps]; r = sorted(q.get() for _ in ps); [p.join() for p in ps]
    return r[len(r) // 2]
if __name__ == "__main__":
    print("speed1 %.0f speed4 %.0f" % (rate(1), rate(4)))
