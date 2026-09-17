/* exact.h — the Apple host's C ABI, v6 (LLP 1008 §4; LLP 1031 D2).
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
#define EXACT_ABI_VERSION 6

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
    uint8_t has_line_height; /* 0 normal, 1 explicit used length */
    float line_height;     /* points; zero is a real length */
    float letter_spacing;  /* points per glyph */
} ExactTextRun;

typedef struct ExactMeasureRequest {
    const ExactTextRun *runs;
    size_t count;
    ExactTextRun strut;    /* paragraph minimum line box, empty text */
    float width;           /* points, or EXACT_MAX_CONTENT / EXACT_MIN_CONTENT */
    float height;          /* points, or EXACT_MAX_CONTENT / EXACT_MIN_CONTENT */
    uint8_t align;         /* 0 left, 1 center, 2 right, 3 justify */
    uint32_t line_clamp;   /* 0 = unlimited */
    uint8_t overflow_wrap; /* 0 normal, 1 break-word, 2 anywhere */
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
/* Copy the immutable binary bake receipt into exact_out; returns its length. */
uint32_t exact_baked_compat(ExactRuntime rt);

/* Boot the plan baked into the library (or, exact_boot_plan, the input
 * buffer's first len bytes — the dev loop's restart, state carried) under a
 * viewport. Returns the first batch's length. A boot that fails leaves the
 * running app, if any, exactly as it was. */
uint32_t exact_boot(ExactRuntime rt, float width, float height);
uint32_t exact_boot_plan(ExactRuntime rt, size_t len, float width, float height);
uint32_t exact_prepare_plan(ExactRuntime rt, uint64_t token, size_t len, float width, float height);
uint32_t exact_commit_plan(ExactRuntime rt);
/* Admitted generation: concatenated plan, pairing receipt, compiled module.
 * A nonzero token supplies signed delivery facts. Identity and grants match
 * the binary; the caller admits the development origin or signed assets. */
uint32_t exact_prepare_module(ExactRuntime rt, uint64_t token, size_t plan, size_t receipt, size_t module, float width, float height);
/* Call after first pixel, never as a prerequisite to painting the baked frame. */
uint32_t exact_data_ready(ExactRuntime rt);
void exact_discard_plan(ExactRuntime rt);
/* Every queued reply into the runner, on its thread: the batch of their
 * commits (empty when none). A request the app sends (LLP 1016) runs on the
 * library's own executor thread — ibex2::host — never through the host. */
uint32_t exact_pump(ExactRuntime rt, double now_ms);

/* LLP 1038 D5/D8: input URL -> UTF-8 canonical location in exact_out.
 * The launch setter takes that location before any boot/prepare call. */
uint32_t exact_location_of(ExactRuntime rt, size_t len);
uint32_t exact_set_launch_location(ExactRuntime rt, size_t len);
/* kind: 0 = press, 1 = change, 2 = hover in, 3 = hover out, 4 = focus,
 * 5 = blur, 6 = key, 7 = submit, 8 = iframe load, 9 = iframe message,
 * 10 = contextmenu, 11 = dblclick, 12 = swiperight, 13 = scroll (UTF-8 scrollLeft,scrollTop),
 * 14 = navigate (UTF-8 location; navigation root only, LLP 1038 D8).
 * A change's text, key's name, or guest message is the payload in the input
 * buffer's first len bytes. */
uint32_t exact_dispatch(ExactRuntime rt, uint32_t view, uint32_t kind, size_t len, double now_ms);
/* Versioned LE collection feedback in exact_in; returns the ordinary batch. */
uint32_t exact_collection_feedback(ExactRuntime rt, size_t len, double now_ms);
/* A horizontal drag offset in points; release returns to authored translate. */
uint32_t exact_drag_x(ExactRuntime rt, uint32_t view, double delta, double velocity, uint32_t release, double now_ms);
uint32_t exact_advance(ExactRuntime rt, double now_ms);   /* the runner's clock: timers */
/* @ref LLP 1039: re-answer viewport facts and relayout in the same batch. */
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

/* A host line for the runner's journal (LLP 1012 §3; LLP 1035.001 D6): the
 * input buffer's first len bytes — a refused intent and its reason. Returns 0. */
uint32_t exact_log(ExactRuntime rt, size_t len);

/* Optional delivery composition (LLP 1030 D4). L=0 returns NULL: no store,
 * keys, selection, check or networking implementation is linked. The higher
 * update adapter supplies these calls only when the app chooses L=A. */
typedef void (*ExactUpdateDoneFn)(void *ctx, const uint8_t *line, size_t len);
typedef struct {
    uint8_t *(*input)(size_t len);
    const uint8_t *(*output)(void);
    uint32_t (*open)(size_t len);
    uint32_t (*select)(void);
    void (*boot_succeeded)(uint64_t token);
    uint32_t (*check)(ExactUpdateDoneFn done, void *ctx);
    uint32_t (*prepare)(void);
    uint32_t (*plan)(uint64_t token);
    uint32_t (*asset)(uint64_t token, size_t len);
    uint32_t (*commit)(uint64_t token);
    void (*discard)(uint64_t token);
    uint32_t (*refuse)(uint64_t token, size_t len);
    void (*started)(uint64_t token);
} ExactDeliveryApi;
const ExactDeliveryApi *exact_delivery_api(void);
uint32_t exact_delivery_sync(ExactRuntime rt);

#ifdef __cplusplus
}
#endif
#endif
