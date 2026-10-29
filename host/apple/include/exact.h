/* exact.h — the Apple host's C ABI, v10 (LLP 1008 §4; LLP 1031 D2).
 *
 * Every call takes a runtime handle: exact_create() hands one out (a u32,
 * never 0, never reused) and exact_destroy() frees everything attributable
 * to it — the runner, its executor, its buffers, its journal. More than one
 * runtime may live in a process; each is called on one thread (the
 * presenter's main thread). A call on a destroyed handle, or one made while
 * another call is in progress on the same runtime (from a callback), is
 * refused with a batch whose error says so — never a trap.
 * A re-entrant refusal also says "busy":true: no runner turn completed, so
 * the presenter must keep its clock, timers, motion and owed canvas draws.
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
#define EXACT_ABI_VERSION 18

#ifdef __cplusplus
extern "C" {
#endif

/* A runtime handle (LLP 1031 D2). 0 is never a runtime. */
typedef uint32_t ExactRuntime;

/* UI-thread callback proxies, not presented-frame counts. Storage and lookup
 * are bounded to 16 collectors; create returns 0 at capacity. */
typedef struct ExactFrameHealthSnapshot {
    uint64_t generation, callbacks, intervals, active_ns, period_ns, max_gap_ns;
    uint64_t work_overruns, cadence_changes, last_arrival_ns, running;
    uint64_t applies, target_applies, apply_overruns, interval_through_ns, apply_through_ns, histogram_overflows;
} ExactFrameHealthSnapshot;
uint32_t exact_frame_health_create(void);
void exact_frame_health_record(uint32_t id, uint64_t arrival, uint64_t target, uint64_t period, uint64_t finished, uint64_t flags);
typedef struct ExactFrameHealthWork {
    uint64_t capture, began_ns, finished_ns, target_ns, queued_ns;
    uint64_t owner_began_ns, owner_finished_ns, landed_ns, operations;
} ExactFrameHealthWork;
typedef struct ExactFrameHealthHitch {
    uint64_t sequence, capture, generation, at_ns, target_ns, period_ns, requested_period_ns, gap_ns, callback_ns;
    uint64_t before_gap_ns, before_callback_ns, after_at_ns, after_gap_ns, after_callback_ns, flags;
    ExactFrameHealthWork work;
} ExactFrameHealthHitch;
typedef struct ExactFrameHealthHistoryStatus {
    uint64_t capture, active, segments, started_ns, changed_ns, callbacks;
    uint64_t total, overwritten, retained, apply_deadlines;
} ExactFrameHealthHistoryStatus;
/* Detail capture: 0 pause, 1 start/resume, 2 clear; all on the UI thread.
 * A producer retains 16 events in fixed storage, never a file or upload. */
void exact_frame_health_capture(uint32_t id, uint32_t action, uint64_t now);
ExactFrameHealthHistoryStatus exact_frame_health_history_status(uint32_t id);
ExactFrameHealthHitch exact_frame_health_hitch(uint32_t id, uint32_t index);
void exact_frame_health_applied(uint32_t id, ExactFrameHealthWork work, uint64_t flags);
void exact_frame_health_requested_period(uint32_t id, uint64_t period);
void exact_frame_health_context(uint32_t id, ExactFrameHealthWork work);
typedef struct ExactFrameHealthDistribution { uint32_t bins[64]; } ExactFrameHealthDistribution;
ExactFrameHealthDistribution exact_frame_health_distribution(uint32_t id, uint32_t kind);
ExactFrameHealthSnapshot exact_frame_health_snapshot(uint32_t id);
void exact_frame_health_stop(uint32_t id);
void exact_frame_health_reset(uint32_t id);
void exact_frame_health_destroy(uint32_t id);

/* Raster accounts outlive replaceable runtimes. Handles are process-unique;
 * charge/lease release is worker-safe and never requires a live Runtime. */
typedef struct ExactRasterDemand {
    uint64_t view, view_generation, source, generation;
    uint32_t width, height, natural_width, natural_height;
    uint32_t priority; /* 0 visible, 1 overscan */
    uint32_t variant;  /* storage (LLP 1100 D7): 1 sRGB8, 2 own-space 8, 3 deep 16F, 4 HDR 16F, 5 reduced 8 */
    uint64_t encoded_bytes, header_bytes, stride, scratch_bytes;
} ExactRasterDemand;
typedef struct ExactRasterWork {
    uint64_t permit, session, source, generation, charge;
    uint32_t width, height, variant;
} ExactRasterWork;
typedef struct ExactRasterReady { uint64_t lease, payload; } ExactRasterReady;
typedef struct ExactRasterStats {
    uint64_t resident_bytes, reserved_bytes, pinned_bytes, cold_bytes, retiring_bytes, peak_bytes;
    uint64_t queued, running, ready, delivery_cells, pending_jobs, subscribers, cold_entries;
    uint64_t dedup_hits, cancelled, evicted, process_running, last_refusal, waiting_budget;
} ExactRasterStats;
/* A session holding up to `budget` decoded bytes (at least 32 MiB): what
   views pin plus a cache of what they left. */
uint64_t exact_raster_session_create(uint64_t budget);
/* Update capacity without releasing displayed or in-flight backings. */
void exact_raster_session_budget(uint64_t session, uint64_t budget);
/* 0 reset, 1 pause, 2 resume, 3 shutdown, 4 trim, 5 wake a worker (a source to read). */
void exact_raster_session_control(uint64_t session, uint32_t op);
uint64_t exact_raster_request(uint64_t session, ExactRasterDemand demand);
void exact_raster_cancel(uint64_t session, uint64_t request);
uint32_t exact_raster_status(uint64_t session, uint64_t request);
/* The gate's wakes so far: read before a worker looks for a source to read,
   and passed to next_decode, which then returns at once at any wake since. */
uint64_t exact_raster_wake_sequence(void);
ExactRasterWork exact_raster_next_decode(uint32_t timeout_ms, uint64_t since);
uint32_t exact_raster_is_cancelled(uint64_t permit);
/* Transfers one retained immutable native payload, even on stale completion.
 * Scratch must already be gone. release(payload) may run on any thread. */
uint32_t exact_raster_complete(uint64_t permit, uint64_t payload, void (*release)(uint64_t), uint64_t bytes);
void exact_raster_fail(uint64_t permit);
void exact_raster_charge_release(uint64_t charge);
ExactRasterReady exact_raster_take_ready(uint64_t session, uint64_t request);
void exact_raster_lease_release(uint64_t lease);
ExactRasterStats exact_raster_stats(uint64_t session);

/* An SVG file as an image (LLP 1011.001 D4). header: 1 when bytes begin an SVG,
 * writing its natural size (CSS pixels) into size[0..2]; else 0. scene: the
 * file resolved at width x height CSS pixels as the svg op's JSON, in a buffer
 * of *out_len bytes to free with exact_svg_image_free; *flags bit 0 means the
 * picture has text (drawn on the main thread, where the fonts are), bit 1 that
 * the file was refused, the buffer then holding the reason. */
