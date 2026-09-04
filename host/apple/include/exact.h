/* exact.h — the Apple host's C ABI, v2 (LLP 1008 §4; LLP 1031 D2).
 *
 * Every call takes a runtime handle: exact_create() hands one out (a u32,
 * never 0, never reused) and exact_destroy() frees everything attributable
 * to it — the runner, its executor, its buffers, its journal. More than one
 * runtime may live in a process; each is called on one thread (the
 * presenter's main thread). A call on a destroyed handle, or one made while
 * another call is in progress on the same runtime (from a callback), is
 * refused with a batch whose error says so — never a trap.
 *
 * The web host's buffer discipline over C: the app never hands the host a
 * pointer the host did not give out. exact_in(rt, len) resizes the runtime's
 * input buffer and returns its address; the app writes a payload there.
 * Every other call returns the length of the output buffer, whose address
 * exact_out(rt) reports; the app reads a UTF-8 JSON batch from it:
 *   {"ops":[...],"timers":bool,"motion":bool,"clock":ms,"error":null|"..."}
 * with ops create / props / style / children / destroy / roots / frame /
 * content / present / surface / command (host/apple/src/batch.rs).
 *
 * Text measurement and the plan-scoped font catalog are the calls the other
 * way, set per runtime before its first boot: exact_set_measure registers
 * the measure function, exact_set_fonts the catalog hook a boot calls
 * synchronously before the first paragraph is measured, exact_set_wake the
 * callback a request's reply queues (LLP 1016 D2). Strings are UTF-8 bytes
 * with lengths, never NUL-terminated. Points throughout.
 */
#ifndef EXACT_H
#define EXACT_H
#include <stddef.h>
#include <stdint.h>

/* The ABI's version: part of the compatibility id (LLP 1030 D3a). */
#define EXACT_ABI_VERSION 2

