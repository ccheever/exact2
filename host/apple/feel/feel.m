// The scroll-feel recorder: a dylib inserted into any iOS simulator app
// (SIMCTL_CHILD_DYLD_INSERT_LIBRARIES). Touches come from the touch runner
// (host/apple/touches.mjs); this watches, and moves the list only for the
// blank check, between gestures. It writes JSON lines to $FEEL_OUT (an
// absolute host path), flushed every 250 ms:
//   {"k":"hello", wall, media, maxFps, screen:[w,h], scale, bundle, pid, sample}
//   {"k":"scan", t, views:[…]}   every visible UIScrollView, when the set or a size changes
//   {"k":"touch", t, ph, x, y}   each touch phase the application dispatched (window points)
//   {"k":"f", …}                 one display-link frame (`tick:`), written while a
//                                finger is down, a scroll view moves, and 0.5 s after; `ag` is
//                                the content's shift under the offset since the last one, `ind`
//                                the vertical scroll indicator, `o` every other view that moved
//   {"k":"where", t, dt, ms, via, samples}  a late frame: where the main thread's time went
// Times are CACurrentMediaTime seconds (the host's mach clock on a simulator);
// `hello` pairs it with the wall clock so the driver's gesture times map onto it.
//
// FEEL_SAMPLE=N: every Nth moving frame, once Core Animation has committed it,
// render the moving scroll view's layer tree and measure uniform bands in it
// ({"k":"blank", t, blank, cost}; 0 turns it off; default 4). Rendered then,
// the tree is the one going on screen: rows laid out for this frame's offset.
// Sampling runs only while a file `$FEEL_OUT.sample` exists (checked every
// 250 ms), so the driver confines its cost to the gestures that measure
// blanks; a frame recorded while it runs carries "s":1, and `own` on each
// frame is the tick's own time. A file `$FEEL_OUT.check` starts the blank
// check (`check`: {"k":"bcheck"} each, then {"k":"checked"}); its frames carry
// "s":1 too. FEEL_DUMP=<dir> saves the first two samples, any with a band,
// and the check's frames as PNGs. FEEL_STACKS=0 turns the stack sampler off.
//
// The display link asks for the screen's maximum rate, so every frame the
// app draws is seen; an app that asked for less is raised to it.
#import <UIKit/UIKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>
#include <sys/time.h>
#include <unistd.h>
#include <dlfcn.h>
#include <pthread.h>
#include <stdatomic.h>
#include <mach/mach.h>
#include <mach-o/dyld.h>
#include <mach-o/loader.h>

static FILE *gOut;
static NSMutableData *gBuf;
static dispatch_queue_t gWriteQ;
static NSInteger gSample = 4;
static BOOL gSampling;     // `$FEEL_OUT.sample` exists
static const char *gSwitch;
static NSString *gDump;    // FEEL_DUMP
static NSInteger gDumped;
static __weak UIScrollView *gSampleView;  // the view to sample after this frame's commit
static double gSampleT;
static NSMutableArray *gBands;  // sampled frames with a blank band, for the check
static BOOL gChecking;          // the check is moving the list
static const char *gCheckSwitch;

static void emit(NSString *line) {
  if (!gOut) return;
  [gBuf appendData:[line dataUsingEncoding:NSUTF8StringEncoding]];
  [gBuf appendBytes:"\n" length:1];
}

static void flush(void) {
  if (!gOut || !gBuf.length) return;
  NSData *d = gBuf;
  gBuf = [NSMutableData dataWithCapacity:1 << 16];
  dispatch_async(gWriteQ, ^{ fwrite(d.bytes, 1, d.length, gOut); fflush(gOut); });
}

static NSString *json(id obj) {
  NSData *d = [NSJSONSerialization dataWithJSONObject:obj options:0 error:nil];
  return d ? [[NSString alloc] initWithData:d encoding:NSUTF8StringEncoding] : @"{}";
}

// ---- main run loop busy time (wake → sleep, as bench/heavy-list/probe) ----------------
static double gBusy, gAwakeAt;
static BOOL gAwake;
static void busyObserver(CFRunLoopObserverRef o, CFRunLoopActivity a, void *info) {
  double t = CACurrentMediaTime();
  if (a == kCFRunLoopAfterWaiting) { gAwake = YES; gAwakeAt = t; }
  else if (a == kCFRunLoopBeforeWaiting) { if (gAwake) gBusy += t - gAwakeAt; gAwake = NO; }
}
static double busyNow(void) { return gBusy + (gAwake ? CACurrentMediaTime() - gAwakeAt : 0); }

// ---- where the main thread's time goes: a stack sampler ---------------------------------
// While the recorder records, a thread of its own stops the main thread about every 2 ms, reads its stack
// by frame pointers and lets it go; between the two it touches no lock and allocates nothing. Each sample is then
// labelled by what its frames are: who (the innermost frame that is author code: the app's JS, Exact's runtime,
// Exact's host, the app's own code) and what (the innermost platform framework below it: SwiftUI, UIKit, Core
// Animation, text and drawing, accessibility), plus how the run loop got there (a timer, the main queue, an event
// or display link, an observer such as Core Animation's commit) when the frames say. A main thread waiting on
// another thread (Exact's runtime runs on `exact.owner`) is labelled by what the running thread does. A late frame
// gets a `where` line: each label's share of its interval in ms.
enum { kTransparent = 0, kSystem = 1, kAuthor = 2, kHostImage = 3 };
typedef struct { uintptr_t lo, hi; uint8_t kind; uint8_t label; } FeelImage;
static FeelImage gImages[1024];
static int gImageCount;
static const char *gLabels[256];
static _Atomic int gLabelCount;
static mach_port_t gMainMach;
static pthread_t gMainPthread;
static _Atomic int gSamplerOn;
static BOOL gExactApp;  // the executable is Exact's host: its code that is not the app's JS is Exact's
static BOOL gStacks = YES;  // FEEL_STACKS=0 turns the sampler off
typedef struct { double t; uint8_t who, what, via, idle; } FeelSample;
enum { kRing = 8192 };
static FeelSample gRing[kRing];
static _Atomic uint64_t gRingN;

static uint8_t labelId(const char *name) {
  int n = atomic_load(&gLabelCount);
  for (int i = 0; i < n; i++) if (!strcmp(gLabels[i], name)) return (uint8_t)i;
  if (n >= 255) return 0;
  gLabels[n] = strdup(name);
  atomic_store(&gLabelCount, n + 1);
  return (uint8_t)n;
}