uint32_t exact_svg_image_header(const uint8_t *bytes, size_t len, double *size);
uint8_t *exact_svg_image_scene(const uint8_t *bytes, size_t len, double width, double height, size_t *out_len, uint32_t *flags);
void exact_svg_image_free(uint8_t *ptr, size_t len);

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
    float font_stretch;    /* CSS font-stretch, percent of normal width */
    uint8_t variation_count;      /* CSS font-variation-settings: axes set */
    uint32_t variation_tags[8];   /* each a big-endian OpenType tag ('wght') */
    float variation_values[8];    /* in the axis's own units */
    uint8_t optical_sizing_none;  /* 1 for CSS font-optical-sizing: none */
    /* LLP 1011.002: 1 when the run is an image in the paragraph's text, its text U+FFFC
     * and its box these points wide and tall, raised box_shift from where box_align
     * (0 baseline, 1 middle, 2 text-top, 3 text-bottom, CSS vertical-align) puts it. */
    uint8_t inline_box;
    uint8_t box_align;
    float box_width, box_height, box_shift;
} ExactTextRun;

/* Evidence of the host's local-network fact from @exact/local-network
 * (LLP 1133.036 D8): 0 granted, 2 denied. Returns 1 if the fact moved. */
uint32_t exact_local_network_report(uint32_t state);

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

/* LLP 1053.000 D4. This platform's name (platform 0 iOS, 1 macOS) for a backgroundMaterial:
 * its length and static bytes at *out, `~` first when another is drawn in its place; 0 when
 * the schema has no such material. */
size_t exact_material_platform(const uint8_t *name, size_t len, uint8_t platform, const uint8_t **out);

/* LLP 1066. Resolve length stops/repetition for a painted box. UTF-8 CSS in,
 * JSON out; returns bytes needed, writing nothing when cap is too small. */
size_t exact_gradient(const uint8_t *css, size_t len, float width, float height,
                      uint8_t dark, uint8_t *out, size_t cap);

/* LLP 1077 D1. A box outline with shaped corners as one closed polygon: `shape` 4 K values
 * (NaN is -exact-continuous), `radii` 8 (top-left first, horizontal then vertical, reduced).
 * Writes x,y pairs into `out` when `cap` holds them all; returns the point count. */
size_t exact_corner_outline(const float *shape, float x, float y, float width, float height,
    const float *radii, float *out, size_t cap);

/* Intern a CSS calculation tree; return 0 for invalid or compact lengths.
 * The returned handle has no environment; exact_calc_create supplies it. */
uint32_t exact_calc_parse(const uint8_t *text, size_t len);

/* A style owns this environment snapshot until destroy. Values: top, right, bottom, left,
 * viewport width/height (the window's when a sheet is open), segment cols/rows, the layout
 * regions' count (0 or 20) and the regions (LLP 1123 D10), the title-bar area's count (0 or 4)
 * and its x,y,w,h, then x,y,w,h per segment. create copies them and returns NULL for malformed input or an unknown handle.
 * resolve borrows a live context; destroy consumes it exactly once (NULL is allowed). */
void *exact_calc_create(uint32_t handle, const double *values, size_t count);
double exact_calc_resolve(const void *context, double basis);
void exact_calc_destroy(void *context);

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
/* Where a line may end in a paragraph (Chrome's opportunities, the walker's):
 * ascending UTF-16 offsets, the last its length. Words as in prepare. Writes
 * min(count,cap) to out (null is a query); returns count, 0 for invalid UTF-8. */
size_t exact_text_line_breaks(const uint8_t *utf8, size_t len,
    const uint32_t *words, size_t word_count, uint32_t *out, size_t cap);

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
    uint8_t white_space;   /* 0 normal, 1 pre-wrap, 2 nowrap, 3 pre-line, 4 pre; runs arrive collapsed unless 1 or 4 */
    uint8_t direction;     /* 0 ltr, 1 rtl */
    const ExactFlowShape *exclusions;
    size_t exclusion_count;
    uint8_t markup;        /* 1: the one run is Markdown source; expand it with exact_markup_pieces (LLP 1045 D3) */
    float text_indent;     /* CSS text-indent, points: the first line's inset from its start edge */
    uint8_t hyphens;       /* CSS hyphens: 0 manual (the initial value), 1 none (soft hyphens already arrive as U+034F), 2 auto */
    const uint8_t *lang;   /* the document language, UTF-8 (auto's hyphenation points); lang_len 0 is unknown */
    size_t lang_len;
    uint8_t text_box_trim;  /* CSS text-box-trim: 0 none, 1 trim-start, 2 trim-end, 3 trim-both */
    uint8_t text_box_over;  /* text-box-edge over: 0 text, 1 cap, 2 ex */
    uint8_t text_box_under; /* text-box-edge under: 0 text, 1 alphabetic */
    uint8_t text_wrap_style; /* CSS text-wrap-style: 0 auto, 1 balance (LLP 1001 §6) */
    const uint8_t *atoms; size_t atoms_len; /* LLP 1045.000 D2: a Markdown paragraph's `atoms` JSON, UTF-8; 0 length for none */
    const uint8_t *faces; size_t faces_len; /* its `atomFaces` JSON */
} ExactMeasureRequest;

/* LLP 1045 D3/D4. Markdown source into display pieces, the same for measure and paint. */
typedef struct ExactMarkupPiece {
    const uint8_t *text; size_t len;   /* UTF-8, not NUL-terminated; may end in a newline */
    float scale;                        /* font size relative to the node's */
    uint16_t font_weight;               /* CSS weight; 0 keeps the node's */
    uint8_t italic, mono, strike;
    uint8_t role;                       /* 0 ink, 1 code, 2 link, 3 marker, 4 quote */
    const uint8_t *href; size_t href_len; /* a link's target; null when none */
    float indent;                       /* CSS px: the head indent of the paragraph it is in (a list item's) */
    uint8_t hang;                       /* 1: a list marker, hung before the indent, its end at it */
    const uint8_t *atom; size_t atom_len; /* LLP 1045.000 D6: an inline atom's JSON {kind,role,icon,title,href,source,start,end,color,background}, its text the label, its colours its face's paint in the colour wire form (null for the kind's); null otherwise */
} ExactMarkupPiece;
/* LLP 1045.000 D2: a node's `atoms` and `atomFaces` JSON, parsed once; pass the handle (0 for none)
 * to the calls below that take `atoms`, and free it with exact_markup_free. */
uint64_t exact_markup_atoms(const uint8_t *kinds, size_t kinds_len, const uint8_t *faces, size_t faces_len);
/* Writes the pieces and their count, valid until exact_markup_free(handle). Zero on invalid UTF-8. */
uint64_t exact_markup_pieces(const uint8_t *utf8, size_t len, uint64_t atoms, const ExactMarkupPiece **out, size_t *count);
/* Styling for an editor, as JSON (see host/apple/src/markup.rs `style`): paragraphs, spans, hidden
 * ranges, replacements and link destinations over the source in UTF-16 units; the selection reveals
 * markers it touches, never a link destination (sel_start UINT32_MAX for none). Valid until
 * exact_markup_free(handle). */
uint64_t exact_markup_style(const uint8_t *utf8, size_t len, uint64_t atoms, uint32_t sel_start, uint32_t sel_end, const uint8_t **out, size_t *count);
/* UTF-16 replacement JSON: {replacements:[[start,end,text],...],selection:[start,end]}.
 * Invalid commands return {error:...}; the original source is unchanged. */
uint64_t exact_markup_edit(const uint8_t *utf8, size_t len, uint32_t start, uint32_t end,
    const uint8_t *command, size_t command_len, const uint8_t *argument, size_t argument_len,
    const uint8_t **out, size_t *count);
/* {formats:string,mixed:bool,link:string,unavailable:string,atom:string}; token lists use spaces. */
uint64_t exact_markup_selection(const uint8_t *utf8, size_t len, uint64_t atoms, uint32_t start, uint32_t end,
    const uint8_t **out, size_t *count);
