/* Android's owner-thread C ABI. One operation returns one borrowed EXA1
 * transaction; consume it before another operation or destroy. The shared
 * paragraph structs remain borrowed only for the measure callback. */
#ifndef EXACT_ANDROID_H
#define EXACT_ANDROID_H
#include "../../apple/include/exact.h"

/* The Android C entrypoints and EXA1 transaction format, baked into the receipt. */
#define EXACT_ANDROID_ABI_VERSION 1

#ifdef __cplusplus
extern "C" {
#endif

uint32_t exact_android_create(void);
void exact_android_destroy(uint32_t rt);
void exact_android_set_measure(uint32_t rt, ExactMeasureFn measure, void *ctx);
/* Default off: the caller must reset existing carriers on the renew operation. */
void exact_android_set_row_reuse(uint32_t rt, uint32_t on);
void exact_android_set_wake(uint32_t rt, ExactWakeFn wake, void *ctx);
void exact_android_set_fonts(uint32_t rt, ExactFontsFn fonts, void *ctx);
uint8_t *exact_android_in(uint32_t rt, size_t len);
const uint8_t *exact_android_out(uint32_t rt);
uint32_t exact_android_boot(uint32_t rt, float width, float height);
/* Optional UTF-8 initial authored press target, applied before first publication. */
uint32_t exact_android_boot_initial(uint32_t rt, float width, float height, size_t len);
uint32_t exact_android_dispatch(uint32_t rt, uint32_t view, uint32_t kind, size_t len, double now);
uint32_t exact_android_resize(uint32_t rt, float width, float height);
uint32_t exact_android_frame(uint32_t rt, double now);
uint32_t exact_android_advance(uint32_t rt, double now);
uint32_t exact_android_tick(uint32_t rt, double now);
uint32_t exact_android_pump(uint32_t rt, double now);
uint32_t exact_android_painted(uint32_t rt);
uint32_t exact_android_preferences(uint32_t rt, uint32_t bits);
uint32_t exact_android_insets(uint32_t rt, float top, float right, float bottom, float left);
uint32_t exact_android_intrinsic(uint32_t rt, uint32_t view, float width, float height);
uint32_t exact_android_intrinsics(uint32_t rt, size_t len);
/* Control contents (0 button face, 1 select options, 2 radio group) are JSON. */
uint32_t exact_android_control_query(uint32_t rt, uint32_t view, uint32_t kind);
uint32_t exact_android_scrolled(uint32_t rt, uint32_t view, double left, double top);
uint32_t exact_android_collection_feedback(uint32_t rt, size_t len, double now);
/* The agent reply alone is UTF-8 JSON, using the same out buffer. */
uint32_t exact_android_agent(uint32_t rt, size_t len);

#ifdef __cplusplus
}
#endif
#endif