/// The loaded images' text ranges, each with what its frames count as.
static void readImages(void) {
  NSString *bundle = NSBundle.mainBundle.bundlePath;
  Dl_info self;
  dladdr((void *)readImages, &self);
  int n = 0;
  for (uint32_t i = 0, count = _dyld_image_count(); i < count && n < 1024; i++) {
    const struct mach_header_64 *h = (const struct mach_header_64 *)_dyld_get_image_header(i);
    const char *path = _dyld_get_image_name(i);
    if (!h || !path) continue;
    intptr_t slide = _dyld_get_image_vmaddr_slide(i);
    const struct load_command *lc = (const struct load_command *)(h + 1);
    uintptr_t lo = 0, hi = 0;
    for (uint32_t c = 0; c < h->ncmds; c++, lc = (const struct load_command *)((const char *)lc + lc->cmdsize)) {
      if (lc->cmd != LC_SEGMENT_64) continue;
      const struct segment_command_64 *seg = (const struct segment_command_64 *)lc;
      if (strcmp(seg->segname, "__TEXT")) continue;
      lo = seg->vmaddr + slide; hi = lo + seg->vmsize;
    }
    if (!hi) continue;
    const char *base = strrchr(path, '/') ? strrchr(path, '/') + 1 : path;
    uint8_t kind = kTransparent, label = 0;
    if (self.dli_fbase == (void *)h) { kind = kSystem; label = labelId("the recorder"); }
    else if ([[NSString stringWithUTF8String:path] hasPrefix:bundle]) kind = kHostImage;
    else {
      static const char *table[][2] = {
        {"SwiftUI", "SwiftUI"}, {"SwiftUICore", "SwiftUI"}, {"AttributeGraph", "SwiftUI"},
        {"UIKitCore", "UIKit"}, {"QuartzCore", "Core Animation"},
        {"CoreText", "text & drawing"}, {"libFontParser.dylib", "text & drawing"}, {"CoreGraphics", "text & drawing"},
        {"UIFoundation", "text & drawing"}, {"ImageIO", "images"}, {"CoreImage", "images"}, {"Metal", "Metal"},
        {"UIAccessibility", "accessibility"}, {"AccessibilityUtilities", "accessibility"}, {"AXRuntime", "accessibility"},
        {"UIKitCoreAccessibility", "accessibility"}, {"SwiftUIAccessibility", "accessibility"}, {"AXCoreUtilities", "accessibility"},
      };
      for (size_t t = 0; t < sizeof table / sizeof *table; t++) if (!strcmp(base, table[t][0])) { kind = kSystem; label = labelId(table[t][1]); }
      if (!kind && strstr(path, "Accessibility")) { kind = kSystem; label = labelId("accessibility"); }
    }
    gImages[n++] = (FeelImage){lo, hi, kind, label};
  }
  gImageCount = n;
  gExactApp = dlsym(RTLD_DEFAULT, "exact_advance") != NULL;
}

static const FeelImage *imageOf(uintptr_t pc) {
  for (int i = 0; i < gImageCount; i++) if (pc >= gImages[i].lo && pc < gImages[i].hi) return &gImages[i];
  return NULL;
}

/// What a return address in the app's bundle is: Exact's runtime (Rust), the app's JS (Hermes and its JSI
/// adapter), Exact's host (Swift and Objective-C named Exact…, and in an Exact app everything else: its author
/// writes no native code), else the app's own code.
static uint8_t authorLabel(uintptr_t pc) {
  Dl_info info;
  const char *name = dladdr((void *)pc, &info) && info.dli_sname ? info.dli_sname : "";
  if (strstr(name, "hermes") || strstr(name, "facebook3jsi") || strstr(name, "ibex2")) return labelId("app JS");
  if (!strncmp(name, "_R", 2) || strstr(name, "exact_")) return labelId("Exact runtime");
  if (!strncmp(name, "$s", 2)) {
    const char *m = name + 2;
    while (*m >= '0' && *m <= '9') m++;
    if (!strncmp(m, "Exact", 5)) return labelId("Exact host");
  }
  if (strstr(name, "[Exact") || gExactApp) return labelId("Exact host");
  return labelId("app code");
}

/// How the run loop reached this frame, by the callout's name where the platform's symbols give it.
static uint8_t viaLabel(uintptr_t pc) {
  Dl_info info;
  if (!dladdr((void *)pc, &info) || !info.dli_sname) return 0;
  const char *n = info.dli_sname;
  if (strstr(n, "TIMER_CALLBACK")) return labelId("timers");
  if (strstr(n, "MAIN_DISPATCH_QUEUE")) return labelId("the main queue");
  if (strstr(n, "SOURCE0")) return labelId("events");
  if (strstr(n, "SOURCE1")) return labelId("display links and ports");
  if (strstr(n, "OBSERVER_CALLBACK")) return labelId("run-loop observers (the commit)");
  return 0;
}

// Return address → (who or what, via) for frames seen before: the sampler's own open-addressed cache.
typedef struct { uintptr_t pc; uint8_t kind, label, via; } FeelPc;
enum { kPcCache = 1 << 15 };
static FeelPc gPcs[kPcCache];
static FeelPc pcInfo(uintptr_t pc) {
  uint32_t h = (uint32_t)((pc >> 2) * 2654435761u) & (kPcCache - 1);
  for (int probe = 0; probe < 8; probe++, h = (h + 1) & (kPcCache - 1)) {
    if (gPcs[h].pc == pc) return gPcs[h];
    if (gPcs[h].pc) continue;
    const FeelImage *img = imageOf(pc);
    FeelPc e = {pc, img ? img->kind : kTransparent, img ? img->label : 0, 0};
    if (e.kind == kHostImage) { e.kind = kAuthor; e.label = authorLabel(pc); }
    if (e.kind == kTransparent) e.via = viaLabel(pc);
    gPcs[h] = e;
    return e;
  }
  FeelPc e = {pc, kTransparent, 0, 0};
  return e;
}

