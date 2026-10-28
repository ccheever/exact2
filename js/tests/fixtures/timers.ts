// LLP 1027.000 D1 (amended 2026-10-28): an answer's own timers, due on the
// runner's clock. `setInterval` and the clock reads stay refused.
const sleep = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
const log: string[] = [];
// A fetch one answer starts and another awaits too.
let shared: Promise<string> | null = null;

let atInit: string;
try { setTimeout(() => {}, 0); atInit = "UNGUARDED"; }
catch (e) { atInit = e.message; }

async function answer(source: string, args: any[]): Promise<unknown> {
  switch (source) {
    case "slept": {
      const [label, ms] = args as [string, number];
      await sleep(ms);
      return label + " after " + ms;
    }
    // Armed out of order, they run in due order, each callback's microtasks
    // before the next callback, as a browser runs them between tasks; one
    // armed from a callback is due from that callback's time.
    case "order": {
      const seen: string[] = [];
      const timer = (ms: number) => new Promise<void>(resolve => setTimeout(() => {
        seen.push("t" + ms);
        Promise.resolve().then(() => seen.push("m" + ms));
        resolve();
      }, ms));
      await Promise.all([timer(300), timer(100), timer(200)]);
      await new Promise<void>(resolve => setTimeout(() => { seen.push("then"); resolve(); }, 50));
      return seen.join(",");
    }
    case "cleared": {
      const id = setTimeout(() => log.push("cleared ran"), 50);
      clearTimeout(id);
      await sleep(100);
      return log.join(",") || "none";
    }
    case "abort": {
      const signal = AbortSignal.timeout(args[0] as number);
      await new Promise(resolve => signal.addEventListener("abort", resolve));
      return (signal.reason as Error).name;
    }
    case "fetchTimeout":
      try {
        await fetch("https://fixture.exact.test/slow", { signal: AbortSignal.timeout(250) });
        return "fetched";
      } catch (e) {
        return e.name;
      }
    case "atInit": return atInit;
    case "interval":
      try { setInterval(() => {}, 10); return "UNGUARDED"; }
      catch (e) { return e.message; }
    case "letGo":
      log.push("armed " + args[0]);
      await sleep(500);
      log.push("ran " + args[0]);
      return "done " + args[0];
    case "unawaited":
      setTimeout(() => log.push("unawaited ran"), 10);
      return "given";
    case "log": return log.join(",");
    case "starter":
      shared = fetch("https://fixture.exact.test/shared").then(() => "shared");
      return shared;
    // Whichever comes first: another answer's fetch, or its own long timer.
    case "racer":
      return Promise.race([shared!, sleep(10000).then(() => "slept")]);
    // Natively one wait at a time: the fetch, then the timer.
    case "raceFetch":
      return Promise.race([
        fetch("https://fixture.exact.test/slow").then(() => "fetched"),
        sleep(100).then(() => "slept"),
      ]);
    case "chain": {
      await sleep(100);
      await sleep(200);
      await sleep(400);
      return "chained";
    }
    // Over the turn's budget, so the runner asks it again later.
    case "spin": {
      if (args[0] === 0) return "idle";
      let x = 0;
      for (let i = 0; i < 20000000; i++) x = (x + i) % 7;
      return "spun " + args[0] + " " + x;
    }
    default: throw new Error("unknown source " + source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.timers",
  grants: "net.fetch https://fixture.exact.test\n",
  answer,
};
