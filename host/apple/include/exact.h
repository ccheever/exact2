/* exact.h — the Apple host's C ABI, v9 (LLP 1008 §4; LLP 1031 D2).
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
 * content / present / surface / surfaceWork / command (host/apple/src/batch.rs).
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
#define EXACT_ABI_VERSION 9

#ifdef __cplusplus
extern "C" {
#endif

/* A runtime handle (LLP 1031 D2). 0 is never a runtime. */
typedef uint32_t ExactRuntime;

/* Raster accounts outlive replaceable runtimes. Handles are process-unique;
 * charge/lease release is worker-safe and never requires a live Runtime. */
typedef struct ExactRasterDemand {
    uint64_t view, view_generation, source, generation;
    uint32_t width, height, natural_width, natural_height;
    uint32_t priority; /* 0 visible, 1 overscan */
    uint64_t encoded_bytes, header_bytes, stride, scratch_bytes;
} ExactRasterDemand;
typedef struct ExactRasterWork {
    uint64_t permit, session, source, generation, charge;
    uint32_t width, height;
} ExactRasterWork;
typedef struct ExactRasterReady { uint64_t lease, payload; } ExactRasterReady;
typedef struct ExactRasterStats {
    uint64_t resident_bytes, reserved_bytes, pinned_bytes, cold_bytes, retiring_bytes, peak_bytes;
    uint64_t queued, running, ready, delivery_cells, pending_jobs, subscribers, cold_entries;
    uint64_t dedup_hits, cancelled, evicted, process_running, last_refusal, waiting_budget;
} ExactRasterStats;
uint64_t exact_raster_session_create(void);
/* 0 reset, 1 pause, 2 resume, 3 shutdown, 4 trim. */
void exact_raster_session_control(uint64_t session, uint32_t op);
uint64_t exact_raster_request(uint64_t session, ExactRasterDemand demand);
void exact_raster_cancel(uint64_t session, uint64_t request);
uint32_t exact_raster_status(uint64_t session, uint64_t request);
ExactRasterWork exact_raster_next_decode(uint32_t timeout_ms);
uint32_t exact_raster_is_cancelled(uint64_t permit);
/* Transfers one retained immutable native payload, even on stale completion.
 * Scratch must already be gone. release(payload) may run on any thread. */
uint32_t exact_raster_complete(uint64_t permit, uint64_t payload, void (*release)(uint64_t), uint64_t bytes);
void exact_raster_fail(uint64_t permit);
void exact_raster_charge_release(uint64_t charge);
ExactRasterReady exact_raster_take_ready(uint64_t session, uint64_t request);
void exact_raster_lease_release(uint64_t lease);
ExactRasterStats exact_raster_stats(uint64_t session);

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
    uint8_t font_variant_numeric; /* CSS bits: 1 tabular-nums (the face's tnum) */
} ExactTextRun;

/* LLP 1053 G5. CSS white space collapsing (normal, nowrap, pre-line) of a paragraph's
 * runs, the one algorithm the measurer applies before each measure callback. `utf8` is
 * the runs joined, `lens` their byte lengths, `white_space` as in ExactMeasureRequest.
 * Returns 0 when nothing collapses (outputs
 * untouched), else the edit count + 1, writing the collapsed text (<= len bytes),
 * each run's collapsed length, and up to edit_cap edits: from collapsed UTF-16 offset
 * `utf16` on, the source offset is collapsed + `removed`. Null outputs query. */
typedef struct ExactCollapseEdit { size_t utf16, removed; } ExactCollapseEdit;
size_t exact_text_collapse(const uint8_t *utf8, size_t len, const size_t *lens, size_t count,
    uint8_t white_space, uint8_t *out, size_t *out_lens, ExactCollapseEdit *edits, size_t edit_cap);

/* LLP 1043.000 D5-D7. Same-thread TextShape lifetime, independent of runtime.
 * Non-null buffers must be aligned and valid for their stated counts. */