/// Up to 128 return addresses of `thread`, stopped while read. 0 when it cannot be read.
static int stackOf(mach_port_t thread, uintptr_t lo, uintptr_t hi, uintptr_t *pcs) {
  if (thread_suspend(thread) != KERN_SUCCESS) return 0;
  arm_thread_state64_t st;
  mach_msg_type_number_t count = ARM_THREAD_STATE64_COUNT;
  int n = 0;
  if (thread_get_state(thread, ARM_THREAD_STATE64, (thread_state_t)&st, &count) == KERN_SUCCESS) {
    pcs[n++] = (uintptr_t)arm_thread_state64_get_pc(st);
    uintptr_t fp = (uintptr_t)arm_thread_state64_get_fp(st);
    pcs[n++] = (uintptr_t)arm_thread_state64_get_lr(st);
    while (n < 128 && fp >= lo && fp + 16 <= hi && !(fp & 7)) {
      uintptr_t next = ((uintptr_t *)fp)[0], ret = ((uintptr_t *)fp)[1];
      if (!ret) break;
      pcs[n++] = ret & 0x0000000FFFFFFFFFull;
      if (next <= fp) break;
      fp = next;
    }
  }
  thread_resume(thread);
  return n;
}

/// `pcs` labelled: who, what and via (0: none).
static void label(const uintptr_t *pcs, int n, uint8_t *who, uint8_t *what, uint8_t *via) {
  *who = *what = *via = 0;
  uint8_t firstSystem = 0;
  for (int i = 0; i < n; i++) {
    FeelPc e = pcInfo(pcs[i]);
    if (e.kind == kAuthor && !*who) *who = e.label;
    if (e.kind == kSystem && !*who && !*what) *what = e.label;
    if (e.kind == kSystem && !firstSystem) firstSystem = e.label;
    if (e.via) *via = e.via;
  }
  if (!*who) { *who = firstSystem ? firstSystem : labelId("the platform"); *what = 0; }
}

/// Whether `pc` is a kernel wait (a lock, a semaphore, a condition), not the run loop's sleep.
static BOOL waiting(uintptr_t pc) {
  Dl_info info;
  if (!dladdr((void *)pc, &info) || !info.dli_sname) return NO;
  const char *n = info.dli_sname;
  return strstr(n, "psynch_cvwait") || strstr(n, "semaphore_wait") || strstr(n, "ulock_wait") || strstr(n, "psynch_mutexwait")
    || strstr(n, "semaphore_timedwait");
}

/// For a main thread that waits: what the running thread it waits on does, by the first running other thread whose
/// frames are author code. `who` gets "<thread>: <label>".
static void waitedOn(uint8_t *who, uint8_t *what) {
  thread_act_array_t threads;
  mach_msg_type_number_t count;
  if (task_threads(mach_task_self(), &threads, &count) != KERN_SUCCESS) return;
  mach_port_t me = mach_thread_self();
  for (mach_msg_type_number_t i = 0; i < count; i++) {
    mach_port_t t = threads[i];
    if (t == gMainMach || t == me) continue;
    struct thread_basic_info bi;
    mach_msg_type_number_t bc = THREAD_BASIC_INFO_COUNT;
    if (thread_info(t, THREAD_BASIC_INFO, (thread_info_t)&bi, &bc) != KERN_SUCCESS || bi.run_state != TH_STATE_RUNNING) continue;
    pthread_t pt = pthread_from_mach_thread_np(t);
    if (!pt) continue;
    uintptr_t hi = (uintptr_t)pthread_get_stackaddr_np(pt), lo = hi - pthread_get_stacksize_np(pt);
    uintptr_t pcs[128];
    int n = stackOf(t, lo, hi, pcs);
    uint8_t w, x, v;
    if (!n) continue;
    label(pcs, n, &w, &x, &v);
    char name[64] = {0}, both[160];
    pthread_getname_np(pt, name, sizeof name);
    snprintf(both, sizeof both, "%s (main waits on %s)", gLabels[w], name[0] ? name : "a thread");
    *who = labelId(both); *what = x;
    break;
  }
  for (mach_msg_type_number_t i = 0; i < count; i++) mach_port_deallocate(mach_task_self(), threads[i]);
  vm_deallocate(mach_task_self(), (vm_address_t)threads, count * sizeof(thread_act_t));
  mach_port_deallocate(mach_task_self(), me);
}

static void *samplerMain(void *arg) {
  pthread_setname_np("feel.sampler");
  uintptr_t hi = (uintptr_t)pthread_get_stackaddr_np(gMainPthread), lo = hi - pthread_get_stacksize_np(gMainPthread);
  uintptr_t pcs[128];
  for (;;) {
    if (!atomic_load(&gSamplerOn)) { usleep(4000); continue; }
    double t = CACurrentMediaTime();
    FeelSample s = {t, 0, 0, 0, 0};
    if (!gAwake) s.idle = 1;
    else {
      int n = stackOf(gMainMach, lo, hi, pcs);
      if (n) {
        label(pcs, n, &s.who, &s.what, &s.via);
        if (waiting(pcs[0])) waitedOn(&s.who, &s.what);
      }
    }
    uint64_t i = atomic_load(&gRingN);
    gRing[i % kRing] = s;
    atomic_store(&gRingN, i + 1);
    usleep(2000);
  }
  return NULL;
}

static void startSampler(void) {
  if (!gStacks) return;
  gMainMach = mach_thread_self();
  gMainPthread = pthread_self();
  readImages();
  pthread_t t;
  pthread_attr_t a;
  pthread_attr_init(&a);
  pthread_attr_set_qos_class_np(&a, QOS_CLASS_USER_INTERACTIVE, 0);
  pthread_create(&t, &a, samplerMain, NULL);
}

/// The samples since `from`: `{label: ms}` over `ms`, each label's share of them (idle left out), and the same for
/// how the run loop got there (`via`).
static NSString *whereSince(double from, double ms) {
  uint64_t end = atomic_load(&gRingN);
  NSMutableDictionary<NSString *, NSNumber *> *count = [NSMutableDictionary dictionary], *via = [NSMutableDictionary dictionary];
  NSInteger total = 0;
  for (uint64_t i = end; i > 0 && end - i < kRing; i--) {
    FeelSample s = gRing[(i - 1) % kRing];
    if (s.t <= from) break;
    total++;
    if (s.idle) continue;
    NSString *key = s.what ? [NSString stringWithFormat:@"%s · %s", gLabels[s.who], gLabels[s.what]] : @(gLabels[s.who]);
    count[key] = @(count[key].integerValue + 1);
    if (s.via) { NSString *v = @(gLabels[s.via]); via[v] = @(via[v].integerValue + 1); }
  }
  if (!total) return nil;
  NSMutableDictionary *out = [NSMutableDictionary dictionary], *viaMs = [NSMutableDictionary dictionary];
  for (NSString *k in count) out[k] = @(round(count[k].doubleValue / total * ms * 10) / 10);
  for (NSString *k in via) viaMs[k] = @(round(via[k].doubleValue / total * ms * 10) / 10);
  return json(@{@"ms": out, @"via": viaMs, @"samples": @(total)});
}