/* Raw UTF-8 plain text, sharing the same handle lifetime. */
uint64_t exact_markup_plain(const uint8_t *utf8, size_t len, uint64_t atoms, uint32_t start, uint32_t end, const uint8_t **out, size_t *count);
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
    uint16_t weight;       /* CSS weight, or the low end of a variable face's range */
    uint16_t weight_max;   /* the range's high end; equal to weight for one weight */
    uint16_t stretch_min;  /* widths in tenths of a percent; 1000 is normal */
    uint16_t stretch_max;
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
/* LLP 1104: UIKit's body font, installed in the host registry before layout. */
typedef struct {
    const uint8_t *family; size_t family_len;
    uint16_t family_id; float size; uint16_t weight; uint8_t italic;
} ExactControlFont;
/* kind: 0 field/textarea, 1 mini, 2 small, 3 medium/default, 4 large button. */
typedef ExactControlFont (*ExactControlTextFn)(void *ctx, uint8_t kind);
/* kind: 0 field, 1 secure, 2 search, 3 textarea; plus 4 for a field in a
 * grouped list's row (LLP 1116.005 D2), borderless in its cell on iOS. */
typedef struct {
    uint8_t kind; uint16_t family_id; float size; uint16_t weight; uint8_t italic;
} ExactFieldChromeRequest;
typedef struct {
    float top, right, bottom, left, minimum_height; uint8_t provisional;
} ExactFieldChrome;
typedef ExactFieldChrome (*ExactFieldChromeFn)(void *ctx, const ExactFieldChromeRequest *request);
void exact_set_control_text(ExactRuntime rt, ExactControlTextFn text, ExactFieldChromeFn chrome);
/* LLP 1069.011.001 D11: identical face/row JSON for sizing and drawing. */
typedef struct {
    const uint8_t *face; size_t face_len; uint8_t width_kind; float width;
} ExactButtonMeasureRequest;
typedef struct { float width, height; uint8_t provisional; } ExactButtonMeasure;
typedef ExactButtonMeasure (*ExactButtonMeasureFn)(void *ctx, const ExactButtonMeasureRequest *request);
void exact_set_button_measure(ExactRuntime rt, ExactButtonMeasureFn measure);
uint32_t exact_control_text_changed(ExactRuntime rt);

typedef struct {
    double datetime, now, relative_value;
    uint32_t relative_unit;
    double utc_offset;
    const uint8_t *format; size_t format_len;
    const uint8_t *locale; size_t locale_len;
    const uint8_t *time_zone; size_t time_zone_len;
} ExactTimeRequest;
/* Return the UTF-8 byte count, writing at most cap bytes. Zero: fallback children. */
typedef size_t (*ExactTimeFormatFn)(void *, const ExactTimeRequest *, uint8_t *, size_t);
void exact_set_time_format(ExactRuntime rt, ExactTimeFormatFn format);
void exact_set_measure(ExactRuntime rt, ExactMeasureFn measure, void *ctx);   /* NULL: a monospace reference measurer */
/* LLP 1056 D8: one Canvas 2D run measured with Core Text where the draw
 * runs, with the context exact_set_measure was given. The strings live for
 * the call. v: width, left, right, ascent, descent (ink, from the run's left
 * alphabetic origin), font ascent, font descent, em ascent, em descent,
 * hanging, ideographic — CSS px. */
typedef struct ExactCanvasText {
    const uint8_t *text; size_t len;
    const uint8_t *families; size_t families_len; /* names joined by ',' */
    double size, stretch, letter_spacing, word_spacing;
    uint16_t weight;
    uint8_t style;   /* 0 normal, 1 italic, 2 oblique */
    uint8_t caps;    /* fontVariantCaps index */
    uint8_t kerning; /* 0 auto, 1 normal, 2 none */
    uint8_t rtl;
} ExactCanvasText;
typedef struct ExactCanvasMetrics { double v[11]; } ExactCanvasMetrics;
typedef ExactCanvasMetrics (*ExactCanvasTextFn)(void *ctx, const ExactCanvasText *run);
void exact_set_canvas_text(ExactRuntime rt, ExactCanvasTextFn measure);
/* LLP 1093 D6: each line box's bottom, in content coordinates, of the paragraph a request
 * answers, with exact_set_measure's context. Writes min(count, cap) floats and returns count.
 * NULL (the default) keeps every paragraph whole in a multi-column flow. */
typedef size_t (*ExactLinesFn)(void *ctx, const ExactMeasureRequest *request, float *out, size_t cap);
void exact_set_lines(ExactRuntime rt, ExactLinesFn lines);
/* LLP 1011.002 D2: where the paragraph a request answers put each inline box (a run with
 * inline_box set), its top left in content coordinates: x then y, one pair per box in run
 * order. Writes min(2 * count, cap) floats and returns 2 * count, with exact_set_measure's
 * context. NULL (the default) leaves each image at the paragraph's content origin. */
void exact_set_inline_boxes(ExactRuntime rt, ExactLinesFn boxes);
/* A system symbol's natural size, measured before layout: an Apple symbol name (UTF-8, not
 * NUL-terminated), its point size and CSS weight. Writes width and height to out and returns 1,
 * or 0 when it cannot say; called with exact_set_measure's context, and only beside a text
 * measurer it registered: with the reference measurer, as with NULL (the default), a symbol is
 * the size its view reports. */
typedef uint8_t (*ExactSymbolFn)(void *ctx, const uint8_t *name, size_t len, float font_size, uint16_t font_weight, float *out);
void exact_set_symbol_measure(ExactRuntime rt, ExactSymbolFn measure);
void exact_set_wake(ExactRuntime rt, ExactWakeFn wake, void *ctx);
void exact_set_fonts(ExactRuntime rt, ExactFontsFn fonts, void *ctx);

/* The session's app module (LLP 1067.000). later(ctx, body, len, reply) is
 * called on the executor's thread with a long native call's JSON body; the
 * host dispatches it to the main thread and returns. Each reply is answered
 * exactly once, from any thread, with exact_app_reply: status 200 carries
 * the JSON answer, any other status a refusal message. call(ctx, body, len,
 * slot) is a native.call on the source's thread: the host answers it on the
 * main thread with exact_app_answer before returning. exact_app_changed
 * announces a device topic (LLP 1016.002), on the main thread. */
typedef void (*ExactAppLaterFn)(void *ctx, const uint8_t *body, size_t len, void *reply);
typedef void (*ExactAppCallFn)(void *ctx, const uint8_t *body, size_t len, void *slot);
void exact_set_app_module(ExactRuntime rt, ExactAppLaterFn later, ExactAppCallFn call, void *ctx);
void exact_app_answer(void *slot, uint32_t status, const uint8_t *bytes, size_t len);
void exact_app_changed(ExactRuntime rt, const uint8_t *topic, size_t len);
void exact_app_reply(void *reply, uint32_t status, const uint8_t *bytes, size_t len);

/* A document the app kept was forgotten by the app (LLP 1069.010.000 D5):
 * called on a storage worker's thread with the object's device and inode;
 * the host takes it off NSDocumentController's recents on the main thread.
 * One hook for the process. */
typedef void (*ExactForgottenFn)(uint64_t device, uint64_t inode);
void exact_on_document_forgotten(ExactForgottenFn forgotten);

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
/* Snapshot-only boot: concatenated plan bytes, UTF-8 kind and optional props JSON.
 * Pure boot reads no store or secrets. Timeline refresh is the only data step. */