#ifdef __cplusplus
extern "C" {
#endif

/* A runtime handle (LLP 1031 D2). 0 is never a runtime. */
typedef uint32_t ExactRuntime;

/* Width/height offers below zero mean "as the content wants". */
#define EXACT_MAX_CONTENT (-1.0f)
#define EXACT_MIN_CONTENT (-2.0f)

typedef struct ExactTextRun {
    const uint8_t *text;   /* UTF-8, not NUL-terminated */
    size_t len;            /* bytes */
    float font_size;       /* points */
    uint16_t font_weight;  /* CSS 100–900 */
    uint16_t font_family;  /* plan stack id */
    uint8_t italic;        /* 1 for italic */
    float line_height;     /* points; 0 = the font's natural line height */
    float letter_spacing;  /* points per glyph */
} ExactTextRun;

typedef struct ExactMeasureRequest {
    const ExactTextRun *runs;
    size_t count;
    float width;           /* points, or EXACT_MAX_CONTENT / EXACT_MIN_CONTENT */
    float height;          /* points, or EXACT_MAX_CONTENT / EXACT_MIN_CONTENT */
    uint8_t align;         /* 0 left, 1 center, 2 right, 3 justify */
    uint32_t line_clamp;   /* 0 = unlimited */
} ExactMeasureRequest;

typedef struct ExactMetrics {
    float width;
    float height;
    float baseline;        /* top to first alphabetic baseline; < 0 = unknown */
} ExactMetrics;

typedef ExactMetrics (*ExactMeasureFn)(void *ctx, const ExactMeasureRequest *request);

/* The plan's declared faces, synchronously before first layout. The strings
 * are UTF-8 and live only for the callback. This is a host seam, not an
 * exact_out() batch: that buffer continues to carry kernel ops only. */
typedef struct ExactFontFace {
    const uint8_t *family;
    size_t family_len;
    const uint8_t *source;
    size_t source_len;
    uint16_t stack;
    uint16_t weight;
    uint8_t italic;
} ExactFontFace;

typedef struct ExactFontCatalog {
    const ExactFontFace *faces;
    size_t count;
} ExactFontCatalog;

typedef void (*ExactFontsFn)(void *ctx, const ExactFontCatalog *catalog);

/* A request's reply is queued (LLP 1016 D2): called on the executor's
 * thread, carrying only ctx; the host hops to the runtime's thread and calls
 * exact_pump. May be NULL: replies then wait for the next exact_pump. A
 * wake may arrive after exact_destroy (a job already running): the host
 * looks its ctx up among its live sessions and drops a stranger's. */
typedef void (*ExactWakeFn)(void *ctx);

/* Lifecycle. */
ExactRuntime exact_create(void);
void exact_destroy(ExactRuntime rt);
void exact_set_measure(ExactRuntime rt, ExactMeasureFn measure, void *ctx);   /* NULL: a monospace reference measurer */
void exact_set_wake(ExactRuntime rt, ExactWakeFn wake, void *ctx);
void exact_set_fonts(ExactRuntime rt, ExactFontsFn fonts, void *ctx);

/* Buffers. exact_in returns NULL for a handle nobody holds. */
uint8_t *exact_in(ExactRuntime rt, size_t len);
const uint8_t *exact_out(ExactRuntime rt);

/* Boot the plan baked into the library (or, exact_boot_plan, the input
 * buffer's first len bytes — the dev loop's restart, state carried) under a
 * viewport. Returns the first batch's length. A boot that fails leaves the
 * running app, if any, exactly as it was. */
uint32_t exact_boot(ExactRuntime rt, float width, float height);
uint32_t exact_boot_plan(ExactRuntime rt, size_t len, float width, float height);
/* Every queued reply into the runner, on its thread: the batch of their
 * commits (empty when none). A request the app sends (LLP 1016) runs on the
 * library's own executor thread — ibex2::host — never through the host. */
uint32_t exact_pump(ExactRuntime rt, double now_ms);

/* kind: 0 = press, 1 = change, 2 = hover in, 3 = hover out, 4 = focus,
 * 5 = blur, 6 = key, 7 = submit, 8 = iframe load, 9 = iframe message.
 * A change's text, key's name, or guest message is the payload in the input
 * buffer's first len bytes. */
uint32_t exact_dispatch(ExactRuntime rt, uint32_t view, uint32_t kind, size_t len, double now_ms);
uint32_t exact_advance(ExactRuntime rt, double now_ms);   /* the runner's clock: timers */
uint32_t exact_resize(ExactRuntime rt, float width, float height);
/* The safe-area insets (points) under viewport-fit=cover — what
 * env(safe-area-inset-*) resolves to; zero when the layout viewport is the
 * safe area itself. A change re-sends the style of every node that reads
 * them and lays out again. */
uint32_t exact_insets(ExactRuntime rt, float top, float right, float bottom, float left);
uint32_t exact_tick(ExactRuntime rt, double now_ms);      /* a motion frame, only while "motion" is true */
/* An image node loaded: its bitmap's pixel counts, taken one-for-one as
 * points (never divided by the backing scale — a 2× asset is not half its
 * pixels wide, as on the web); a width or height ≤ 0 clears it (the load
 * failed, or the source was removed). Lays out again; the batch carries
 * every frame that moved. */
uint32_t exact_intrinsic(ExactRuntime rt, uint32_t view, float width, float height);

/* The agent API (LLP 1012): a request in the input buffer's first len bytes
 * ({"op":"tree"} / "state" / "logs" / "settle"), the reply in the output
 * buffer — JSON, not a batch. */
uint32_t exact_agent(ExactRuntime rt, size_t len);

/* The update store (LLP 1026 D9/D11; LLP 1030 D7): one per process — the
 * app's container holds it and every runtime boots from the same selection
 * — so these take no handle, except exact_update_sync. The same buffer
 * discipline, with the store's own pair: exact_update_in(len) is where a
 * payload goes, each call answers with a length, exact_update_out() is the
 * answer's address. Order at launch: open, then select (the entry's assets
 * directory, for the host's asset overrides), then exact_boot, which boots
 * the selected plan itself — the entry's or the baked one — and counts the
 * boot; exact_update_boot_succeeded at first pixel. The check runs on a
 * thread of the library's own over its transport and reports through done,
 * called on that thread with ctx and one UTF-8 line ("current" / "staged
 * seq N" / "refused: …") alive for the call; the host hops to its main
 * thread and calls exact_update_sync per runtime so the app's delivery
 * resource follows. A binary that links no store (L = 0) refuses open and
 * answers the embedded facts. */
typedef void (*ExactUpdateDoneFn)(void *ctx, const uint8_t *line, size_t len);
uint8_t *exact_update_in(size_t len);
const uint8_t *exact_update_out(void);
/* The payload: {"base":"<data directory>","assets":"<asset root>"}; the store
 * is <base>/exact/<app id>/update. 0, or the refusal's length. */
uint32_t exact_update_open(size_t len);
/* {"entry":"<sha256>"|null,"seq":N,"plan":"<path>","assets":"<dir>"}, the
 * paths empty for entry zero. */
uint32_t exact_update_select(void);
void exact_update_boot_succeeded(void);
/* 0 started, 1 a check is already running, 2 no store is open. */
uint32_t exact_update_check(ExactUpdateDoneFn done, void *ctx);
/* The staged plan's bytes (the app's deliveryActivate); 0 when none. The
 * host applies them to every session with carry (exact_boot_plan) and takes
 * the entry's assets from exact_update_select. */
uint32_t exact_update_activate(void);
uint32_t exact_update_sync(ExactRuntime rt);

#ifdef __cplusplus
}
#endif
#endif