typedef struct ExactFlowPair { float x, y; } ExactFlowPair;
typedef struct ExactFlowShape {
    uint32_t kind; /* 0 circle, 1 ellipse, 2 round-rect, 3 nonzero polygon, 4 spans, 5 evenodd polygon */
    float x, y, a, b, radius;
    const ExactFlowPair *pairs;
    size_t count;
} ExactFlowShape;
typedef struct ExactFlowFragment {
    size_t start, end, utf16_start, utf16_end, paint_start, paint_end;
    float x, y, width, available;
    uint8_t hyphenated;
    uint32_t line;
} ExactFlowFragment;
typedef struct ExactFlowResult { size_t count; float height; size_t bytes; uint8_t complete, clamped; } ExactFlowResult;
/* Advances: one per UTF-16 unit; cluster advance at its lowest string index.
 * Words: ascending UTF-16 line-break boundaries between Thai, Lao, Khmer or
 * Myanmar letters, the walker's only breaks inside such a run (it has no dictionary). */
uint64_t exact_textflow_prepare(const uint8_t *utf8, size_t len,
    const float *advances, size_t count, const uint32_t *words, size_t word_count,
    uint32_t overflow_wrap, uint32_t white_space, float hyphen_advance);
/* Returns required count, height, and completion. Writes min(count,cap); null output
 * is a query. max_lines counts bands (0 = no line clamp). Reject incomplete output
 * unless clamped is set; guard exhaustion requires ordinary paragraph fallback. */
ExactFlowResult exact_textflow_flow(uint64_t handle, const ExactFlowShape *shapes, size_t count,
    float width, float line_height, float font_size, uint32_t max_lines, uint32_t direction,
    ExactFlowFragment *out, size_t cap);
/* Zero, stale, and repeated free are harmless. */
void exact_textflow_free(uint64_t handle);

typedef struct ExactMeasureRequest {
    uint32_t view, node_index, node_generation;
    uint64_t revision;
    const ExactTextRun *runs;
    size_t count;
    ExactTextRun strut;    /* paragraph minimum line box, empty text */
    float width;           /* points, or EXACT_MAX_CONTENT / EXACT_MIN_CONTENT */
    float height;          /* points, or EXACT_MAX_CONTENT / EXACT_MIN_CONTENT */
    uint8_t align;         /* 0 left, 1 center, 2 right, 3 justify */
    uint32_t line_clamp;   /* 0 = unlimited */
    uint8_t overflow_wrap; /* 0 normal, 1 break-word, 2 anywhere */
    uint8_t white_space;   /* 0 normal, 1 pre-wrap, 2 nowrap, 3 pre-line; runs arrive collapsed unless 1 */
    uint8_t direction;     /* 0 ltr, 1 rtl */
    const ExactFlowShape *exclusions;
    size_t exclusion_count;
    uint8_t markup;        /* 1: the one run is Markdown source; expand it with exact_markup_pieces (LLP 1045 D3) */
} ExactMeasureRequest;

/* LLP 1045 D3/D4. Markdown source into display pieces, the same for measure and paint. */
typedef struct ExactMarkupPiece {
    const uint8_t *text; size_t len;   /* UTF-8, not NUL-terminated; may end in a newline */
    float scale;                        /* font size relative to the node's */
    uint16_t font_weight;               /* CSS weight; 0 keeps the node's */
    uint8_t italic, mono, strike;
    uint8_t role;                       /* 0 ink, 1 code, 2 link, 3 marker, 4 quote */
    const uint8_t *href; size_t href_len; /* a link's target; null when none */
} ExactMarkupPiece;
/* Writes the pieces and their count, valid until exact_markup_free(handle). Zero on invalid UTF-8. */
uint64_t exact_markup_pieces(const uint8_t *utf8, size_t len, const ExactMarkupPiece **out, size_t *count);
/* Styling for an editor, as JSON (see host/apple/src/markup.rs `style`): paragraphs, spans, hidden
 * ranges and replacements over the source in UTF-16 units; the selection reveals markers it touches
 * (sel_start UINT32_MAX for none). Valid until exact_markup_free(handle). */