uint32_t exact_boot_entry(ExactRuntime rt, size_t plan, size_t kind, size_t props, float width, float height);
uint32_t exact_entry_timeline(ExactRuntime rt, double now_ms, double remaining_ms, const uint8_t *receipt, size_t receipt_len, const uint8_t *module, size_t module_len);
/* Gallery-only query: a fresh data seam and scratch store, no host activation. */
uint32_t exact_refresh_entry(ExactRuntime rt, double now_ms);
uint32_t exact_boot_plan(ExactRuntime rt, size_t len, float width, float height);
/* First root's literal viewport-fit=cover, without settlement. Input plan
 * bytes of length len, or the baked plan when len is zero. */
uint32_t exact_plan_covers_viewport(ExactRuntime rt, size_t len);
uint32_t exact_prepare_plan(ExactRuntime rt, uint64_t token, size_t len, float width, float height);
/* Check every participant and commit in one owner job, without intervening turns. */
uint32_t exact_check_prepared(ExactRuntime rt);
uint32_t exact_commit_plan(ExactRuntime rt);
/* Admitted generation: concatenated plan, pairing receipt, compiled module.
 * A nonzero token supplies signed delivery facts. Identity and grants match
 * the binary; the caller admits the development origin or signed assets.
 * Requests in flight refuse it (`"requests":true`) unless let_go, nonzero
 * only for a dev generation past its settle window (LLP 1007 §6). */
uint32_t exact_prepare_module(ExactRuntime rt, uint64_t token, size_t plan, size_t receipt, size_t module, float width, float height, uint32_t let_go);
/* After first pixel: activate the data source (evaluate a TypeScript module)
 * without committing, so a presenter can run it off its own thread before
 * exact_data_ready. Optional; a no-op once activated. */
void exact_prepare_data(ExactRuntime rt);
/* Call after first pixel, never as a prerequisite to painting the baked frame. */
uint32_t exact_data_ready(ExactRuntime rt);
/* A peer committed to the shared store. Does not activate before first pixel. */
uint32_t exact_store_changed(ExactRuntime rt, uint32_t retry_only);
void exact_discard_plan(ExactRuntime rt);
/* Content has settled (a scroll came to rest): the tree and the motion
 * engine give back storage beyond the live nodes. Changes nothing shown. */
void exact_trim(ExactRuntime rt);
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
/* NUL-delimited UTF-8 href, declared protocol, and path template; 0 if no match. */
uint32_t exact_protocol_location_of(ExactRuntime rt, size_t len);
/* LLP 1038 §7: 1 when the input location names a declared route (a followed
 * link to it navigates in the app), else 0. */
uint32_t exact_route_matches(ExactRuntime rt, size_t len);
uint32_t exact_set_launch_location(ExactRuntime rt, size_t len);
/* LLP 1115 D5: the location of the visit beneath visit `id` on its stack,
 * UTF-8 in exact_out, length 0 when none — where the host's own Back goes
 * for a route with no authored Back control. Not a batch. */
uint32_t exact_location_beneath(ExactRuntime rt, uint64_t id);
/* A retained visit's canonical location, UTF-8 in exact_out; 0 when absent. */
uint32_t exact_visit_location(ExactRuntime rt, uint64_t id);
/* Return from source to an earlier destination in the selected stack, one commit. A batch. */
uint32_t exact_host_back_to(ExactRuntime rt, uint64_t source, uint64_t destination, double now_ms);
/* LLP 1133.016 D15 item 1: a package's call of the app's handler, the JSON
 * {"extension","handle","handler","args"} in the input buffer. The runner's
 * answer, JSON ({"text"} or {"error","message"}), goes to reply(ctx, answer,
 * len) once: during this call, or when the handler's own requests land on
 * the runner's thread. Not a batch. */
typedef void (*ExactInvokeReply)(void *ctx, const uint8_t *answer, uint32_t len);
void exact_module_invoke(ExactRuntime rt, size_t len, ExactInvokeReply reply, void *ctx);
/* LLP 1115 D5: the platform's own Back from visit `id` for a route with no
 * authored Back control under a root with no `navigate` handler: the
 * router's `back` as a commit of its own. A batch. */
uint32_t exact_host_back(ExactRuntime rt, uint64_t id, double now_ms);
/* The runner's own go to the UTF-8 location in exact_in, a batch. */
uint32_t exact_host_go(ExactRuntime rt, size_t len, double now_ms);
/* LLP 1133.024 D2-D4: a received share, the JSON in exact_in
 * ({"root", "target", "share", "inbox"?}), a batch; before the boot its
 * location and tokens become the launch's, and exact_share_land, after
 * storage is configured, lands its files (a batch). */
uint32_t exact_share(ExactRuntime rt, size_t len, double now_ms);
uint32_t exact_share_land(ExactRuntime rt);
/* kind: 0 = press, 1 = change, 2 = hover in, 3 = hover out, 4 = focus,
 * 5 = blur, 6 = key, 7 = submit, 8 = iframe load, 9 = iframe message,
 * 10 = contextmenu, 11 = dblclick, 12 = swiperight, 13 = scroll (UTF-8 scrollLeft,scrollTop),
 * 14 = navigate (UTF-8 location; navigation root only, LLP 1038 D8),
 * 15 = heightrelease, 16 = transformgeometry, 17 = transformrelease,
 * 18 = reorder (collection move payload),
 * 20 = pan (UTF-8 dx,dy; incremental viewport CSS pixels, LLP 1043.000 D8),
 * 19 = media (UTF-8 event name, newline, payload; numeric times in seconds),
 * 21 = select (formats + newline + mixed 0/1 + newline + unavailable + newline + link);
 * 22 = refresh (the platform's pull-to-refresh control fired; no payload);
 * 28 = panrelease (UTF-8 vx,vy; px/s, once when a pan that began ends; a
 *      cancelled contact releases at 0,0; LLP 1057 §10.6);
 * 29 = pointerdown, 30 = pointerup, 31 = pointermove (UTF-8
 *      offsetX,offsetY,buttons,pressure,pointerType,pointerId,clientX,clientY:
 *      content-box CSS px, DOM's buttons bits, 0 to 1, mouse|pen|touch, then
 *      the viewport point; LLP 1056 §3 stage 3, LLP 1094 D11);
 * 40 = a text field's input, 41 its change, 42 its select (UTF-8
 *      start,end,direction,text: UTF-16 offsets, forward|backward|none, then
 *      the whole value verbatim; x2apps codeedit #2);
 * 48 = range replacement input (same payload as 40, inputType insertReplacementText);
 * 49 = close (dialog); 50 = toggle: UTF-8 oldState,newState (open or closed).
 * 6 = key (keydown), 43 = keyup and 44 = keycapture (#140): UTF-8 chord (`Shift+Meta+b`),
 *      optionally newline, KeyboardEvent.code (`KeyB`, "" unknown), newline,
 *      true|false for repeat, and optionally newline, true|false for isComposing;
 * 46 = focusin, 47 = focusout (#283): UTF-8 `<target byte length>:<target id>`
 *      then `0` (no relatedTarget) or `1<related id>`;
 * any other kind is refused with an error batch.
 * Format lists are space-separated command tokens. Link keeps the remaining bytes.
 * A change's text, key's name, or guest message is the payload in the input
 * buffer's first len bytes. */
uint32_t exact_dispatch(ExactRuntime rt, uint32_t view, uint32_t kind, size_t len, double now_ms);
/* A scroll container the presenter shows (or, nonzero page, the page) now
 * stands at left, top CSS px (scrollLeft, scrollTop), handler or not: what
 * frame() and measure() subtract from the kernel's scroll-free box, so an
 * action reads the box where the viewer sees it (LLP 1051.000 D1). No batch. */