// ---- touches, as the application dispatches them ---------------------------------------
static NSInteger gDown;          // fingers down now
static double gLastTouchT;
static IMP gOrigSendEvent;
static void feel_sendEvent(id self, SEL _cmd, UIEvent *e) {
  if (e.type == UIEventTypeTouches) {
    NSInteger down = 0;
    for (UITouch *t in e.allTouches) {
      CGPoint p = [t locationInView:nil];
      UITouchPhase ph = t.phase;
      const char *name = ph == UITouchPhaseBegan ? "began" : ph == UITouchPhaseMoved ? "moved" : ph == UITouchPhaseStationary ? "still"
        : ph == UITouchPhaseEnded ? "ended" : ph == UITouchPhaseCancelled ? "cancelled" : "other";
      if (ph != UITouchPhaseEnded && ph != UITouchPhaseCancelled) down++;
      if (ph != UITouchPhaseStationary)
        emit([NSString stringWithFormat:@"{\"k\":\"touch\",\"t\":%.6f,\"ph\":\"%s\",\"x\":%.2f,\"y\":%.2f,\"id\":%lu}",
              t.timestamp, name, p.x, p.y, (unsigned long)(uintptr_t)t % 100000]);
    }
    gDown = down;
    gLastTouchT = CACurrentMediaTime();
  }
  ((void (*)(id, SEL, UIEvent *))gOrigSendEvent)(self, _cmd, e);
}

// ---- scroll views -------------------------------------------------------------------------
@interface FeelView : NSObject
@property (nonatomic, weak) UIScrollView *view;
@property (nonatomic) NSInteger ident, depth;
@property (nonatomic) CGPoint last;
@end
@implementation FeelView
@end

/// A layer drawn under `s` that holds the middle of its viewport and is shorter than 0.6 of it (a row, or what draws
/// one), the shallowest; not a scroll indicator's. Its position in `s`'s content tells content that moved under the
/// offset from a list that scrolled.
static CALayer *anchorIn(UIScrollView *s) {
  CGRect b = s.bounds;
  double mid = CGRectGetMidY(b);
  NSMutableArray<CALayer *> *level = [NSMutableArray arrayWithObject:s.layer];
  for (int depth = 0; depth < 6 && level.count; depth++) {
    NSMutableArray<CALayer *> *next = [NSMutableArray array];
    for (CALayer *parent in level) {
      for (CALayer *l in parent.sublayers) {
        if (l.hidden || l.opacity < 0.01) continue;
        id d = l.delegate;
        if ([d isKindOfClass:UIView.class] && strstr(object_getClassName(d), "ScrollIndicator")) continue;
        CGRect r = [l convertRect:l.bounds toLayer:s.layer];
        if (r.origin.y > mid || CGRectGetMaxY(r) <= mid || r.size.width < 1) continue;
        if (r.size.height >= 4 && r.size.height < 0.6 * b.size.height) return l;
        [next addObject:l];
      }
    }
    level = next;
  }
  return nil;
}

static NSArray<UIWindow *> *visibleWindows(void) {
  NSMutableArray *out = [NSMutableArray array];
  for (UIScene *s in UIApplication.sharedApplication.connectedScenes) {
    if (![s isKindOfClass:UIWindowScene.class] || s.activationState == UISceneActivationStateBackground) continue;
    for (UIWindow *w in ((UIWindowScene *)s).windows) if (!w.hidden && w.alpha > 0.01) [out addObject:w];
  }
  return out;
}

static BOOL shown(UIView *v) {
  for (UIView *a = v; a; a = a.superview) if (a.hidden || a.alpha < 0.01) return NO;
  return YES;
}

static void collect(UIView *v, NSInteger depth, NSMutableArray *out) {
  if (v.hidden || v.alpha < 0.01) return;
  if ([v isKindOfClass:UIScrollView.class] && v.bounds.size.width >= 40 && v.bounds.size.height >= 40) [out addObject:@[v, @(depth)]];
  for (UIView *c in v.subviews) collect(c, depth + 1, out);
}

/// A sampled frame with a blank band, kept for the check.
@interface FeelBand : NSObject
@property (nonatomic, weak) UIScrollView *view;
@property (nonatomic) double t, blank;
@property (nonatomic) CGPoint offset;
@property (nonatomic) CGRect rect;
@property (nonatomic) double shift;  // the list's content shift under its offset when sampled (`shiftOf:`)
@property (nonatomic, strong) NSData *rows;
@end
@implementation FeelBand
@end

@interface FeelProbe : NSObject
@property (nonatomic, strong) CADisplayLink *link;
@property (nonatomic, strong) NSMapTable<UIScrollView *, FeelView *> *views;
@property (nonatomic) NSInteger nextIdent, frame, movingFrames, sinceScan;
@property (nonatomic, weak) FeelView *focus;
@property (nonatomic) double lastStamp, lastBusy, lastMotion, lastScanT;
@property (nonatomic, copy) NSString *scanKey;
@property (nonatomic, strong) NSMutableArray<FeelBand *> *checkTodo;
@property (nonatomic, weak) UIScrollView *checkHome;
@property (nonatomic) CGPoint checkBack;
@property (nonatomic) NSUInteger checkKept;
@property (nonatomic) NSInteger checkDone;
@property (nonatomic, weak) CALayer *anchor;
@property (nonatomic, weak) UIScrollView *anchorOf;
@property (nonatomic) double anchorY, contentShift;
@end

@implementation FeelProbe