uint64_t exact_markup_style(const uint8_t *utf8, size_t len, uint32_t sel_start, uint32_t sel_end, const uint8_t **out, size_t *count);
/* UTF-16 replacement JSON: {replacements:[[start,end,text],...],selection:[start,end]}.
 * Invalid commands return {error:...}; the original source is unchanged. */
uint64_t exact_markup_edit(const uint8_t *utf8, size_t len, uint32_t start, uint32_t end,
    const uint8_t *command, size_t command_len, const uint8_t *argument, size_t argument_len,
    const uint8_t **out, size_t *count);
/* {formats:string,mixed:bool,link:string,unavailable:string}; token lists use spaces. */
uint64_t exact_markup_selection(const uint8_t *utf8, size_t len, uint32_t start, uint32_t end,
    const uint8_t **out, size_t *count);
/* Raw UTF-8 plain text, sharing the same handle lifetime. */
uint64_t exact_markup_plain(const uint8_t *utf8, size_t len, const uint8_t **out, size_t *count);
void exact_markup_free(uint64_t handle);

typedef struct ExactMetrics {
    float width;
    float height;
    float baseline;        /* top to first alphabetic baseline; -1 = unknown;
                              -2 = pending native metrics for this revision */
} ExactMetrics;

/* Region completion takes one retained native artifact on every return path. */
typedef void (*ExactRegionReleaseFn)(void *owner);
uint32_t exact_text_ready(ExactRuntime rt, uint32_t index, uint32_t generation, uint64_t revision);
uint32_t exact_region_request(ExactRuntime rt, uint64_t request, uint64_t known_source);
uint32_t exact_region_complete(ExactRuntime rt, uint64_t request, ExactMetrics metrics, void *owner, ExactRegionReleaseFn release);

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
/* Presenter-owned surface work uses the ordinary runner ticket. Kinds are
 * 2 refused, 3 unsupported, 4 aborted, 6 captured bytes, 7 restored. */
uint8_t exact_request_active(ExactRuntime rt, uint64_t ticket);
uint32_t exact_fulfill_surface(ExactRuntime rt, uint64_t ticket, uint32_t kind, size_t len, double now_ms);

/* LLP 1038 D5/D8: input URL -> UTF-8 canonical location in exact_out.
 * The launch setter takes that location before any boot/prepare call. */
uint32_t exact_location_of(ExactRuntime rt, size_t len);
uint32_t exact_set_launch_location(ExactRuntime rt, size_t len);
/* kind: 0 = press, 1 = change, 2 = hover in, 3 = hover out, 4 = focus,
 * 5 = blur, 6 = key, 7 = submit, 8 = iframe load, 9 = iframe message,
 * 10 = contextmenu, 11 = dblclick, 12 = swiperight, 13 = scroll (UTF-8 scrollLeft,scrollTop),
 * 14 = navigate (UTF-8 location; navigation root only, LLP 1038 D8),
 * 15 = heightrelease, 16 = transformgeometry, 17 = transformrelease,
 * 18 = reorder (collection move payload),
 * 20 = pan (UTF-8 dx,dy; incremental viewport CSS pixels, LLP 1043.000 D8),
 * 19 = media (UTF-8 event name, newline, payload; numeric times in seconds),
 * 21 = select (formats + newline + mixed 0/1 + newline + unavailable + newline + link);
 * any other kind is refused with an error batch.
 * Format lists are space-separated command tokens. Link keeps the remaining bytes.
 * A change's text, key's name, or guest message is the payload in the input
 * buffer's first len bytes. */
uint32_t exact_dispatch(ExactRuntime rt, uint32_t view, uint32_t kind, size_t len, double now_ms);
/* Versioned LE collection feedback in exact_in; returns the ordinary batch. */
uint32_t exact_collection_feedback(ExactRuntime rt, size_t len, double now_ms);
/* Property: 0 translate, 1 scale, 2 rotate, 3 opacity. Begin replies with a
 * hold op {token:decimal-string,x,y}. Tokens belong to this runtime incarnation.
 * Check liveness before an authored action; final update, action, then end. */