void exact_scrolled(ExactRuntime rt, uint32_t page, uint32_t view, double left, double top);
/* An inline run's shaped fragments, united in viewport coordinates. Called
 * synchronously on the runtime owner; UI work is served on main. */
typedef struct { double x, y, width, height; uint32_t available; } ExactInlineFrame;
typedef ExactInlineFrame (*ExactInlineFrameFn)(void *ctx, uint32_t view);
typedef ExactInlineFrame (*ExactRangeFrameFn)(void *ctx, uint32_t view, uint32_t start, uint32_t end);
void exact_set_inline_geometry(ExactRuntime rt, ExactInlineFrameFn read, ExactRangeFrameFn range, void *ctx);
/* A textarea's decorations (LLP 1114.007): UTF-8 JSON {record, current, styles} in,
 * {provisional, spans, styles} out, valid until the next call on this thread; null when
 * the request does not parse. */
const uint8_t *exact_field_decorations(const uint8_t *input, size_t len, size_t *count);
/* Open top-layer views in back-to-front order, with viewport x/y/width/height
 * in four doubles per id: where the host shows each, a block's root by its
 * block's rectangle (the root sits in it by its own frame) and any other view
 * by its border box, before its own CSS transform, which the runner applies.
 * Structural reads use their actual presentation. `modal` is the index of
 * the first view of the frontmost modal presentation, or UINT32_MAX while
 * none is open: it and the views after it are live, and the views before it
 * and the page are inert behind it. `clips` is null or four viewport
 * x/y/width/height doubles per id; NaN at x means no clip for that entry.
 * A clip bounds hits and visibility without changing the frame's mapping. */
void exact_top_layer(ExactRuntime rt, const uint32_t *views, const double *rects, const double *clips, uint32_t count, uint32_t modal);
/* Versioned LE collection feedback in exact_in; returns the ordinary batch. */
uint32_t exact_collection_feedback(ExactRuntime rt, size_t len, double now_ms);
/* The agent's tap <list> into <key> (LLP 1070.000): "key\nblock\ninline" in
   exact_in; returns the ordinary batch. */
uint32_t exact_into_view(ExactRuntime rt, uint32_t view, size_t len);
/* What UIKit covers of a virtualized list's port at its start and end, in
   points: a row the runner aligns (scrollIntoView, a focus) lands clear of it. */
void exact_list_covered(ExactRuntime rt, uint32_t view, double start, double end);
/* Property: 0 translate, 1 scale, 2 rotate, 3 opacity. Begin replies with a
 * hold op {token:decimal-string,x,y}. Tokens belong to this runtime incarnation.
 * Check liveness before an authored action; final update, action, then end. */
/* Authored header binding, generational keys and live Height token. */
uint32_t exact_height_drag_begin(ExactRuntime rt, uint64_t handle_key, uint64_t target_key, double now_ms);
uint32_t exact_height_drag_update(ExactRuntime rt, uint64_t token, double height, double now_ms);
/* The release velocity is the engine's, over the heights shown (LLP 1057.001 §3). */
uint32_t exact_height_drag_release(ExactRuntime rt, uint64_t token, double height, double now_ms);
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
/* Dropping across lists (reorderGroup, LLP 1094 D5-D9). A grouped grip lifts
 * with group_begin: ghost nonzero when the host draws the row in its top
 * layer (the runner then hides the row itself until group_finish); zero for
 * a key's or custom action's session. move_into hands the ghost centre's y in
 * target's (a grouped list view) content, at target_scroll_top as its
 * collection feedback reports it; inside zero (the centre in no grouped
 * port) keeps the certified gap. step: 1 earlier, 2 later, 3 the previous
 * grouped list, 4 the next. group_end drops into the session's target
 * (nonzero) or cancels; a holding drop ignores a cancel. Every reply, and
 * every later batch that changes it, carries {"op":"reorder","group":true,
 * "token","list","wrapper","phase":"active"|"holding"|"cancelling"|
 * "settling"|"finished"|"refused","dispatched","ending":null|"landed"|"gone"|
 * "timeout","target","row"}: row is the wrapper that holds the dragged row
 * now, where a ghost lands. A new lift is refused until group_finish. */
uint32_t exact_reorder_group_begin(ExactRuntime rt, uint32_t handle, double scroll_top, uint32_t ghost, double now_ms);
uint32_t exact_reorder_move_into(ExactRuntime rt, uint64_t token, uint32_t target, double content_y, double target_scroll_top, uint32_t inside, double now_ms);
uint32_t exact_reorder_step(ExactRuntime rt, uint64_t token, uint32_t step, double now_ms);
uint32_t exact_reorder_group_end(ExactRuntime rt, uint64_t token, uint32_t drop, double now_ms);
uint32_t exact_reorder_group_finish(ExactRuntime rt, uint64_t token, double now_ms);
uint32_t exact_hold_begin(ExactRuntime rt, uint32_t view, uint32_t property, double now_ms);
uint32_t exact_has_hold(ExactRuntime rt, uint64_t token);
uint32_t exact_hold_update(ExactRuntime rt, uint64_t token, double x, double y, double now_ms);
/* cancel: 0 releases at vx, vy; 1 cancels; 2 releases at the velocity the
 * engine measured over the hold's values (LLP 1057.001 §3). */
uint32_t exact_hold_end(ExactRuntime rt, uint64_t token, uint32_t cancel, double vx, double vy, double now_ms);
/* A recognition threshold exact2 defines itself: 0 the swipe knee, 1 its
 * resistance, 2 the leading edge a swipe yields, 3 the drag slop (LLP 1057.001 §3). */
double exact_gesture_constant(uint32_t which);
/* A route's header as its bar shows it (LLP 1115.001 D11): the kernel's rule
 * (kernel/src/bars.rs), which the Android host reads from the kernel's own
 * tree, over this host's mirror of the route. words: the route, then its first
 * child's subtree, in preorder, two words a node: its facts and the count of
 * its children. Facts: 1 a heading (a text whose role is heading), 2 an
 * input type="search", 4 a tablist, 8 it hears press, 16 a button that opens a
 * popover, 32 its id is the stack's Back control's, 64 a header, 128
 * display: none, 256 a route that says navigationBar="none"; a heading's
 * aria-level (1 to 6, else 0) shifted left 16. flags: 1 the platform's own Back
 * stands for the Back control on this route, 2 the header as the bar is
 * projected from it (none for a route that says no bar, or a hidden header).
 * Writes to out, naming nodes by their place in words: the header, the heading,
 * its level, one more than the pressable around the heading, the search field
 * and the tablist (0: none), the counts of leading items, trailing items and
 * all items, then those three lists. Returns the count written; 0 for a route
 * that is not header-shaped, or an answer longer than cap. No runtime. */
uint32_t exact_header_shape(const uint32_t *words, size_t count, uint32_t flags, uint32_t *out, size_t cap);
/* What a route shows of the platform's bar (LLP 1115.000 D5.2): 0 its header,
 * 1 the platform's Back alone, 2 nothing. facts: 1 the route says
 * navigationBar="none", 2 its header is the bar's, 4 it is pushed, 8 the
 * platform's bar has a Back button. */
uint32_t exact_route_bar(uint32_t facts);
/* The pan contact's release velocity where the platform measures none (AppKit,
 * the iOS agent's recognized contact; LLP 1057 §10.6, 1057.001 §3): the engine's
 * tracker, one per runtime. A sample is the pointer in viewport CSS px at t
 * seconds (any monotonic origin, e.g. NSEvent.timestamp); nonzero first starts
 * a contact. Returns 1 taken, 0 refused (non-finite; a first still resets). */