- (void)start {
  self.views = [NSMapTable weakToStrongObjectsMapTable];
  self.link = [CADisplayLink displayLinkWithTarget:self selector:@selector(tick:)];
  float maxFps = (float)UIScreen.mainScreen.maximumFramesPerSecond;
  self.link.preferredFrameRateRange = CAFrameRateRangeMake(maxFps, maxFps, maxFps);
  [self.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
  self.lastBusy = busyNow();
  // Common modes: a scroll view's tracking mode would hold a default-mode timer until it stops.
  [NSRunLoop.mainRunLoop addTimer:[NSTimer timerWithTimeInterval:0.25 repeats:YES block:^(NSTimer *t) { 
    flush();
    gSampling = access(gSwitch, F_OK) == 0;
    if (!gChecking && access(gCheckSwitch, F_OK) == 0) {
      NSString *first = [NSString stringWithContentsOfFile:@(gCheckSwitch) encoding:NSUTF8StringEncoding error:nil];
      unlink(gCheckSwitch);
      [self check:first];
    }
  }] forMode:NSRunLoopCommonModes];
}

/// How far `s`'s content has moved under its offset since the anchor was first taken in it: the ticks' `ag`, and
/// the anchor's move since the last tick (a layout after it).
- (double)shiftOf:(UIScrollView *)s {
  CALayer *a = self.anchor;
  if (!a || self.anchorOf != s) return self.contentShift;
  CALayer *up = a;
  while (up && up != s.layer) up = up.superlayer;
  return self.contentShift + (up ? [a convertRect:a.bounds toLayer:s.layer].origin.y - self.anchorY : 0);
}

/// Every visible scroll view, registered; a `scan` line when the set or a geometry changed.
- (void)rescan:(double)now {
  NSMutableArray *found = [NSMutableArray array];
  for (UIWindow *w in visibleWindows()) collect(w, 0, found);
  NSMutableArray *rows = [NSMutableArray array];
  NSMutableString *key = [NSMutableString string];
  for (NSArray *pair in found) {
    UIScrollView *s = pair[0];
    FeelView *fv = [self.views objectForKey:s];
    if (!fv) { fv = [FeelView new]; fv.view = s; fv.ident = ++self.nextIdent; fv.last = s.contentOffset; [self.views setObject:fv forKey:s]; }
    fv.depth = [pair[1] integerValue];
    CGRect f = [s convertRect:s.bounds toView:nil];
    UIEdgeInsets in = s.adjustedContentInset;
    // Whether a touch at the middle of the visible content would reach this view (or a view inside it).
    CGPoint mid = CGPointMake(CGRectGetMidX(f), f.origin.y + in.top + (f.size.height - in.top - in.bottom) / 2);
    UIView *hit = [s.window hitTest:[s.window convertPoint:mid fromWindow:nil] withEvent:nil];
    BOOL reach = hit && [hit isDescendantOfView:s];
    [key appendFormat:@"%ld:%.0f,%.0f,%.0f,%.0f,%.0f,%.0f,%d;", (long)fv.ident, f.origin.x, f.origin.y, f.size.width, f.size.height,
      s.contentSize.width, s.contentSize.height, reach];
    [rows addObject:@{@"id": @(fv.ident), @"cls": NSStringFromClass(s.class), @"depth": @(fv.depth),
      @"frame": @[@(f.origin.x), @(f.origin.y), @(f.size.width), @(f.size.height)],
      @"cs": @[@(s.contentSize.width), @(s.contentSize.height)], @"off": @[@(s.contentOffset.x), @(s.contentOffset.y)],
      @"inset": @[@(in.top), @(in.left), @(in.bottom), @(in.right)],
      @"decel": @(s.decelerationRate), @"bounces": @(s.bounces), @"bouncesV": @(s.alwaysBounceVertical),
      @"scrollsToTop": @(s.scrollsToTop), @"delays": @(s.delaysContentTouches), @"paging": @(s.pagingEnabled),
      @"enabled": @(s.scrollEnabled), @"shown": @(shown(s)), @"reach": @(reach)}];
  }
  if (![key isEqualToString:self.scanKey ?: @""] && now - self.lastScanT > 0.2) {
    self.scanKey = key; self.lastScanT = now;
    UIWindow *keyWin = nil;
    for (UIWindow *w in visibleWindows()) if (w.isKeyWindow) keyWin = w;
    emit(json(@{@"k": @"scan", @"t": @(now), @"views": rows, @"safeTop": @(keyWin.safeAreaInsets.top)}));
  }
}

/// The part of `s` on screen with content under it (an overscroll's gap is no blank), in its own space: the content's.
static CGRect visibleContent(UIScrollView *s) {
  UIWindow *w = s.window;
  if (!w) return CGRectNull;
  UIEdgeInsets in = s.adjustedContentInset;
  CGRect vis = s.bounds;
  vis.origin.y += in.top; vis.size.height -= in.top + in.bottom;
  CGRect r = CGRectIntersection(vis, [s convertRect:w.bounds fromView:w]);
  return CGRectIntersection(r, (CGRect){CGPointZero, s.contentSize});
}

static const CGFloat kScale = 0.25, kMargin = 28;
// A rendered scanline: whether it is uniform, then the mean luminance of each quarter of the inner width.
static const size_t kStride = 5;

/// `r` of `s` (content coordinates) rendered at a quarter scale, `kStride` bytes a scanline: 1 where the scanline's
/// luminance range across the inner width is under 10 (uniform), then its quarters' mean luminance. Nil when too small.
- (NSData *)uniformIn:(UIScrollView *)s rect:(CGRect)r name:(NSString *)name {
  size_t W = (size_t)(r.size.width * kScale), H = (size_t)(r.size.height * kScale);
  if (W < 8 || H < 8) return nil;
  uint8_t *px = calloc(W * H * 4, 1);
  CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
  CGContextRef ctx = CGBitmapContextCreate(px, W, H, 8, W * 4, cs, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
  CGColorSpaceRelease(cs);
  CGContextTranslateCTM(ctx, 0, H);
  CGContextScaleCTM(ctx, kScale, -kScale);
  // Each sublayer that meets the rect, at its frame: the scroll view's own layer, rendered whole, was clipped as if
  // its bounds had no offset (a band as tall as the offset read blank), and the window's whole tree cost about 9 ms
  // a sample in the UIKit crypto list.
  for (CALayer *l in s.layer.sublayers) {
    CGRect f = l.frame;
    if (l.hidden || l.opacity < 0.01 || !CGRectIntersectsRect(f, r)) continue;
    CGContextSaveGState(ctx);
    CGContextTranslateCTM(ctx, f.origin.x - r.origin.x, f.origin.y - r.origin.y);
    [l renderInContext:ctx];
    CGContextRestoreGState(ctx);
  }
  NSMutableData *rows = [NSMutableData dataWithLength:H * kStride];
  uint8_t *u = rows.mutableBytes;
  size_t x0 = (size_t)(kMargin * kScale), x1 = W - x0, seg = (x1 - x0) / 4;
  for (size_t y = 0; y < H; y++) {
    int lo = 255, hi = 0, sum[4] = {0};
    for (size_t x = x0; x < x1; x++) {
      uint8_t *p = px + (y * W + x) * 4;
      int l = (p[0] * 3 + p[1] * 6 + p[2]) / 10;
      if (l < lo) lo = l;
      if (l > hi) hi = l;
      sum[MIN(3, (x - x0) / MAX(seg, 1))] += l;
    }
    uint8_t *row = u + y * kStride;
    row[0] = (hi - lo) < 10;
    for (int i = 0; i < 4; i++) row[1 + i] = (uint8_t)(sum[i] / (int)MAX(seg, 1));
  }
  if (name) {
    CGImageRef img = CGBitmapContextCreateImage(ctx);
    [UIImagePNGRepresentation([UIImage imageWithCGImage:img]) writeToFile:[gDump stringByAppendingFormat:@"/%@.png", name] atomically:NO];
    CGImageRelease(img);
  }
  CGContextRelease(ctx);
  free(px);
  return rows;
}

/// Points of `rows` in runs of at least 60 pt of uniform scanlines.
static double bandPoints(NSData *rows) {
  const uint8_t *u = rows.bytes;
  size_t H = rows.length / kStride, minRun = (size_t)(60 * kScale), run = 0;
  double pts = 0;
  for (size_t y = 0; y <= H; y++) {
    if (y < H && u[y * kStride]) { run++; continue; }
    if (run >= minRun) pts += run / kScale;
    run = 0;
  }
  return pts;
}

/// Of the bands in `rows` (a sampled frame), the points that `settled` (the list at rest near the same offset) has
/// ink in: at least 12 pt of scanlines that are not uniform. A list that measures rows as they show moves its
/// content under a fixed offset, so `settled` is first aligned to `rows`: the shift whose scanlines differ least
/// from the sample's inked ones. A band the shift puts outside `settled` is not judged; -1 when none is, or when
/// nothing aligns (a frame so blank it has under 8 inked scanlines is judged unshifted).
static const double kAlignErr = 1;
static double latePoints(NSData *rows, NSData *settled, double *alignErr, long *alignShift) {
  const uint8_t *u = rows.bytes, *v = settled.bytes;
  long Hs = (long)(rows.length / kStride), Hr = (long)(settled.length / kStride);
  long inked = 0;
  for (long i = 0; i < Hs; i++) inked += !u[i * kStride];
  long shift = 0;
  if (inked >= 8) {
    double best = 1e9;
    for (long k = -Hs + 1; k < Hr; k++) {
      long n = 0; double err = 0;
      for (long i = MAX(0, -k); i < Hs && i + k < Hr; i++) {
        if (u[i * kStride]) continue;
        const uint8_t *a = u + i * kStride, *b = v + (i + k) * kStride;
        err += abs(a[1] - b[1]) + abs(a[2] - b[2]) + abs(a[3] - b[3]) + abs(a[4] - b[4]);
        n++;
      }
      if (n < MAX(8, inked * 3 / 10)) continue;
      // Slightly in favour of small shifts, so a uniform stretch does not drift.
      err = err / (4.0 * n) + labs(k) * 0.002;
      if (err < best) { best = err; shift = k; }
    }
    *alignErr = best; *alignShift = shift;
    if (best > kAlignErr) return -1;
  }
  long minRun = (long)(60 * kScale), run = 0;
  double pts = 0; BOOL judged = NO;
  for (long y = 0; y <= Hs; y++) {
    if (y < Hs && u[y * kStride]) { run++; continue; }
    if (run >= minRun) {
      long from = y - run + shift, to = y + shift;
      if (from >= 0 && to <= Hr) {
        judged = YES;
        long ink = 0;
        for (long j = from; j < to; j++) ink += !v[j * kStride];
        if (ink >= (long)(12 * kScale)) pts += run / kScale;
      }
    }
    run = 0;
  }
  return judged ? pts : -1;
}

/// The blank bands of `s` in this frame; a frame with one is kept for the check.
- (double)blankIn:(UIScrollView *)s at:(double)t {
  CGRect r = visibleContent(s);
  if (CGRectIsNull(r)) return -1;
  BOOL dump = gDump && gDumped < 8;
  NSData *rows = [self uniformIn:s rect:r name:nil];
  if (!rows) return -1;
  double blank = bandPoints(rows);
  if (dump && (gDumped < 2 || blank > 0)) {
    NSString *name = [NSString stringWithFormat:@"sample-%ld-%.0f", (long)gDumped++, blank];
    [self uniformIn:s rect:r name:name];
    // What the screen shows, for comparison: the window as the render server has it.
    UIGraphicsImageRenderer *g = [[UIGraphicsImageRenderer alloc] initWithBounds:s.window.bounds];
    UIImage *shot = [g imageWithActions:^(UIGraphicsImageRendererContext *c) { [s.window drawViewHierarchyInRect:s.window.bounds afterScreenUpdates:NO]; }];
    [UIImageJPEGRepresentation(shot, 0.5) writeToFile:[gDump stringByAppendingFormat:@"/%@-screen.jpg", name] atomically:NO];
  }
  if (blank > 0 && gBands.count < 256) {
    FeelBand *b = [FeelBand new];
    b.view = s; b.t = t; b.blank = blank; b.offset = s.contentOffset; b.rect = r; b.rows = rows; b.shift = [self shiftOf:s];
    [gBands addObject:b];
  }
  return blank;
}

/// The check (`$FEEL_OUT.check`): each kept band's frame drawn again at the same offset once the list has rested
/// there, so a band counts as blank only where rows that will be drawn were not drawn yet. A grouped list's
/// background, a gap between sections or a row's own plain area is uniform at rest too. At most `kChecks` bands,
/// spread over those kept, those sampled at the times the switch file names first (up to half); the list goes back
/// to where it was. Each `{"k":"bcheck", t, blank, late, ms}` (`late`: the points of the band with ink at rest;
/// `gone`: the view left), then `{"k":"checked", n}`.
static const NSUInteger kChecks = 10;
- (void)check:(NSString *)first {
  gChecking = YES;
  NSArray<FeelBand *> *all = gBands;
  gBands = [NSMutableArray array];
  self.checkTodo = [NSMutableArray array];
  NSMutableArray<FeelBand *> *rest = [all mutableCopy];
  for (NSString *word in [first ?: @"" componentsSeparatedByCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet]) {
    if (!word.length || self.checkTodo.count >= kChecks / 2) continue;
    double t = word.doubleValue;
    for (FeelBand *b in rest) if (fabs(b.t - t) < 1e-4) { [self.checkTodo addObject:b]; [rest removeObject:b]; break; }
  }
  for (NSUInteger i = 0, n = MIN(rest.count, kChecks - self.checkTodo.count); i < n; i++) [self.checkTodo addObject:rest[i * rest.count / n]];
  self.checkKept = all.count;
  self.checkDone = 0;
  self.checkHome = self.checkTodo.firstObject.view;
  self.checkBack = self.checkHome.contentOffset;
  [self checkNext];
}

- (void)checkNext {
  FeelBand *b = self.checkTodo.firstObject;
  if (!b) {
    UIScrollView *home = self.checkHome;
    if (home.window) [home setContentOffset:self.checkBack animated:NO];
    emit([NSString stringWithFormat:@"{\"k\":\"checked\",\"t\":%.6f,\"n\":%ld,\"kept\":%lu}", CACurrentMediaTime(), (long)self.checkDone, (unsigned long)self.checkKept]);
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 200 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{ gChecking = NO; });
    return;
  }
  [self.checkTodo removeObjectAtIndex:0];
  UIScrollView *s = b.view;
  if (!s.window) {
    emit([NSString stringWithFormat:@"{\"k\":\"bcheck\",\"t\":%.6f,\"blank\":%.1f,\"gone\":1}", b.t, b.blank]);
    [self checkNext];
    return;
  }
  // Where the sampled content is now: a list that measured rows above it since has moved it down.
  double moved = [self shiftOf:s] - b.shift;
  [s setContentOffset:CGPointMake(b.offset.x, b.offset.y + moved) animated:NO];
  // Drawn again after 150 ms, and once more after 500 ms when the first shows nothing new.
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 150 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{ [self checkLook:b after:150]; });
}

