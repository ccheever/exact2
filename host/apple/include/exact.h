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
 * Text measurement is the one call the other way: exact_boot takes a
 * function the host calls with an ExactMeasureRequest for every paragraph
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

uint8_t *exact_in(size_t len);
const uint8_t *exact_out(void);

/* Boot the plan baked into the library (or, exact_boot_plan, the input
 * buffer's first len bytes). measure may be NULL: a monospace reference
 * measurer is used. Returns the first batch's length. */
uint32_t exact_boot(ExactMeasureFn measure, void *ctx, float width, float height);
uint32_t exact_boot_plan(size_t len, ExactMeasureFn measure, void *ctx, float width, float height);

/* kind: 0 = press, 1 = change (payload = the input buffer's first len bytes). */
uint32_t exact_dispatch(uint32_t view, uint32_t kind, size_t len, double now_ms);
uint32_t exact_advance(double now_ms);   /* the runner's clock: timers */
uint32_t exact_resize(float width, float height);
uint32_t exact_tick(double now_ms);      /* a motion frame, only while "motion" is true */

#ifdef __cplusplus
}
#endif
#endif