uint32_t exact_pan_sample(ExactRuntime rt, uint32_t first, double x, double y, double t);
/* The velocity along axis (0 x, 1 y) at t seconds, px/s: 0 with fewer than two
 * samples in the window, for a non-finite t, another axis, or a dead runtime. */
double exact_pan_velocity(ExactRuntime rt, uint32_t axis, double t);
/* The runner's clock: timers. Nonzero until_request stops after a timer that
 * sends, the clock at its due time (an agent's jump; the wall clock passes 0). */
uint32_t exact_advance(ExactRuntime rt, double now_ms, uint32_t mode);
/* @ref LLP 1073 D5: a presented display frame — timers due by now_ms, then
 * every frame task once at it. The batch says "frames" while one wants it. */
uint32_t exact_frame(ExactRuntime rt, double now_ms);
/* @ref LLP 1003.001 D5: exact_frame at the target now_ms, the wall at wall_ms
 * stopping the motion engine's input clock. */
uint32_t exact_frame_at(ExactRuntime rt, double now_ms, double wall_ms);
/* @ref LLP 1003.001 D7: nonzero, motion a commit begins waits for the first
 * presented frame, in every host booted after; zero at the agent's takeover,
 * where what waits starts at at_ms. Returns the batch's length. */
uint32_t exact_start_on_frame(ExactRuntime rt, uint32_t on, double at_ms);
/* Whether the display drives frame tasks: exact_frame turns it on; 0 when the
 * agent's clock takes over, whose advances then fire virtual frames. */
uint32_t exact_present_frames(ExactRuntime rt, uint32_t on);
/// The presenter's clock owns time (`wall`, milliseconds in the runner's
/// domain), or the agent's seekable one does (NULL): the timers RFC, D1.
typedef double (*ExactWallFn)(void);
uint32_t exact_set_live_clock(ExactRuntime rt, ExactWallFn wall);
/* Input: name alone clears; name NUL JSON publishes a current record. */
uint32_t exact_surface_record(ExactRuntime rt, size_t len);
/* @ref LLP 1027.000.000: the date — Unix ms at clock zero, minutes east of UTC. */
uint32_t exact_set_time(ExactRuntime rt, double epoch_at_zero, double utc_offset);
/* The date the next boot or candidate settles under, told before it; the
 * running runner is not told. A date that does not validate is ignored. */
uint32_t exact_set_launch_time(ExactRuntime rt, double epoch_at_zero, double utc_offset);
/* Where the viewer is, as exact_set_place's input (locale NUL timeZone NUL
 * seed, then optionally NUL formatLocale), for the next boot or candidate; the running runner is not told.
 * Returns zero when accepted, otherwise an error batch length; keeps the last valid fact. */
uint32_t exact_set_launch_place(ExactRuntime rt, size_t len);
/* Beside the date: the locale and IANA time zone, as locale NUL timeZone input,
 * then optionally NUL seed NUL formatLocale. */
uint32_t exact_set_place(ExactRuntime rt, size_t len);
/* @ref LLP 1126.001 WP9: declare the packaged arm before boot, then deliver
 * its source snapshots or send completions through the input buffer. */
void exact_link_activities(ExactRuntime rt);
uint32_t exact_activities(ExactRuntime rt, size_t len);
/* @ref LLP 1125 D6-D8: a MIDI message, JSON in the input buffer: the MIDI
 * arm's {"ports":…} or {"messages":…}, or the agent's `prefer midi` and
 * `tap midi`. Returns the batch length. */
uint32_t exact_midi(ExactRuntime rt, size_t len);
/* @ref LLP 1124 D7: the agent's `prefer extension`, JSON in the input buffer:
 * a native module source's preferred value. Returns the batch length. */
uint32_t exact_extension(ExactRuntime rt, size_t len);
/* A module event the platform reported (LLP 1133.004 D6): {"extension",
 * "event", "args", "activation"} in the input buffer; the batch's length. */
uint32_t exact_module_event(ExactRuntime rt, size_t len);
/* @ref LLP 1133.001 D4: the extensions the app's module composes, one name a
 * line in the input buffer: the runner sends their names to the module. A
 * line `agent <name>` is an implementation bound under the agent, which
 * answers an interface that is its own substitute (LLP 1133.006 D8). */
void exact_bind_extensions(ExactRuntime rt, size_t len);
/* @ref LLP 1133.019 D11 item 1: an `img` source of a package's scheme, in the
 * input buffer, admitted as a read: the output is {"extension","scheme",
 * "agent","owned"?} to ask that extension's implementation for the picture,
 * {"error","message"} for an unresolved image, or empty for a source of no
 * package's scheme. Never raises a ticket. */
uint32_t exact_picture(ExactRuntime rt, size_t len);
/* @ref LLP 1121 D5: app modes, `axis=value` pins comma-separated (the agent's `prefer mode`); a leading `?` only checks them. */
uint32_t exact_set_mode(ExactRuntime rt, size_t len);
/* @ref LLP 1121 D13, stage 4: a live design batch or snapshot frame from the dev
 * stream (JSON), applied in place; the batch says `live`: applied, stale,
 * structure or refused. Development builds only. */
uint32_t exact_design_rows(ExactRuntime rt, size_t len);
/* @ref LLP 1121 D12, stage 5: the binding core. A request
 * `{"view": id, "names": [token, …]}` (view 0: the session's root) in the
 * input buffer; answers `{"inputs": "…", "colors": {token: colour|null}}`,
 * UTF-8 in exact_out, each colour as a row crosses it, null for a name the
 * plan lacks or a value that is no colour (the binding keeps its fallback).
 * `inputs` moves when anything the answer read does. A view that is not live,
 * or names that are not strings, answer {"error"}. Not a batch. */
uint32_t exact_tamber(ExactRuntime rt, size_t len);
/* The design digest the running app runs (its plan's, or the live table's
 * applied since), UTF-8 in exact_out, length 0 before boot. Not a batch. */
uint32_t exact_design_digest(ExactRuntime rt);
/* The running plan's digest, UTF-8 in exact_out, length 0 before boot. */
uint32_t exact_plan_digest(ExactRuntime rt);
/* @ref LLP 1061 D4, LLP 1069.000 D1: the user's display preferences, told
 * after boot and on each change — bit 0 reduced motion, bit 1 reduced
 * transparency, bit 2 contrast more, bit 3 contrast less, bit 4 a dark system;
 * bits 5–10 input/gamut/range (runner/src/viewport.rs), bits 11–13 display mode
 * (LLP 1039 D7): 0 standalone, 1 browser, 2 fullscreen, 3 minimal-ui,
 * 4 window-controls-overlay, 5 picture-in-picture, 6 tabbed. */
uint32_t exact_set_preferences(ExactRuntime rt, uint32_t bits);
/* @ref LLP 1069.000 D2: the page's facts, told after boot and on each change —
 * bit 0 hidden, bit 1 offline, bit 2 a share sheet. */
uint32_t exact_set_page(ExactRuntime rt, uint32_t bits);
/* 1 when the running plan reads `onLine`, and before boot: only then does
 * the host watch the network. */
uint32_t exact_reads_on_line(ExactRuntime rt);
/* @ref LLP 1069.000 D3: the root font size `rem` follows, in points. */
uint32_t exact_set_root_font_size(ExactRuntime rt, double px);
/* @ref LLP 1021 §5.3: the host's zoom over the root size, the app's own
 * included (the Mac's View ▸ Actual Size, Zoom In, Zoom Out); after boot. */