- (void)checkLook:(FeelBand *)b after:(double)ms {
  UIScrollView *s = b.view;
  NSString *name = nil;
  if (gDump && gDumped < 16) name = [NSString stringWithFormat:@"check-%ld-%.0fms", (long)gDumped++, ms];
  NSData *rows = s.window ? [self uniformIn:s rect:CGRectOffset(b.rect, 0, [self shiftOf:s] - b.shift) name:name] : nil;
  double err = 0; long shift = 0;
  double late = rows ? latePoints(b.rows, rows, &err, &shift) : -1;
  if (late > 0 || ms >= 500 || !rows) {
    emit([NSString stringWithFormat:@"{\"k\":\"bcheck\",\"t\":%.6f,\"blank\":%.1f,\"late\":%.1f,\"ms\":%.0f,\"err\":%.2f,\"shift\":%.0f%@}", b.t, b.blank, late, ms, err, shift / kScale, !rows ? @",\"gone\":1" : late < 0 ? @",\"unjudged\":1" : @""]);
    self.checkDone++;
    [self checkNext];
  } else {
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 350 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{ [self checkLook:b after:500]; });
  }
}

- (void)tick:(CADisplayLink *)l {
  double t0 = CACurrentMediaTime();
  self.frame++;
  BOOL touching = gDown > 0;
  // Rescan often enough to meet a pushed screen's list, rarely while moving.
  if (++self.sinceScan >= (t0 - self.lastMotion < 0.5 ? 30 : 8)) { self.sinceScan = 0; [self rescan:t0]; }
  // The moving view: the innermost one a finger or momentum drives, else the innermost one only touched (a
  // horizontal carousel under a finger that drags the list it is in is touched, not dragged), else the one whose
  // offset changed. Every other view that moved goes in `o`.
  FeelView *active = nil, *touched = nil, *shifted = nil;
  double shiftBy = 0;
  NSMutableArray<FeelView *> *moved = [NSMutableArray array];
  for (FeelView *fv in self.views.objectEnumerator) {
    UIScrollView *s = fv.view;
    if (!s || !s.window) continue;
    CGPoint o = s.contentOffset;
    double d = fabs(o.x - fv.last.x) + fabs(o.y - fv.last.y);
    fv.last = o;
    if ((s.isDragging || s.isDecelerating) && (!active || fv.depth > active.depth)) active = fv;
    if (s.isTracking && (!touched || fv.depth > touched.depth)) touched = fv;
    if (d > 0.01) [moved addObject:fv];
    if (d > 0.01 && d > shiftBy) { shifted = fv; shiftBy = d; }
  }
  FeelView *now = active ?: touched ?: shifted;
  if (now) { self.focus = now; self.lastMotion = t0; }
  BOOL record = touching || now || t0 - self.lastMotion < 0.5 || t0 - gLastTouchT < 0.5;
  double busy = busyNow(), busyMs = (busy - self.lastBusy) * 1000;
  self.lastBusy = busy;
  double stamp = l.timestamp, prev = self.lastStamp, dt = prev > 0 ? (stamp - prev) * 1000 : 0;
  self.lastStamp = stamp;
  atomic_store(&gSamplerOn, record);
  if (!record) return;
  // A late frame: where the main thread's time went since the last one.
  if (prev > 0 && dt > 1500.0 / UIScreen.mainScreen.maximumFramesPerSecond) {
    NSString *where = gStacks ? whereSince(prev, (t0 - prev) * 1000) : nil;
    if (where) emit([NSString stringWithFormat:@"{\"k\":\"where\",\"t\":%.6f,\"dt\":%.3f,%@", stamp, dt, [where substringFromIndex:1]]);
  }
  FeelView *fv = self.focus;
  UIScrollView *s = fv.view;
  if (now) self.movingFrames++;
  if (s && now && gSampling && gSample > 0 && self.movingFrames % gSample == 0) { gSampleView = s; gSampleT = stamp; }
  NSMutableString *line = [NSMutableString stringWithFormat:@"{\"k\":\"f\",\"t\":%.6f,\"tt\":%.6f,\"c\":%.6f,\"dt\":%.3f,\"busy\":%.3f,\"down\":%ld,\"mv\":%d",
    stamp, l.targetTimestamp, t0, dt, busyMs, (long)gDown, now != nil];
  if (s) {
    CGPoint o = s.contentOffset;
    CALayer *pl = s.layer.presentationLayer;
    UIEdgeInsets in = s.adjustedContentInset;
    [line appendFormat:@",\"v\":%ld,\"ox\":%.3f,\"oy\":%.3f,\"py\":%.3f,\"cw\":%.1f,\"ch\":%.1f,\"bw\":%.1f,\"bh\":%.1f,\"it\":%.1f,\"ib\":%.1f,\"il\":%.1f,\"ir\":%.1f,\"tr\":%d,\"dr\":%d,\"de\":%d",
      (long)fv.ident, o.x, o.y, pl ? pl.bounds.origin.y : o.y, s.contentSize.width, s.contentSize.height, s.bounds.size.width, s.bounds.size.height,
      in.top, in.bottom, in.left, in.right, s.isTracking, s.isDragging, s.isDecelerating];
    // `ind`: the vertical scroll indicator in window points and its alpha, where a finger grabs it to scrub.
    for (UIView *sub in s.subviews) {
      if (!strstr(object_getClassName(sub), "ScrollIndicator") || sub.bounds.size.height <= sub.bounds.size.width) continue;
      CGRect r = [sub convertRect:sub.bounds toView:nil];
      [line appendFormat:@",\"ind\":[%.1f,%.1f,%.1f,%.1f,%.2f]", r.origin.x, r.origin.y, r.size.width, r.size.height, sub.hidden ? 0 : sub.alpha];
      break;
    }
    // `ag`: how far the content under the offset moved since the last tick, as last tick's anchor moved in the
    // content; a list that measures rows above the viewport and keeps what is shown in place moves its offset by
    // as much in the same frame.
    if (self.anchor && self.anchorOf == s) {
      double ag = [self shiftOf:s] - self.contentShift;
      self.contentShift += ag;
      [line appendFormat:@",\"ag\":%.3f", ag];
    } else if (self.anchorOf != s) self.contentShift = 0;
    CALayer *a = anchorIn(s);
    self.anchor = a; self.anchorOf = s;
    self.anchorY = a ? [a convertRect:a.bounds toLayer:s.layer].origin.y : 0;
  }
  // `o`: each other view that moved, [id, x, y, tracking | dragging << 1 | decelerating << 2].
  NSMutableString *others = [NSMutableString string];
  for (FeelView *m in moved) {
    if (m == fv) continue;
    UIScrollView *ms = m.view;
    [others appendFormat:@"%@[%ld,%.3f,%.3f,%d]", others.length ? @"," : @"", (long)m.ident, ms.contentOffset.x, ms.contentOffset.y,
      ms.isTracking | ms.isDragging << 1 | ms.isDecelerating << 2];
  }
  if (others.length) [line appendFormat:@",\"o\":[%@]", others];
  if (gSampling || gChecking) [line appendString:@",\"s\":1"];
  [line appendFormat:@",\"own\":%.3f}", (CACurrentMediaTime() - t0) * 1000];
  emit(line);
}
@end

