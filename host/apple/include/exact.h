/* exact.h — the Apple host's C ABI, v1 (LLP 1008 §4).
 *
 * The web host's buffer discipline over C: the app never hands the host a
 * pointer the host did not give out. exact_in(len) resizes a host-owned
 * input buffer and returns its address; the app writes a payload there.
 * Every other call returns the length of the output buffer, whose address
 * exact_out() reports; the app reads a UTF-8 JSON batch from it:
 *   {"ops":[...],"timers":bool,"motion":bool,"error":null|"..."}
 * with ops create / props / style / children / destroy / roots / frame /
 * content / present (host/apple/src/batch.rs). All calls on one thread.
 *
 * Text measurement and the plan-scoped font catalog are the calls the other
 * way: exact_set_fonts registers the catalog hook, and exact_boot installs
 * it synchronously before calling the measure function for each paragraph
 * the kernel lays out. Strings are UTF-8 bytes with lengths, never
 * NUL-terminated. Points throughout.
 */
#ifndef EXACT_H
#define EXACT_H
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

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

typedef void (*ExactFontsFn)(const ExactFontCatalog *catalog);
void exact_set_fonts(ExactFontsFn fonts);

/* A request's reply is queued (LLP 1016 D2): called on the executor's
 * thread, carrying nothing; the host hops to its main thread and calls
 * exact_pump. May be NULL: replies then wait for the next exact_pump. */
typedef void (*ExactWakeFn)(void *ctx);

uint8_t *exact_in(size_t len);
const uint8_t *exact_out(void);

/* Boot the plan baked into the library (or, exact_boot_plan, the input
 * buffer's first len bytes). measure may be NULL: a monospace reference
 * measurer is used. Returns the first batch's length. */
uint32_t exact_boot(ExactMeasureFn measure, void *ctx, ExactWakeFn wake, void *wake_ctx, float width, float height);
uint32_t exact_boot_plan(size_t len, ExactMeasureFn measure, void *ctx, ExactWakeFn wake, void *wake_ctx, float width, float height);
/* Every queued reply into the runner, on the main thread: the batch of their
 * commits (empty when none). A request the app sends (LLP 1016) runs on the
 * library's own executor thread — ibex2::host — never through the host. */
uint32_t exact_pump(double now_ms);

/* kind: 0 = press, 1 = change, 2 = hover in, 3 = hover out, 4 = focus,
 * 5 = blur, 6 = key, 7 = submit, 8 = iframe load, 9 = iframe message.
 * A change's text, key's name, or guest message is the payload in the input
 * buffer's first len bytes. */
uint32_t exact_dispatch(uint32_t view, uint32_t kind, size_t len, double now_ms);
uint32_t exact_advance(double now_ms);   /* the runner's clock: timers */
uint32_t exact_resize(float width, float height);
/* The safe-area insets (points) under viewport-fit=cover — what
 * env(safe-area-inset-*) resolves to; zero when the layout viewport is the
 * safe area itself. A change re-sends the style of every node that reads
 * them and lays out again. */
uint32_t exact_insets(float top, float right, float bottom, float left);
uint32_t exact_tick(double now_ms);      /* a motion frame, only while "motion" is true */
/* An image node loaded: its bitmap's pixel counts, taken one-for-one as
 * points (never divided by the backing scale — the web without srcset); a
 * finite value ≤ 0 clears it, a non-finite one is an error. The kernel lays
 * the image out from it as CSS does a replaced element. */
uint32_t exact_intrinsic(uint32_t view, float width, float height);
/* The agent API (LLP 1012): a JSON request in the input buffer's first len
 * bytes — {"op":"tree"|"state"|"settle"} or {"op":"logs","since":N} — and
 * the reply (JSON, not a batch) in the output buffer. */
uint32_t exact_agent(size_t len);

#ifdef __cplusplus
}
#endif
#endif