uint32_t exact_set_root_font_zoom(ExactRuntime rt, double factor);
/* @ref LLP 1115 D6, LLP 1115.000 D13.4: whether a confirmation's rows are an
 * alert's (explanatory text, one cancel, at most three buttons): 1 or 0. The
 * kernel's rule, which lowering and the Linux host read too; needs no runtime. */
uint32_t exact_confirmation_is_alert(uint32_t texts, uint32_t actions, uint32_t cancels);
/* @ref LLP 1115.000 D10.3: the display's scale, device pixels a point, which a
 * hairline (`-exact-hairline`) is one device pixel of; told after boot and when
 * the view moves to another display. */
uint32_t exact_set_display_scale(ExactRuntime rt, double scale);
/* @ref LLP 1039: re-answer viewport facts and relayout in the same batch. */
/// The display's scale and physical memory for Canvas 2D (LLP 1056 D4);
/// callable before boot. Returns the batch length.
uint32_t exact_canvas_display(ExactRuntime rt, double scale, double memory);
/* LLP 1056 D9: a Canvas 2D image handle the batch's "canvasImages" named,
 * decoded (ok 1, its size in pixels) or not (ok 0); the handle is the input
 * buffer's first len bytes. Returns the batch length. */
uint32_t exact_canvas_image(ExactRuntime rt, size_t len, uint32_t width, uint32_t height, uint32_t ok);
/* LLP 1056 D5: a 2D canvas's replay is behind (held 1) or caught up (0);
 * while held, its frame request waits (frames drop rather than queue). */
void exact_canvas_held(ExactRuntime rt, uint32_t view, uint32_t held);
/* LLP 1072 §8.5: canvas draws in a turn of their own (deferred 1): every other
 * turn's batch says "canvasOwed" and exact_canvas_draw runs the draws, off the
 * turns main waits on; a tick draws in its own turn. */
void exact_canvas_defer(ExactRuntime rt, uint32_t deferred);
/* LLP 1079 D1: runtimes booted from now count each plan site's work for perf
 * from the first ask, exact_measure_work, not from boot (a release build's
 * choice). exact_measure_work starts this runtime's count; returns 0. */
void exact_measure_work_later(void);
uint32_t exact_measure_work(ExactRuntime rt);
uint32_t exact_canvas_draw(ExactRuntime rt);
uint32_t exact_resize(ExactRuntime rt, float width, float height);
/* A placement root's resolved margins and its width's authorship (LLP 1126.001 WP10): exact_out
 * holds JSON [margin left, top, right, bottom, authored width 0|1, minimum border-box width] with no batch around it, or
 * {"error": …}; returns its length. */
uint32_t exact_placement_box(ExactRuntime rt, uint32_t view);
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
/* @ref LLP 1123 D10: the layout regions' insets (points) from the layout
 * viewport, what env(-exact-<region>-inset-<edge>) resolves to: count is 20,
 * five regions (safe area with corner adaptation, layout margins, layout
 * margins with corner adaptation, readable content, readable content with
 * corner adaptation) of top, right, bottom, left each; or 0, when every
 * region is the safe area. A change re-sends the style of every node that
 * reads the environment and lays out again. */
uint32_t exact_regions(ExactRuntime rt, const float *insets, uint32_t count);
/* The title-bar area beside the window's controls (points, from the layout
 * viewport), what env(titlebar-area-x|y|width|height) resolves to (Window
 * Controls Overlay): count is 4, x, y, width, height (all zero while the area
 * is hidden), or 0, when the window gives none and each variable is undefined.
 * A change re-sends the style of every node that reads the environment and
 * lays out again. */
uint32_t exact_titlebar_area(ExactRuntime rt, const float *rect, uint32_t count);
/* @ref LLP 1075.003 §9.11: the window's own size (points), whatever is
 * presented in it — what every viewport unit (vw, vh, vmin, vmax and kin) resolves against
 * everywhere, root and every sheet, never a sheet's viewport (exact_segments
 * sends the window's segment grid too, its tops and lefts; a keyboard ends the
 * segments with the viewport, LLP 1078). A nonpositive size clears it. */
uint32_t exact_screen(ExactRuntime rt, float width, float height);
/* What changed of the view at once, from the input buffer's first len bytes,
 * little-endian 32-bit words: a mask of what is given (1 the window's size,
 * 2 the safe-area insets, 4 the layout regions, 8 the viewport, 16 the
 * segments' rects), the window's width and height (nonpositive clears it),
 * the insets top, right, bottom, left, the viewport's width and height, the
 * regions' count (20 or 0) and their insets, then, with 16, the rects' count
 * and each rect's x y w h, on the grid exact_segments set: exact_screen,
 * exact_insets, exact_regions, exact_resize and a keyboard's segment extents
 * in one batch under one layout. Rects off the grid, or not finite, are
 * refused and change nothing. */
uint32_t exact_view(ExactRuntime rt, size_t len);
/* @ref LLP 1078 D4: the device's posture (0 continuous, 1 folded) and the
 * viewport segments a fold makes — cols × rows rects, row-major, each
 * x y w h as four little-endian floats in the input buffer (count rects;
 * none for 1 × 1). Sets the kernel's env(viewport-segment-*) grid and
 * exactViewport's three fields in one batch, as exact_resize sets the
 * viewport and preferences together. */
uint32_t exact_segments(ExactRuntime rt, uint32_t posture, uint32_t cols, uint32_t rows, uint32_t count);
/* The presenter's appearance (nonzero: dark), which a light-dark() colour
 * under paint motion resolves by; a change transitions it (LLP 1062). */
uint32_t exact_scheme(ExactRuntime rt, uint32_t dark);
/* One view's appearance (nonzero: dark), where the presenter finds it
 * differs from the session's: its node's light-dark() colours resolve by it
 * (LLP 1062). */
uint32_t exact_view_scheme(ExactRuntime rt, uint32_t view, uint32_t dark);
/** LLP 1095 D1: every colour reference the presenter should resolve, as JSON
 *  `[[kind, id, "name"], …]` in the output buffer; returns its length. */
uint32_t exact_color_references(ExactRuntime rt);
/** LLP 1095 D1: what the presenter resolved them to, as LE records in the
 *  input buffer (u8 kind, u8 dark, u16 id, u8 r, g, b, a); returns the
 *  batch's length. */
uint32_t exact_colors(ExactRuntime rt, size_t len);
uint32_t exact_tick(ExactRuntime rt, double now_ms);      /* a motion frame, only while "motion" is true */
uint32_t exact_tick_at(ExactRuntime rt, double now_ms, double frame_ms); /* LLP 1003.001 D5: for the frame presented at frame_ms */
/* An image node loaded: its bitmap's pixel counts, taken one-for-one as
 * points (never divided by the backing scale — a 2× asset is not half its
 * pixels wide, as on the web); a width or height ≤ 0 clears it (the load
 * failed, or the source was removed). Lays out again; the batch carries
 * every frame that moved. */
uint32_t exact_intrinsic(ExactRuntime rt, uint32_t view, float width, float height);
/* exact_intrinsic for several views under one layout: the input buffer's
 * first len bytes are LE records of (uint32 view, float width, float height). */