static FeelProbe *gProbe;

// After Core Animation's commit (a BeforeWaiting observer at order 2,000,000), before the busy observer's sleep.
static void sampleObserver(CFRunLoopObserverRef o, CFRunLoopActivity a, void *info) {
  UIScrollView *s = gSampleView;
  if (!s) return;
  gSampleView = nil;
  double t0 = CACurrentMediaTime(), blank = [gProbe blankIn:s at:gSampleT];
  if (blank >= 0) emit([NSString stringWithFormat:@"{\"k\":\"blank\",\"t\":%.6f,\"blank\":%.1f,\"cost\":%.3f}", gSampleT, blank, (CACurrentMediaTime() - t0) * 1000]);
}

__attribute__((constructor)) static void feel_init(void) {
  NSDictionary *env = NSProcessInfo.processInfo.environment;
  NSString *path = env[@"FEEL_OUT"];
  if (!path.length) return;
  if (env[@"FEEL_SAMPLE"]) gSample = [env[@"FEEL_SAMPLE"] integerValue];
  if ([env[@"FEEL_STACKS"] isEqual:@"0"]) gStacks = NO;
  if ([env[@"FEEL_DUMP"] length]) gDump = env[@"FEEL_DUMP"];
  gOut = fopen(path.fileSystemRepresentation, "w");
  gSwitch = strdup([path stringByAppendingString:@".sample"].fileSystemRepresentation);
  gCheckSwitch = strdup([path stringByAppendingString:@".check"].fileSystemRepresentation);
  gBands = [NSMutableArray array];
  if (!gOut) { NSLog(@"[feel] cannot write %@", path); return; }
  gBuf = [NSMutableData dataWithCapacity:1 << 16];
  gWriteQ = dispatch_queue_create("feel.write", DISPATCH_QUEUE_SERIAL);
  gAwake = YES; gAwakeAt = CACurrentMediaTime();
  // Wake is stamped by the first AfterWaiting observer and sleep by the last BeforeWaiting one, so
  // Core Animation's commit (a BeforeWaiting observer) counts as busy.
  CFRunLoopAddObserver(CFRunLoopGetMain(), CFRunLoopObserverCreate(NULL, kCFRunLoopAfterWaiting, true, LONG_MIN, busyObserver, NULL), kCFRunLoopCommonModes);
  CFRunLoopAddObserver(CFRunLoopGetMain(), CFRunLoopObserverCreate(NULL, kCFRunLoopBeforeWaiting, true, LONG_MAX, busyObserver, NULL), kCFRunLoopCommonModes);
  CFRunLoopAddObserver(CFRunLoopGetMain(), CFRunLoopObserverCreate(NULL, kCFRunLoopBeforeWaiting, true, 2100000, sampleObserver, NULL), kCFRunLoopCommonModes);
  Method m = class_getInstanceMethod(UIApplication.class, @selector(sendEvent:));
  gOrigSendEvent = method_setImplementation(m, (IMP)feel_sendEvent);
  dispatch_async(dispatch_get_main_queue(), ^{
    struct timeval tv; gettimeofday(&tv, NULL);
    UIScreen *sc = UIScreen.mainScreen;
    emit(json(@{@"k": @"hello", @"wall": @(tv.tv_sec * 1000.0 + tv.tv_usec / 1000.0), @"media": @(CACurrentMediaTime()),
                @"maxFps": @(sc.maximumFramesPerSecond), @"screen": @[@(sc.bounds.size.width), @(sc.bounds.size.height)],
                @"scale": @(sc.scale), @"bundle": NSBundle.mainBundle.bundleIdentifier ?: @"", @"pid": @(getpid()), @"sample": @(gSample)}));
    startSampler();
    gProbe = [FeelProbe new];
    [gProbe start];
  });
}