/* Authored header binding, generational keys and live Height token. */
uint32_t exact_height_drag_begin(ExactRuntime rt, uint64_t handle_key, uint64_t target_key, double now_ms);
uint32_t exact_height_drag_update(ExactRuntime rt, uint64_t token, double height, double now_ms);
uint32_t exact_height_drag_release(ExactRuntime rt, uint64_t token, double height, double velocity, double now_ms);
uint32_t exact_transform_motion(uint32_t rt, uint32_t len);
/* Arrange (reorderFor / reorderdrop, LLP 1041 §8.5): the platform recognizes
 * the contact on the handle view. scroll_top is the List's actual offset as
 * its collection feedback reports it; dy the pointer's downward travel since
 * recognition, points; inside whether the pointer is in the List's port.
 * Every reply carries {"op":"reorder","token":decimal-string,"list","wrapper",
 * "phase":"active"|"settling"|"finished"|"refused","dispatched"}; a later
 * batch may carry "settling" (a receipt ended the contact) or "finished"
 * (the source settled: release the handle's interaction pin). */
uint32_t exact_reorder_begin(ExactRuntime rt, uint32_t handle, double scroll_top, double now_ms);
uint32_t exact_reorder_move(ExactRuntime rt, uint64_t token, double dy, double scroll_top, uint32_t inside, double now_ms);
uint32_t exact_reorder_end(ExactRuntime rt, uint64_t token, uint32_t drop, double dy, double scroll_top, uint32_t inside, double velocity, double now_ms);
uint32_t exact_hold_begin(ExactRuntime rt, uint32_t view, uint32_t property, double now_ms);
uint32_t exact_has_hold(ExactRuntime rt, uint64_t token);
uint32_t exact_hold_update(ExactRuntime rt, uint64_t token, double x, double y, double now_ms);
uint32_t exact_hold_end(ExactRuntime rt, uint64_t token, uint32_t cancel, double vx, double vy, double now_ms);
uint32_t exact_advance(ExactRuntime rt, double now_ms);   /* the runner's clock: timers */
/* Input: name alone clears; name NUL JSON publishes a current record. */
uint32_t exact_surface_record(ExactRuntime rt, size_t len);
/* @ref LLP 1027.000.000: the date — Unix ms at clock zero, minutes east of UTC. */
uint32_t exact_set_time(ExactRuntime rt, double epoch_at_zero, double utc_offset);
/* @ref LLP 1039: re-answer viewport facts and relayout in the same batch. */
uint32_t exact_resize(ExactRuntime rt, float width, float height);
/* Actual list scrollport and focused/interacting descendants (zero if absent).
   Row heights use the kernel frames already delivered to the presenter. */
uint32_t exact_list(ExactRuntime rt, uint32_t view, double top, double height,
                    double width, double origin, uint32_t focus, uint32_t interaction,
                    uint32_t limit, double velocity);
/* `limit` rations the report: 0 fills the whole window; n creates at most
   n - 1 rows beyond those the scrollport shows, which are never rationed.
   velocity is recent user travel in points/second (0 for deterministic reports).
   exact_list_pending says whether rows remain to create or retire (1 or 0). */
uint32_t exact_list_pending(ExactRuntime rt, uint32_t view);
/* Opaque row key in the input buffer; UINT32_MAX means absent. */
uint32_t exact_list_index(ExactRuntime rt, uint32_t view, uint32_t len);
/* Two concatenated UTF-8 keys in input; empty first key selects all text.
   Returns raw UTF-8 bytes, not a batch. Positions use UTF-16 offsets. */
uint32_t exact_list_text(ExactRuntime rt, uint32_t view, uint32_t first_len,
                        uint32_t len, uint32_t first_paragraph, uint32_t first_offset,
                        uint32_t last_paragraph, uint32_t last_offset);
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
/* exact_intrinsic for several views under one layout: the input buffer's
 * first len bytes are LE records of (uint32 view, float width, float height). */
uint32_t exact_intrinsics(ExactRuntime rt, size_t len);

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