uint32_t exact_intrinsics(ExactRuntime rt, size_t len);
/* What native containers cover of boxes (LLP 1075.003 §3.5) under one
 * layout: LE records of (uint32 view, uint32 kind — 0 clears, 1 edges,
 * 2 whole — float top, right, bottom, left). Edges add to the box's
 * padding; a whole box is laid out as display: none. Kind 6 places a host
 * block: float x, y, width, height, uint32 holding view, then float content
 * scroll offsets x and y (36 bytes including view and kind). Kind 13 uses
 * the same record for a scrollport, including at offset zero; kind 6 ignores
 * the offset. Kind 7 forgets either placement. Kind 10 is a cells
 * list's rows' cell margin (LLP 1138: float left, right, then two unused),
 * 11 clears it. Kind 14 shows a split's `navigationSplit="placeholder"` box
 * and kind 15 hides it again (LLP 1116.006 D2; the floats unused). */
uint32_t exact_host_covers(ExactRuntime rt, size_t len);
/* What a block's root would measure in another block (a popover's
 * position-area option, LLP 1021 D2): each axis a length and a policy as
 * exact_host_covers' kind 4 takes them (0 size, 1 maximum, 2 fit, 3 a
 * position-area's region). The border box's width and height, then the
 * margins (top, right, bottom, left) as the layout resolved them, six LE
 * floats in the output buffer, and 24; 0 when refused. Not a batch:
 * nothing is laid out, published or sent. */
uint32_t exact_measure_block(ExactRuntime rt, uint32_t view, float width, float height, uint32_t width_policy, uint32_t height_policy);
/* A number that moves whenever exact_measure_block could answer otherwise
 * (a commit that changed something, a layout, a reset): a host keeps a
 * measure while it stands. Not a batch. */
uint64_t exact_layout_stamp(ExactRuntime rt);
/* A select's options (LLP 1069.001 D5), JSON in the output buffer, not a
 * batch: {"options":[{"value","label","disabled"}],"chosen":index|null}. */
uint32_t exact_select_options(ExactRuntime rt, uint32_t view);
/* A radio's group (x2apps survey #2), JSON in the output buffer, not a batch:
 * {"group":[view...],"next":view|null,"previous":view|null} — the radios of its
 * name in tree order, and the enabled radio ArrowDown/ArrowUp moves the check to. */
uint32_t exact_radio_group(ExactRuntime rt, uint32_t view);
/* A navigation root's tabs by the kernel's rule (LLP 1115.001 D11), JSON in the
 * output buffer, not a batch: {"tablist":view,"tabs":[view...],"panels":[view...],
 * "unsaid":bool,"covered":bool}, the tabs that name a panel and those panels in tab
 * order, whether the app left the bar's placement unsaid (the web's rule for its
 * sidebar, LLP 1116.004 D1.1), and whether the tablist is covered whole; null for
 * one stack. */
uint32_t exact_root_tabs(ExactRuntime rt, uint32_t view);
/* Whether the stack whose root route is `view` shows that route as a list beside
 * its detail, by the runner's rule (LLP 1115.002 D4), JSON in the output buffer,
 * not a batch: {"list":view|null,"width":number|null,"rows":[[route,row]...],
 * "locations":[[route,url]...],"pressWrites":bool|null}, the list whose row pushes,
 * the list column's width where `navigationSplit` names one, each route above the
 * root that is a row's detail with that row, each route above the root with its
 * location, and whether the press of `pressed` writes the router; null for a stack
 * that pushes. */
/* `retained` is a previous list to keep while empty and still eligible, or 0;
 * `pressed` is a view being pressed, or 0. */
uint32_t exact_stack_split(ExactRuntime rt, uint32_t view, uint32_t retained, uint32_t pressed);
/* A button's face, custom or native (LLP 1069.011.000 D1), JSON in the output
 * buffer, not a batch: {"button":bool,"title":string|null,"symbol":apple-name|null,
 * "raster","leading","fits":bool,"label":string|null,"style",...the native style}. */
uint32_t exact_press_face(ExactRuntime rt, uint32_t view);
/* A grouped list's sections and rows (LLP 1084 D4), JSON in the output
 * buffer, not a batch: {"style","sections":[{"view","header","footer","rows":
 * [{"view","custom","symbol","title","secondary","subtitle","accessory",
 * "target","pressable","destructive","disabled"}]}]}, or null. */
uint32_t exact_grouped_list(ExactRuntime rt, uint32_t view);

/* The agent API (LLP 1012): a request in the input buffer's first len bytes
 * ({"op":"tree"} / "state" / "logs" / "settle"), the reply in the output
 * buffer — JSON, not a batch. */
uint32_t exact_agent(ExactRuntime rt, size_t len);

/* A host line for the runner's journal (LLP 1012 §3; LLP 1035.001 D6): the
 * input buffer's first len bytes — a refused intent and its reason. Returns 0. */
uint32_t exact_log(ExactRuntime rt, size_t len);

/* What the presenter applied of the session's orientation lock (LLP 1133.029
 * §8): JSON in the input buffer's first len bytes, for the agent's
 * state.orientationlock.applied; conflict nonzero when its resolution of
 * `natural` or `current` found one. Returns 0. */
uint32_t exact_orientation_applied(ExactRuntime rt, size_t len, uint8_t conflict);

/* A command that shows system UI, about to run: {"command":"share",
 * "title","text","url","source","agent"} (LLP 1069.003) or {"command":
 * "saveFile","id","from","suggestedName","agent"} (LLP 1069.010 D3) in the
 * input buffer; the output is the runner's ruling, {"refused":…} /
 * {"ticket":N} (held for the agent) / {"present":true,…}. */
uint32_t exact_command(ExactRuntime rt, size_t len);

/* What SVG pixel work the live plan can need, as bits (0 before a boot):
 * 1 an island, a `mask` or a `filter` (LLP 1055.000 §8 ruling 4), when the
 * host should open its island module off the main thread now; 2 a filter,
 * when it should make its GPU filter pipelines now. */
uint8_t exact_svg_islands(ExactRuntime rt);

/* An auth session's word (LLP 1069.006), JSON in the input buffer:
 * {"op":"hold","ticket":N} under the agent, or {"op":"done","ticket":N,
 * "url":…} / {"op":"done","ticket":N,"status":N,"message":…}. Returns 0; the
 * executor wakes and the next pump delivers the answer. */
uint32_t exact_auth(ExactRuntime rt, size_t len);

/* How a presence check ended (LLP 1133.010 D13), JSON in the input buffer:
 * {"call":N,"status":"passed"} (the backend reads the item),
 * {"call":N,"status":"read","value":"…"}, {"call":N,"status":"read"} for an
 * item the platform dropped, or {"call":N,"status":"failed","code":…,
 * "message":…}. Returns 0; the next pump settles the call. Never journaled. */
uint32_t exact_presence(ExactRuntime rt, size_t len);

/* The keychain the presence backend keeps locked items in (D6). Called on
 * the runtime's thread, synchronously; answered with done(reply, status,
 * bytes, len) before it returns: status 0 done, 1 not found or not capable,
 * below 0 failed with a message. op 1 add {"service","account","policy"}
 * with the value; 2 delete {"service","account"}; 3 whether the indexed
 * {"service","account"} exists, without authenticating; 4 delete every item
 * under {"service"}; 5
 * whether this build keeps locked items. Never shows the person anything. */
typedef void (*ExactPresenceReplyFn)(void *reply, int32_t status, const uint8_t *bytes, size_t len);
typedef void (*ExactPresenceKeychainFn)(uint32_t op, const uint8_t *json, size_t len, const uint8_t *value, size_t value_len, void *reply, ExactPresenceReplyFn done);
void exact_on_presence_keychain(ExactPresenceKeychainFn keychain);

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
