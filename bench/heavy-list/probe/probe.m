// The heavy list benchmark's probe (bench/heavy-list), extended from the easy list benchmark's
// probe. Injected into each app (SIMCTL_CHILD_DYLD_INSERT_LIBRARIES on a simulator,
// DYLD_INSERT_LIBRARIES from devicectl on the phone). Finds the tallest UIScrollView,
// drives it from a CADisplayLink on the main thread, and writes one JSON result to
// $BENCH_OUT (relative = the app's Documents directory).
//
// BENCH_SCENARIO: fling     constant-speed 2 s segments (±1k/3k/6k/12k/24k pt/s) after a warm-up
//                 ladder    the same, at ±3k/6k/12k/24k/48k/96k pt/s (each speed down then up)
//                 jump      ten absolute jumps, time until the viewport has no blank band
//                 coldstart from process start to the first display-link frame where the
//                           list exists and has no blank band (every frame sampled; no scrolling)
//                 rest      no scrolling: 10 s measured at the list's current offset (the apps run their
//                           live ticks in this scenario); same per-frame and CPU/busy/memory stats
//                 innerfling  (added for the extra-heavy feed's nested lists) from the list's launch offset, find a
//                           row's inner horizontal list (content width >= 20,000 pt, < 300 pt tall: the
//                           "filmstrip") and place it fully in view, then drive ITS contentOffset.x at the
//                           ladder speeds (3k warm-up, ±3k…96k, 2 s each); then the same on an inner
//                           vertical list (300–600 pt tall, content height >= 20,000 pt: the "inbox").
//                           BENCH_SAMPLE measures blank bands inside the inner list (columns >= 60 pt wide
//                           for the strip, rows >= 60 pt tall for the inbox).
//                 innerkeep for each inner kind: find it, set its offset to a mark, fling the OUTER list
//                           away and back (3,000 pt at 6k pt/s, then 24,000 pt at 12k pt/s, each way),
//                           snap back to the marked outer offset, and read the inner list found at the
//                           same place: "keep" = [{kind, trip, mark, got, err, sameView, markRead}].
//                           Primary criterion (2026-09-29, the web standard allows scroll anchoring):
//                           `keptAnchor` = the box shows the same content at the same place +/- 1 pt:
//                           its screen image at the end of the mark hold vs at the check, best shift
//                           along the axis (anchorShiftPt, +/-60 strip, +/-150 inbox) with >= 90 % of
//                           inked pixels matching (anchorMatch). `kept` (raw offset < 1 pt) is secondary.
//                           The outer fling segments are measured like fling's.
// BENCH_SAMPLE:   N = measure blank bands every Nth frame (perturbs timing)
// BENCH_RENDER:   layer = cheap renderInContext sampler (default for coldstart), else
//                 drawViewHierarchyInRect
// BENCH_DELAY:    seconds after launch before starting (default 5; coldstart ignores it)
// BENCH_LIVE:     not implemented here (the apps read it); recorded in the JSON as "live"
//
// Added metrics (per segment, in "segStats"; whole run in "run"):
//   busy  main run loop awake time: AfterWaiting -> BeforeWaiting, CFRunLoopObserver
//   cpu   process user+sys CPU (getrusage RUSAGE_SELF)
//   main  main thread user+sys CPU (thread_info THREAD_BASIC_INFO on its mach port)
//   mem   phys_footprint (task_info TASK_VM_INFO): at probe load, start, peak (per tick), end
#import <UIKit/UIKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/sysctl.h>
#include <sys/time.h>
#include <mach/mach.h>
#include <pthread.h>

static CFTimeInterval gLoad;               // dylib constructor, CACurrentMediaTime
static CFTimeInterval gProcStart;          // process start, CACurrentMediaTime basis
static NSString *gStartSource = @"constructor";
// Thermal state (2026-09-30): a hot phone was indistinguishable from a regression. New result fields only:
// thermalStart/lowPowerStart (when measuring begins), thermal/lowPower in each segStats entry (the segment's end),
// thermalEnd/lowPowerEnd (when the result is written), thermalChanges ([ms since process start, state] for every
// NSProcessInfoThermalStateDidChangeNotification since the probe loaded). States: 0 nominal, 1 fair, 2 serious,
// 3 critical; -1 = not read. Nothing else about the probe changed.
static NSMutableArray *gThermalChanges;
static NSInteger thermalNow(void) { return (NSInteger)NSProcessInfo.processInfo.thermalState; }
static BOOL lowPowerNow(void) { return NSProcessInfo.processInfo.lowPowerModeEnabled; }
// In the result every lowPower value is a JSON boolean (lowPowerStart is null if the run never started).
static mach_port_t gMainThread;
static uint64_t gFootLoad;

// ---- main run loop busy time ------------------------------------------------------
static double gBusy, gAwakeAt;
static BOOL gAwake;
static void busyObserver(CFRunLoopObserverRef o, CFRunLoopActivity a, void *info) {
  double t = CACurrentMediaTime();
  if (a == kCFRunLoopAfterWaiting) { gAwake = YES; gAwakeAt = t; }
  else if (a == kCFRunLoopBeforeWaiting) { if (gAwake) gBusy += t - gAwakeAt; gAwake = NO; }
}
static double busyNow(void) { return gBusy + (gAwake ? CACurrentMediaTime() - gAwakeAt : 0); }

// ---- CPU and memory ---------------------------------------------------------------
static double procCpu(void) {
  struct rusage ru;
  getrusage(RUSAGE_SELF, &ru);
  return ru.ru_utime.tv_sec + ru.ru_stime.tv_sec + (ru.ru_utime.tv_usec + ru.ru_stime.tv_usec) / 1e6;
}
static double mainCpu(void) {
  thread_basic_info_data_t bi;
  mach_msg_type_number_t n = THREAD_BASIC_INFO_COUNT;
  if (thread_info(gMainThread, THREAD_BASIC_INFO, (thread_info_t)&bi, &n) != KERN_SUCCESS) return -1;
  return bi.user_time.seconds + bi.system_time.seconds + (bi.user_time.microseconds + bi.system_time.microseconds) / 1e6;
}
static uint64_t footprint(uint64_t *lifetimePeak) {
  task_vm_info_data_t vi;
  mach_msg_type_number_t n = TASK_VM_INFO_COUNT;
  if (task_info(mach_task_self(), TASK_VM_INFO, (task_info_t)&vi, &n) != KERN_SUCCESS) return 0;
  if (lifetimePeak) *lifetimePeak = n >= TASK_VM_INFO_REV2_COUNT ? (uint64_t)vi.ledger_phys_footprint_peak : 0;
  return vi.phys_footprint;
}


// BENCH_VM=1: footprint by VM tag (dirty + swapped of private memory), malloc in use,
// and the task ledgers, for attribution runs (perturbs timing: ~ms per walk).
#include <malloc/malloc.h>
static BOOL gVm;
static NSDictionary *vmBreakdown(void) {
  task_vm_info_data_t vi; mach_msg_type_number_t n = TASK_VM_INFO_COUNT;
  task_info(mach_task_self(), TASK_VM_INFO, (task_info_t)&vi, &n);
  NSMutableDictionary *tags = [NSMutableDictionary dictionary];
  vm_address_t addr = 0; vm_size_t size = 0; natural_t depth = 0;
  uint64_t page = vm_kernel_page_size;
  while (1) {
    vm_region_submap_info_data_64_t info; mach_msg_type_number_t c = VM_REGION_SUBMAP_INFO_COUNT_64;
    if (vm_region_recurse_64(mach_task_self(), &addr, &size, &depth, (vm_region_recurse_info_64_t)&info, &c) != KERN_SUCCESS) break;
    if (info.is_submap) { depth++; continue; }
    if (info.share_mode == SM_PRIVATE || info.share_mode == SM_COW || info.share_mode == SM_PRIVATE_ALIASED || info.share_mode == SM_LARGE_PAGE) {
      uint64_t b = (uint64_t)(info.pages_dirtied + info.pages_swapped_out) * page;
      if (b) { NSString *k = [NSString stringWithFormat:@"%u", info.user_tag]; tags[k] = @([tags[k] unsignedLongLongValue] + b); }
    }
    addr += size;
  }
  vm_address_t *zones = NULL; unsigned count = 0; size_t inUse = 0, alloc = 0;
  if (malloc_get_all_zones(mach_task_self(), NULL, &zones, &count) == KERN_SUCCESS)
    for (unsigned i = 0; i < count; i++) { malloc_statistics_t st; malloc_zone_statistics((malloc_zone_t *)zones[i], &st); inUse += st.size_in_use; alloc += st.size_allocated; }
  return @{@"footprint": @(vi.phys_footprint), @"internal": @(vi.internal), @"compressed": @(vi.compressed),
           @"graphics": @(vi.ledger_tag_graphics_footprint), @"media": @(vi.ledger_tag_media_footprint),
           @"purgeableNonvolatile": @(vi.ledger_purgeable_nonvolatile), @"mallocInUse": @(inUse), @"mallocAllocated": @(alloc), @"tags": tags};
}


// BENCH_MALLOCLOG=1: record every allocation of >= 128 KiB (size + 12 return addresses) through
// libmalloc's malloc_logger hook into a fixed ring; written as mallocLog with image slides for atos.
#include <execinfo.h>
#include <mach-o/dyld.h>
#include <mach-o/loader.h>
#include <dlfcn.h>
typedef void (malloc_logger_t)(uint32_t type, uintptr_t arg1, uintptr_t arg2, uintptr_t arg3, uintptr_t result, uint32_t skip);
extern malloc_logger_t *malloc_logger;
#define MLOG_N 200000
typedef struct { uint64_t size; double t; void *pc[12]; } MRec;
static MRec *gMlog; static volatile int64_t gMlogN;
static void mlogHook(uint32_t type, uintptr_t a1, uintptr_t a2, uintptr_t a3, uintptr_t result, uint32_t skip) {
  uint64_t size = 0;
  if ((type & 2) && (type & 4)) size = a3;          // realloc: arg2 old ptr, arg3 size
  else if (type & 2) size = a2;                      // allocate: arg2 size
  if (size < 131072 || !result) return;
  int64_t i = __atomic_fetch_add(&gMlogN, 1, __ATOMIC_RELAXED);
  if (i >= MLOG_N) return;
  gMlog[i].size = size; gMlog[i].t = CACurrentMediaTime();
  void *f[14]; int n = backtrace(f, 14);
  for (int k = 0; k < 12; k++) gMlog[i].pc[k] = k + 2 < n ? f[k + 2] : 0;
}
static NSDictionary *mlogDump(void) {
  NSMutableArray *recs = [NSMutableArray array];
  int64_t n = MIN(gMlogN, MLOG_N);
  for (int64_t i = 0; i < n; i++) {
    NSMutableArray *pcs = [NSMutableArray array];
    for (int k = 0; k < 12 && gMlog[i].pc[k]; k++) [pcs addObject:[NSString stringWithFormat:@"%p", gMlog[i].pc[k]]];
    [recs addObject:@[@(gMlog[i].size), @(gMlog[i].t), pcs]];
  }
  NSMutableArray *images = [NSMutableArray array];
  for (uint32_t i = 0; i < _dyld_image_count(); i++)
    [images addObject:@[@(_dyld_get_image_name(i)), [NSString stringWithFormat:@"%p", (void *)_dyld_get_image_header(i)]]];
  return @{@"recs": recs, @"images": images, @"total": @(gMlogN)};
}

typedef struct { double t, busy, cpu, main; } Snap;
static Snap snapNow(void) { return (Snap){CACurrentMediaTime(), busyNow(), procCpu(), mainCpu()}; }

@interface LBProbe : NSObject
@property (nonatomic, weak) UIScrollView *scroll;
@property (nonatomic, strong) CADisplayLink *link;
@property (nonatomic) NSInteger sample, frame, dumped;
@property (nonatomic, copy) NSString *scenario;
@property (nonatomic, strong) NSMutableArray<NSNumber *> *dts, *expected, *blankPts, *busyMs;
@property (nonatomic, strong) NSMutableArray *segments, *jumps, *segStats, *timeline;
@property (nonatomic) CFTimeInterval last, segStart, jumpAt, lastCleanAt;
@property (nonatomic) NSInteger seg, jumpIndex, cleanRun, segFrames, ticks;
@property (nonatomic) double firstContentMs, sampleCost, segTravel, lastBusy, lastLeading;
@property (nonatomic) NSInteger sampleCount;
@property (nonatomic, strong) NSMutableArray<NSNumber *> *setCost;
@property (nonatomic) BOOL useLayer, live, done, forceHierarchy, firstDumped;
@property (nonatomic, strong) UIImage *lastSampleImage; // BENCH_JUMPDUMP: the latest sample, kept for a timed-out jump
@property (nonatomic) Snap runSnap, segSnap;
@property (nonatomic) uint64_t footStart, footPeak, vmPeakAt, footEnd;
@property (nonatomic) NSInteger thermalStart, lowPowerStart; // -1 until read at the run's start (lowPowerStart: 0/1 once read)
@property (nonatomic, strong) NSDictionary *vmPeak, *vmEndNow;
@property (nonatomic, strong) NSDictionary *cold;
// innerfling / innerkeep
@property (nonatomic, weak) UIScrollView *driven;          // the inner list being found / driven / marked
@property (nonatomic) double placedAt;                      // when `driven` was first seen fully in view
@property (nonatomic) BOOL stepDone;
@property (nonatomic, strong) NSMutableArray *keep, *innerInfo;
@property (nonatomic, strong) NSMutableDictionary *marks;   // kind -> {outer, rect, value, read}
@property (nonatomic, strong) NSMapTable<NSString *, UIScrollView *> *markViews; // kind -> the marked view (weak)
@end

// BENCH_HUD=1: an on-screen frame-time graph, so a 60 fps screen recording
// still shows every late 120 Hz frame. Off for measured runs.
@interface LBHud : NSObject
@property (nonatomic, strong) UIWindow *window;
@property (nonatomic, strong) UILabel *label;
@property (nonatomic, strong) CAShapeLayer *ok, *late;
@property (nonatomic, strong) NSMutableArray<NSNumber *> *frames;
@property (nonatomic) double windowStart, lateMs;
@property (nonatomic) NSInteger count;
@end
@implementation LBHud
- (instancetype)initOn:(UIWindowScene *)scene {
  if (!(self = [super init])) return nil;
  CGFloat w = 360, h = 96;
  self.window = [[UIWindow alloc] initWithWindowScene:scene];
  self.window.frame = CGRectMake(scene.coordinateSpace.bounds.size.width - w - 16, 60, w, h);
  self.window.windowLevel = UIWindowLevelAlert + 1;
  self.window.userInteractionEnabled = NO;
  self.window.backgroundColor = [UIColor colorWithWhite:0 alpha:0.72];
  self.window.layer.cornerRadius = 10; self.window.clipsToBounds = YES;
  self.label = [[UILabel alloc] initWithFrame:CGRectMake(10, 6, w - 20, 22)];
  self.label.font = [UIFont monospacedDigitSystemFontOfSize:15 weight:UIFontWeightSemibold];
  self.label.textColor = UIColor.whiteColor;
  [self.window addSubview:self.label];
  self.ok = [CAShapeLayer layer]; self.ok.strokeColor = UIColor.systemGreenColor.CGColor; self.ok.lineWidth = 1.2;
  self.late = [CAShapeLayer layer]; self.late.strokeColor = UIColor.systemRedColor.CGColor; self.late.lineWidth = 1.2;
  for (CAShapeLayer *l in @[self.ok, self.late]) { l.frame = CGRectMake(10, 32, w - 20, h - 40); [self.window.layer addSublayer:l]; }
  self.frames = [NSMutableArray array];
  self.window.hidden = NO;
  return self;
}
- (void)frame:(double)dt expected:(double)exp now:(double)now {
  if (dt <= 0) return;
  [self.frames addObject:@(dt)];
  if (self.frames.count > 240) [self.frames removeObjectAtIndex:0];
  self.count++; if (dt > exp * 1.5) self.lateMs += (dt - exp) * 1000;
  if (now - self.windowStart >= 0.5) {
    double span = now - self.windowStart;
    NSString *app = NSBundle.mainBundle.bundleIdentifier.pathExtension.uppercaseString ?: @"";
    NSDictionary *names = @{@"SWIFTUI": @"SwiftUI", @"EXACT": @"Exact2", @"SF": @"Exact2", @"OM": @"Exact2", @"MEM": @"Exact2", @"ADOPT": @"Exact2", @"LM": @"Exact2", @"RM": @"Exact2", @"RMBASE": @"Exact2", @"SWIFTUIRM": @"SwiftUI", @"EXPO": @"Expo", @"SVG": @"Exact2 SVG", @"SVGP": @"Exact2 SVG", @"SVGBASE": @"Exact2 SVG", @"CM": @"Exact2 SVG", @"CMBASE": @"Exact2 SVG", @"LMAP": @"Exact2 SVG", @"LMAPBASE": @"Exact2 SVG", @"GPU": @"Exact2 GPU", @"EXACT2": @"Exact2", @"MAPREUSE": @"Exact2 maps", @"MAPBASE": @"Exact2 base"};
    if (names[app]) app = names[app];
    self.label.text = [NSString stringWithFormat:@"%@  %.0f fps  late %.0f ms/s", app, self.count / span, self.lateMs / span];
    self.windowStart = now; self.count = 0; self.lateMs = 0;
  }
  CGFloat W = self.ok.bounds.size.width, H = self.ok.bounds.size.height, bw = W / 240;
  UIBezierPath *ok = [UIBezierPath bezierPath], *late = [UIBezierPath bezierPath];
  [self.frames enumerateObjectsUsingBlock:^(NSNumber *f, NSUInteger i, BOOL *stop) {
    double v = f.doubleValue; CGFloat bh = MIN(H, v / 0.050 * H); CGFloat x = i * bw + bw / 2;
    UIBezierPath *p = v > exp * 1.5 ? late : ok;
    [p moveToPoint:CGPointMake(x, H)]; [p addLineToPoint:CGPointMake(x, H - bh)];
  }];
  [CATransaction begin]; [CATransaction setDisableActions:YES];
  self.ok.path = ok.CGPath; self.late.path = late.CGPath;
  [CATransaction commit];
}
@end
static LBHud *gHud;

@implementation LBProbe

static UIWindow *keyWindow(void) {
  for (UIScene *s in UIApplication.sharedApplication.connectedScenes) {
    if (![s isKindOfClass:UIWindowScene.class]) continue;
    for (UIWindow *w in ((UIWindowScene *)s).windows) if (w.isKeyWindow) return w;
  }
  return nil;
}

// ---- Metal content under the layer sampler -------------------------------------------
// `renderInContext` does not draw a CAMetalLayer's presented content on iOS 17 (it draws its
// background), so a GPU canvas that is showing a frame would read as a blank band. Every
// CAMetalLayer that has vended a drawable (`nextDrawable`, swizzled below, any thread) is
// remembered; after a layer-sampler render, each such layer that is visible in the window
// (attached, not hidden, opacity > 0 up its chain) is painted over in the sample as a 2 x 2
// dark/light checker, so every scanline and column across it reads as ink. A Metal layer that has
// never drawn stays as rendered (its background), so an unbuilt or undrawn region still reads blank.
// The same rule applies to every app; SwiftUI's and Expo's rows here have no CAMetalLayer.
static double gInkMs; static NSInteger gInkFrames; // `still`: process start -> first frame whose ink >= 90 % of the settled ink
static NSMutableArray<NSArray *> *gInkSamples;   // [ms, ink fraction] per frame until the measured segment starts
static void settleInk(void);
static CADisplayLink *gInkLink;
static NSString *exeUUID(void) {
  const struct mach_header_64 *h = (const struct mach_header_64 *)_dyld_get_image_header(0);
  if (!h) return @"";
  const uint8_t *p = (const uint8_t *)(h + 1);
  for (uint32_t i = 0; i < h->ncmds; i++) {
    const struct load_command *lc = (const struct load_command *)p;
    if (lc->cmd == LC_UUID) { NSUUID *u = [[NSUUID alloc] initWithUUIDBytes:((const struct uuid_command *)lc)->uuid]; return u.UUIDString; }
    p += lc->cmdsize;
  }
  return @"";
}
static BOOL envOn(const char *k) { const char *v = getenv(k); return v && *v && strcmp(v, "0"); }
static NSString *gJumpDump; // BENCH_JUMPDUMP=<dir under Documents>
static NSHashTable<CALayer *> *gDrewMetal;
static NSMapTable<CALayer *, NSString *> *gDrewHow; // layer -> which path vended its drawable (BENCH_METALDBG)
static IMP gOrigNextDrawable;
static void noteDrew(CALayer *l, NSString *how) {
  if (!l) return;
  @synchronized(gDrewMetal) { [gDrewMetal addObject:l]; if (![gDrewHow objectForKey:l]) [gDrewHow setObject:how forKey:l]; }
}
static id probe_nextDrawable(id self, SEL _cmd) {
  id d = ((id (*)(id, SEL))gOrigNextDrawable)(self, _cmd);
  if (d) noteDrew(self, @"CAMetalLayer");
  return d;
}
// A CAMetalLayer subclass that overrides -nextDrawable without calling super never reaches the base-class
// swizzle: such an override is wrapped the first time a layer of that class is met under the window.
static NSMutableSet<NSString *> *gHookedClasses;
static void hookMetalSubclass(Class c) {
  Class base = NSClassFromString(@"CAMetalLayer");
  if (!base || c == base) return;
  BOOL sub = NO; for (Class k = class_getSuperclass(c); k; k = class_getSuperclass(k)) if (k == base) { sub = YES; break; }
  if (!sub) return;
  unsigned n = 0; Method *ms = class_copyMethodList(c, &n); Method own = NULL;
  for (unsigned i = 0; i < n; i++) if (method_getName(ms[i]) == @selector(nextDrawable)) { own = ms[i]; break; }
  free(ms);
  if (!own) return;
  NSString *name = NSStringFromClass(c);
  @synchronized(gHookedClasses) { if ([gHookedClasses containsObject:name]) return; [gHookedClasses addObject:name]; }
  IMP orig = method_getImplementation(own);
  method_setImplementation(own, imp_implementationWithBlock(^id(id me) {
    id d = ((id (*)(id, SEL))orig)(me, @selector(nextDrawable));
    if (d) noteDrew(me, name);
    return d;
  }));
}
// iOS 17's CAMetalDisplayLink hands drawables out through its update object, not -nextDrawable.
static IMP gOrigLinkDrawable;
static id probe_linkDrawable(id self, SEL _cmd) {
  id d = ((id (*)(id, SEL))gOrigLinkDrawable)(self, _cmd);
  if (d && [d respondsToSelector:@selector(layer)]) noteDrew([d layer], @"CAMetalDisplayLink");
  return d;
}
static void installMetalHook(void) {
  gDrewMetal = [NSHashTable weakObjectsHashTable];
  gDrewHow = [NSMapTable weakToStrongObjectsMapTable];
  gHookedClasses = [NSMutableSet set];
  Class c = NSClassFromString(@"CAMetalLayer");
  Method m = c ? class_getInstanceMethod(c, @selector(nextDrawable)) : NULL;
  if (m) gOrigNextDrawable = method_setImplementation(m, (IMP)probe_nextDrawable);
  Class u = NSClassFromString(@"CAMetalDisplayLinkUpdate");
  Method um = u ? class_getInstanceMethod(u, @selector(drawable)) : NULL;
  if (um) gOrigLinkDrawable = method_setImplementation(um, (IMP)probe_linkDrawable);
  // No image scan (removed 2026-09-29): scanning realized every ObjC class of every loaded image in the
  // constructor, ~317 ms pre-main on exact2 against far less for smaller apps (it scales with linked
  // frameworks). MapKit on iOS 17/27 draws through a plain CAMetalLayer, which the base swizzle sees.
  // A subclass with its own -nextDrawable is hooked lazily when the layer walk first meets it (markMetal).
}
// BENCH_METALDBG: every layer under the window that is a CAMetalLayer, or whose class names Metal,
// VectorKit (VK), MapKit (MK) or has IOSurface contents, with whether a drawable was seen for it.
static void dumpMetalish(CALayer *l, CALayer *root, int depth, int *budget) {
  if (*budget <= 0 || depth > 60) return;
  NSString *cn = NSStringFromClass(l.class);
  id contents = l.contents;
  BOOL surf = contents && CFGetTypeID((__bridge CFTypeRef)contents) != CGImageGetTypeID();
  BOOL metal = [l isKindOfClass:NSClassFromString(@"CAMetalLayer")];
  if (metal || surf || [cn containsString:@"Metal"] || [cn hasPrefix:@"VK"] || [cn hasPrefix:@"MK"] || [cn hasPrefix:@"_MK"]) {
    NSString *how; @synchronized(gDrewMetal) { how = [gDrewHow objectForKey:l]; }
    NSLog(@"[probe-metal] tree d=%d %@ metal=%d contents=%@ drew=%@ hidden=%d op=%.2f frame=%@", depth, cn, metal,
          contents ? NSStringFromClass([contents class]) : @"-", how ?: @"no", l.hidden, (l.presentationLayer ?: l).opacity,
          NSStringFromCGRect([l convertRect:l.bounds toLayer:root]));
    (*budget)--;
  }
  for (CALayer *s in l.sublayers) dumpMetalish(s, root, depth + 1, budget);
}
static void hookSubclassesUnder(CALayer *l, int depth) {
  if (depth > 80) return;
  if (object_getClass(l) != NSClassFromString(@"CAMetalLayer") && [l isKindOfClass:NSClassFromString(@"CAMetalLayer")]) hookMetalSubclass(object_getClass(l));
  for (CALayer *s in l.sublayers) hookSubclassesUnder(s, depth + 1);
}
static void markMetal(CGContextRef ctx, UIWindow *w) {
  static CFTimeInterval lastWalk = 0; // lazy subclass hooks: at most one layer walk per second, while sampling
  CFTimeInterval tw = CACurrentMediaTime();
  if (tw - lastWalk > 1.0) { lastWalk = tw; hookSubclassesUnder(w.layer, 0); }
  NSArray<CALayer *> *layers;
  @synchronized(gDrewMetal) { layers = gDrewMetal.allObjects; }
  static int dbg = 0;
  if (envOn("BENCH_METALDBG") && dbg++ < 40) {
    for (CALayer *l in layers) {
      NSMutableString *why = [NSMutableString string];
      for (CALayer *a = l; a; a = a.superlayer) { CALayer *pa = a.presentationLayer ?: a; if (a.hidden) [why appendFormat:@" hidden:%@", a.class]; if (pa.opacity <= 0.01) [why appendFormat:@" op0:%@", a.class]; }
      NSLog(@"[probe-metal] n=%lu layer=%@ reachesWindow=%d frame=%@%@", (unsigned long)layers.count, l.class, ({BOOL r=NO; for (CALayer *a=l;a;a=a.superlayer) if (a==w.layer) {r=YES;break;} r;}), NSStringFromCGRect([l convertRect:l.bounds toLayer:w.layer]), why);
    }
    if (!layers.count) NSLog(@"[probe-metal] no drawn Metal layers (hook %p)", gOrigNextDrawable);
  }
  static long calls = 0; const char *md = getenv("BENCH_METALDBG");
  if (md && *md && calls++ % MAX(1, atoi(md)) == 0) {
    int budget = 40;
    NSLog(@"[probe-metal] sample %ld: hooked subclasses %@ link hook %p", calls, gHookedClasses, gOrigLinkDrawable);
    dumpMetalish(w.layer, w.layer, 0, &budget);
  }
  for (CALayer *l in layers) {
    // Attached under this window's layer (the window's own layer has a superlayer of its own).
    BOOL shown = YES, inWindow = NO;
    for (CALayer *a = l; a; a = a.superlayer) {
      CALayer *pa = a.presentationLayer ?: a;
      if (a.hidden || pa.opacity <= 0.01) { shown = NO; break; }
      if (a == w.layer) { inWindow = YES; break; }
    }
    if (!shown || !inWindow) continue;
    CGRect f = [l convertRect:l.bounds toLayer:w.layer];
    if (CGRectIsEmpty(f)) continue;
    CGFloat hw = f.size.width / 2, hh = f.size.height / 2;
    CGContextSetGrayFillColor(ctx, 0.25, 1);
    CGContextFillRect(ctx, CGRectMake(f.origin.x, f.origin.y, hw, hh));
    CGContextFillRect(ctx, CGRectMake(f.origin.x + hw, f.origin.y + hh, hw, hh));
    CGContextSetGrayFillColor(ctx, 0.75, 1);
    CGContextFillRect(ctx, CGRectMake(f.origin.x + hw, f.origin.y, hw, hh));
    CGContextFillRect(ctx, CGRectMake(f.origin.x, f.origin.y + hh, hw, hh));
  }
}

static void collect(UIView *v, NSMutableArray *out) {
  if ([v isKindOfClass:UIScrollView.class]) [out addObject:v];
  for (UIView *c in v.subviews) collect(c, out);
}

- (UIScrollView *)findScroll {
  UIWindow *w = keyWindow();
  if (!w) return nil;
  NSMutableArray *all = [NSMutableArray array];
  collect(w, all);
  UIScrollView *best = nil;
  for (UIScrollView *s in all) {
    if (s.bounds.size.height < 300) continue;
    if (!best || s.contentSize.height > best.contentSize.height) best = s;
  }
  return best.contentSize.height > 20000 ? best : nil;
}

// Sum of ink-free horizontal bands at least 60 pt tall inside the list's
// visible rect, sampled at quarter scale. A band is a run of scanlines whose
// luminance range across the inner width is < 10. Unlike the listbench probe, the rect
// excludes the scroll view's adjustedContentInset top and bottom (status bar, bars, home
// indicator): at offset 0 that strip is empty by design and would never read as clean.
- (double)blankPoints {
  UIScrollView *s = self.scroll;
  UIWindow *w = s.window;
  if (!w) return -1;
  UIEdgeInsets in = s.adjustedContentInset;
  CGRect vis = s.bounds;
  vis.origin.y += in.top; vis.size.height -= in.top + in.bottom;
  CGRect r = [s convertRect:vis toView:w];
  r = CGRectIntersection(r, w.bounds);
  const CGFloat scale = 0.25, margin = 28;
  size_t W = (size_t)(r.size.width * scale), H = (size_t)(r.size.height * scale);
  if (W < 8 || H < 8) return -1;
  uint8_t *px = calloc(W * H * 4, 1);
  CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
  CGContextRef ctx = CGBitmapContextCreate(px, W, H, 8, W * 4, cs, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
  CGColorSpaceRelease(cs);
  UIGraphicsPushContext(ctx);
  CGContextTranslateCTM(ctx, 0, H);
  CGContextScaleCTM(ctx, scale, -scale);
  CGContextTranslateCTM(ctx, -r.origin.x, -r.origin.y);
  CFTimeInterval t0 = CACurrentMediaTime();
  if (self.useLayer && !self.forceHierarchy) { [w.layer.presentationLayer ?: w.layer renderInContext:ctx]; markMetal(ctx, w); }
  else [w drawViewHierarchyInRect:w.bounds afterScreenUpdates:NO];
  if (!self.forceHierarchy) { self.sampleCost += CACurrentMediaTime() - t0; self.sampleCount++; }
  UIGraphicsPopContext();
  if (gJumpDump) { CGImageRef im = CGBitmapContextCreateImage(ctx); self.lastSampleImage = [UIImage imageWithCGImage:im]; CGImageRelease(im); }
  size_t x0 = (size_t)(margin * scale), x1 = W - x0;
  double blank = 0; size_t run = 0;
  const size_t minRun = (size_t)(60 * scale);
  self.lastLeading = 0; // a band that starts at the top edge of the visible rect (coldstart uses it)
  for (size_t y = 0; y <= H; y++) {
    BOOL uniform = NO;
    if (y < H) {
      int lo = 255, hi = 0;
      for (size_t x = x0; x < x1; x++) {
        uint8_t *p = px + (y * W + x) * 4;
        int l = (p[0] * 3 + p[1] * 6 + p[2]) / 10;
        if (l < lo) lo = l;
        if (l > hi) hi = l;
      }
      uniform = (hi - lo) < 10;
    }
    if (uniform) run++;
    else {
      if (run >= minRun) { blank += run / scale; if (y == run) self.lastLeading = run / scale; }
      run = 0;
    }
  }
  NSString *dir = NSProcessInfo.processInfo.environment[@"BENCH_DUMP"];
  if (dir.length && ![dir hasPrefix:@"/"]) dir = [NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES).firstObject stringByAppendingPathComponent:dir];
  if (!dir.length) dir = nil;
  if (dir && blank > 0 && self.dumped < 6) {
    [NSFileManager.defaultManager createDirectoryAtPath:dir withIntermediateDirectories:YES attributes:nil error:nil];
    CGImageRef img = CGBitmapContextCreateImage(ctx);
    NSData *png = UIImagePNGRepresentation([UIImage imageWithCGImage:img]);
    [png writeToFile:[dir stringByAppendingFormat:@"/blank-%ld-%.0f.png", (long)self.dumped, blank] atomically:YES];
    CGImageRelease(img); self.dumped++;
  }
  CGContextRelease(ctx);
  free(px);
  return blank;
}

// ---- inner lists (innerfling, innerkeep) --------------------------------------------
static BOOL isInner(NSString *sc) { return [sc isEqualToString:@"innerfling"] || [sc isEqualToString:@"innerkeep"]; }
static BOOL horizontalKind(NSString *kind) { return [kind isEqualToString:@"filmstrip"]; }
static CGRect visibleRect(UIScrollView *s) { // the outer list's visible content rect, insets excluded
  UIEdgeInsets in = s.adjustedContentInset; CGRect v = s.bounds;
  v.origin.y += in.top; v.size.height -= in.top + in.bottom; return v;
}
static CGFloat innerOffset(UIScrollView *v, BOOL x) {
  return x ? v.contentOffset.x + v.adjustedContentInset.left : v.contentOffset.y + v.adjustedContentInset.top;
}
static void setInnerOffset(UIScrollView *v, BOOL x, CGFloat value) {
  CGPoint o = v.contentOffset;
  if (x) {
    CGFloat lo = -v.adjustedContentInset.left, hi = MAX(lo, v.contentSize.width - v.bounds.size.width + v.adjustedContentInset.right);
    o.x = MAX(lo, MIN(hi, value - v.adjustedContentInset.left));
  } else {
    CGFloat lo = -v.adjustedContentInset.top, hi = MAX(lo, v.contentSize.height - v.bounds.size.height + v.adjustedContentInset.bottom);
    o.y = MAX(lo, MIN(hi, value - v.adjustedContentInset.top));
  }
  v.contentOffset = o;
}

/// The outer list's descendant scroll views that look like `kind`'s inner list.
- (NSArray<UIScrollView *> *)innerOfKind:(NSString *)kind {
  NSMutableArray *all = [NSMutableArray array], *out = [NSMutableArray array];
  collect(self.scroll, all);
  CGFloat W = self.scroll.bounds.size.width;
  for (UIScrollView *v in all) {
    if (v == self.scroll || !v.window || v.hidden || v.alpha < 0.01) continue;
    CGSize c = v.contentSize, b = v.bounds.size;
    BOOL ok = horizontalKind(kind) ? (c.width >= 20000 && b.height < 300)
                                   : (c.height >= 20000 && b.height >= 300 && b.height <= 600 && b.width < W);
    if (ok) [out addObject:v];
  }
  return out;
}

/// One find tick: done once an inner list of `kind` has been fully in view for 0.8 s.
- (BOOL)findStep:(NSString *)kind now:(double)now {
  UIScrollView *s = self.scroll;
  CGRect vis = visibleRect(s);
  UIScrollView *inView = nil, *nearest = nil; CGRect nf = CGRectZero;
  for (UIScrollView *v in [self innerOfKind:kind]) {
    CGRect f = [v convertRect:v.bounds toView:s];
    if (CGRectContainsRect(CGRectInset(vis, -1, -1), f)) { if (!inView || v == self.driven) inView = v; }
    else if (!nearest || fabs(CGRectGetMidY(f) - CGRectGetMidY(vis)) < fabs(CGRectGetMidY(nf) - CGRectGetMidY(vis))) { nearest = v; nf = f; }
  }
  if (inView) {
    if (inView != self.driven || self.placedAt <= 0) { self.driven = inView; self.placedAt = now; }
    return now - self.placedAt >= 0.8;
  }
  self.placedAt = 0;
  CGFloat lo = -s.adjustedContentInset.top, hi = s.contentSize.height - s.bounds.size.height + s.adjustedContentInset.bottom;
  CGPoint o = s.contentOffset;
  if (nearest) o.y = nf.origin.y - s.adjustedContentInset.top - 0.25 * vis.size.height; // its top at a quarter down
  else o.y += 0.6 * vis.size.height;                                                      // search downward
  o.y = MAX(lo, MIN(hi, o.y));
  s.contentOffset = o;
  return NO;
}

/// Blank bands inside an inner list: columns (strip) or rows (inbox) of uniform luminance >= 60 pt.
- (double)innerBlank:(UIScrollView *)v horizontal:(BOOL)x {
  UIWindow *w = v.window;
  if (!w) return -1;
  CGRect r = CGRectIntersection([v convertRect:v.bounds toView:w], w.bounds);
  const CGFloat scale = 0.5, margin = 4;
  size_t W = (size_t)(r.size.width * scale), H = (size_t)(r.size.height * scale);
  if (W < 8 || H < 8) return -1;
  uint8_t *px = calloc(W * H * 4, 1);
  CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
  CGContextRef ctx = CGBitmapContextCreate(px, W, H, 8, W * 4, cs, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
  CGColorSpaceRelease(cs);
  UIGraphicsPushContext(ctx);
  CGContextTranslateCTM(ctx, 0, H);
  CGContextScaleCTM(ctx, scale, -scale);
  CGContextTranslateCTM(ctx, -r.origin.x, -r.origin.y);
  CFTimeInterval t0 = CACurrentMediaTime();
  if (self.useLayer) { [w.layer.presentationLayer ?: w.layer renderInContext:ctx]; markMetal(ctx, w); }
  else [w drawViewHierarchyInRect:w.bounds afterScreenUpdates:NO];
  self.sampleCost += CACurrentMediaTime() - t0; self.sampleCount++;
  UIGraphicsPopContext();
  size_t m = (size_t)(margin * scale), minRun = (size_t)(60 * scale);
  size_t lines = x ? W : H, across0 = m, across1 = (x ? H : W) - m;
  double blank = 0; size_t run = 0;
  for (size_t i = 0; i <= lines; i++) {
    BOOL uniform = NO;
    if (i < lines) {
      int lo = 255, hi = 0;
      for (size_t j = across0; j < across1; j++) {
        uint8_t *p = x ? px + (j * W + i) * 4 : px + (i * W + j) * 4;
        int l = (p[0] * 3 + p[1] * 6 + p[2]) / 10;
        if (l < lo) lo = l;
        if (l > hi) hi = l;
      }
      uniform = (hi - lo) < 10;
    }
    if (uniform) run++;
    else { if (run >= minRun) blank += run / scale; run = 0; }
  }
  CGContextRelease(ctx);
  free(px);
  return blank;
}

/// Luminance of an inner list's box as the screen shows it (drawViewHierarchyInRect, 1 px per pt),
/// for innerkeep's anchor check.
static NSData *boxLuma(UIScrollView *v, size_t *W, size_t *H) {
  UIWindow *w = v.window; if (!w) return nil;
  CGRect r = CGRectIntersection([v convertRect:v.bounds toView:w], w.bounds);
  *W = (size_t)r.size.width; *H = (size_t)r.size.height;
  if (*W < 16 || *H < 16) return nil;
  uint8_t *px = calloc(*W * *H * 4, 1);
  CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
  CGContextRef ctx = CGBitmapContextCreate(px, *W, *H, 8, *W * 4, cs, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
  CGColorSpaceRelease(cs);
  UIGraphicsPushContext(ctx);
  CGContextTranslateCTM(ctx, 0, *H); CGContextScaleCTM(ctx, 1, -1);
  CGContextTranslateCTM(ctx, -r.origin.x, -r.origin.y);
  [w drawViewHierarchyInRect:w.bounds afterScreenUpdates:NO];
  UIGraphicsPopContext();
  NSMutableData *l = [NSMutableData dataWithLength:*W * *H]; uint8_t *o = l.mutableBytes;
  for (size_t i = 0; i < *W * *H; i++) { uint8_t *p = px + i * 4; o[i] = (uint8_t)((p[0] * 3 + p[1] * 6 + p[2]) / 10); }
  CGContextRelease(ctx); free(px);
  return l;
}
/// The shift (pt, along the list's axis) at which `after` best matches `before`: content at p before is
/// at p + shift after. Match = the fraction of overlapping pixels within 12 luma levels (4 pt edge
/// margin excluded), counted only where either side has ink (luma < 240), so a shared white background
/// does not make different content look alike; a shift must overlap at least half the box.
/// Returns the best shift; *best and *zero get the match there and at shift 0.
/// A 5 x 5 box blur (2 pt each way): content anchored at a fractional pixel re-rasterizes its text and
/// image edges differently; the blur keeps that from reading as different content (validated on the
/// iPad dumps 2026-09-29: the same inbox reads 0.54-0.77 unblurred, 1.0 blurred; a lost one 0.43).
static NSData *boxBlur(NSData *src, size_t W, size_t H) {
  const uint8_t *a = src.bytes; NSMutableData *o = [NSMutableData dataWithLength:W * H]; uint8_t *b = o.mutableBytes;
  for (size_t y = 0; y < H; y++) for (size_t x = 0; x < W; x++) {
    int sum = 0, n = 0;
    for (long dy = -2; dy <= 2; dy++) for (long dx = -2; dx <= 2; dx++) {
      long yy = (long)y + dy, xx = (long)x + dx;
      if (yy < 0 || xx < 0 || yy >= (long)H || xx >= (long)W) continue;
      sum += a[yy * W + xx]; n++;
    }
    b[y * W + x] = (uint8_t)(sum / n);
  }
  return o;
}
static int anchorShift(NSData *before0, NSData *after0, size_t W, size_t H, BOOL x, int maxShift, double *best, double *zero) {
  NSData *before = boxBlur(before0, W, H), *after = boxBlur(after0, W, H);
  const uint8_t *a = before.bytes, *b = after.bytes; int bs = 0; *best = -1; *zero = 0;
  for (int s = -maxShift; s <= maxShift; s++) {
    size_t n = 0, ok = 0;
    for (size_t y = 4; y + 4 < H; y++) for (size_t xx = 4; xx + 4 < W; xx++) {
      long ty = x ? (long)y : (long)y + s, tx = x ? (long)xx + s : (long)xx;
      if (ty < 4 || tx < 4 || ty + 4 >= (long)H || tx + 4 >= (long)W) continue;
      int pa = a[y * W + xx], pb = b[ty * W + tx];
      if (pa >= 240 && pb >= 240) continue;
      n++; if (abs(pa - pb) <= 16) ok++;
    }
    if ((size_t)abs(s) * 2 > (x ? W : H)) continue;
    double m = n ? (double)ok / n : 0;
    if (s == 0) *zero = m;
    if (m > *best + 1e-9 || (fabs(m - *best) < 1e-9 && abs(s) < abs(bs))) { *best = m; bs = s; }
  }
  return bs;
}

/// Actions at a step's start (mark) and end (mark read-back, return, check) — innerkeep.
- (void)enterStep:(NSDictionary *)seg {
  if (seg[@"find"]) { self.driven = nil; self.placedAt = 0; self.stepDone = NO; }
  NSString *kind = seg[@"mark"];
  if (kind && self.driven) {
    UIScrollView *v = self.driven; BOOL x = horizontalKind(kind);
    setInnerOffset(v, x, [seg[@"to"] doubleValue]);
    CGRect rect = [v.superview convertRect:v.frame toView:nil];
    self.marks[kind] = @{@"outer": @(self.scroll.contentOffset.y), @"rect": [NSValue valueWithCGRect:rect],
                         @"value": @(innerOffset(v, x))};
    [self.markViews setObject:v forKey:kind];
  }
}
- (void)exitStep:(NSDictionary *)seg {
  if (seg[@"find"] && self.driven) {
    UIScrollView *v = self.driven;
    [self.innerInfo addObject:@{@"kind": seg[@"find"], @"class": NSStringFromClass(v.class),
                                @"bounds": NSStringFromCGSize(v.bounds.size), @"content": NSStringFromCGSize(v.contentSize),
                                @"outerOffset": @(self.scroll.contentOffset.y)}];
  }
  NSString *kind = seg[@"mark"];
  if (kind && self.marks[kind] && self.driven) {
    NSMutableDictionary *m = [self.marks[kind] mutableCopy];
    m[@"read"] = @(innerOffset(self.driven, horizontalKind(kind)));
    size_t W = 0, H = 0; NSData *l = boxLuma(self.driven, &W, &H); // the anchor: what the box shows after the 0.8 s hold
    if (l) { m[@"luma"] = l; m[@"lw"] = @(W); m[@"lh"] = @(H); }
    self.marks[kind] = m;
  }
  kind = seg[@"returnTo"];
  if (kind && self.marks[kind]) {
    CGPoint o = self.scroll.contentOffset; o.y = [self.marks[kind][@"outer"] doubleValue]; self.scroll.contentOffset = o;
  }
  kind = seg[@"check"];
  if (kind && self.marks[kind]) {
    NSDictionary *m = self.marks[kind];
    CGRect want = [m[@"rect"] CGRectValue];
    UIScrollView *best = nil; double bd = 1e9;
    for (UIScrollView *v in [self innerOfKind:kind]) {
      CGRect f = [v.superview convertRect:v.frame toView:nil];
      double d = hypot(f.origin.x - want.origin.x, f.origin.y - want.origin.y);
      if (d < bd) { bd = d; best = v; }
    }
    NSMutableDictionary *r = [@{@"kind": kind, @"trip": seg[@"trip"] ?: @"", @"mark": m[@"value"],
                                @"markRead": m[@"read"] ?: [NSNull null]} mutableCopy];
    if (best && bd < 100) {
      double got = innerOffset(best, horizontalKind(kind));
      r[@"got"] = @(got); r[@"err"] = @(got - [m[@"value"] doubleValue]); r[@"rowShiftPt"] = @(bd);
      r[@"sameView"] = @(best == [self.markViews objectForKey:kind]);
      r[@"kept"] = @(fabs(got - [m[@"value"] doubleValue]) < 1); // raw offset (secondary)
      r[@"errRead"] = m[@"read"] ? @(got - [m[@"read"] doubleValue]) : [NSNull null];
      // Primary (web standard, scroll anchoring allowed): the same content at the same on-screen place
      // within the box, +/- 1 pt, against what the box showed at the mark.
      size_t W = 0, H = 0; NSData *l = boxLuma(best, &W, &H);
      if (l && m[@"luma"] && W == [m[@"lw"] unsignedLongValue] && H == [m[@"lh"] unsignedLongValue]) {
        double bm = 0, zm = 0;
        int sh = anchorShift(m[@"luma"], l, W, H, horizontalKind(kind), horizontalKind(kind) ? 60 : 150, &bm, &zm);
        r[@"anchorShiftPt"] = @(sh); r[@"anchorMatch"] = @(bm); r[@"anchorMatch0"] = @(zm);
        r[@"keptAnchor"] = @(abs(sh) <= 1 && bm >= 0.9);
        if (gJumpDump) { // validation: the two luma images
          NSString *dir = [NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES).firstObject stringByAppendingPathComponent:gJumpDump];
          [NSFileManager.defaultManager createDirectoryAtPath:dir withIntermediateDirectories:YES attributes:nil error:nil];
          NSArray *pair = @[m[@"luma"], l];
          for (int i = 0; i < 2; i++) {
            CGColorSpaceRef g = CGColorSpaceCreateDeviceGray();
            CGContextRef c = CGBitmapContextCreate((void *)[pair[i] bytes], W, H, 8, W, g, (CGBitmapInfo)kCGImageAlphaNone);
            CGImageRef im = CGBitmapContextCreateImage(c);
            [UIImagePNGRepresentation([UIImage imageWithCGImage:im]) writeToFile:[dir stringByAppendingFormat:@"/keep-%@-%@-%@.png", kind, seg[@"trip"] ?: @"", i ? @"after" : @"before"] atomically:YES];
            CGImageRelease(im); CGContextRelease(c); CGColorSpaceRelease(g);
          }
        }
      } else { r[@"keptAnchor"] = @NO; r[@"anchorNote"] = [NSString stringWithFormat:@"box %zux%zu vs %@x%@", W, H, m[@"lw"], m[@"lh"]]; }
      self.driven = best; // the next mark uses the list now showing this row
    } else { r[@"got"] = [NSNull null]; r[@"kept"] = @NO; r[@"keptAnchor"] = @NO; r[@"note"] = @"no inner list at the marked place"; }
    [self.keep addObject:r];
  }
}

- (void)trackPeak {
  uint64_t f = footprint(NULL);
  if (f > self.footPeak) {
    self.footPeak = f;
    if (gVm && f > self.vmPeakAt + 4000000) { self.vmPeakAt = f; self.vmPeak = vmBreakdown(); }
  }
}

- (void)setup {
  NSDictionary *env = NSProcessInfo.processInfo.environment;
  self.scenario = env[@"BENCH_SCENARIO"] ?: @"fling";
  self.sample = [env[@"BENCH_SAMPLE"] integerValue];
  NSString *render = env[@"BENCH_RENDER"];
  self.useLayer = [render isEqualToString:@"layer"] || (!render.length && [self.scenario isEqualToString:@"coldstart"]);
  self.live = [env[@"BENCH_LIVE"] length] && ![env[@"BENCH_LIVE"] isEqualToString:@"0"];
  self.dts = [NSMutableArray array]; self.expected = [NSMutableArray array]; self.busyMs = [NSMutableArray array];
  self.blankPts = [NSMutableArray array]; self.setCost = [NSMutableArray array]; self.jumps = [NSMutableArray array];
  self.segments = [NSMutableArray array]; self.segStats = [NSMutableArray array]; self.timeline = [NSMutableArray array];
  self.keep = [NSMutableArray array]; self.innerInfo = [NSMutableArray array]; self.marks = [NSMutableDictionary dictionary];
  self.markViews = [NSMapTable strongToWeakObjectsMapTable];
  self.footStart = self.footPeak = footprint(NULL);
  self.thermalStart = thermalNow(); self.lowPowerStart = lowPowerNow();
  self.link = [CADisplayLink displayLinkWithTarget:self selector:@selector(tick:)];
  float maxFps = (float)UIScreen.mainScreen.maximumFramesPerSecond;
  self.link.preferredFrameRateRange = CAFrameRateRangeMake(maxFps, maxFps, maxFps);
  self.last = 0; self.seg = -1; self.jumpIndex = -1;
}

- (void)startColdstart {
  [self setup];
  self.sample = 1;
  self.runSnap = snapNow();
  [self.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
}

- (void)start {
  [self setup];
  if ([NSProcessInfo.processInfo.environment[@"BENCH_HUD"] isEqualToString:@"1"] && !gHud) {
    for (UIScene *sc in UIApplication.sharedApplication.connectedScenes)
      if ([sc isKindOfClass:UIWindowScene.class]) { gHud = [[LBHud alloc] initOn:(UIWindowScene *)sc]; break; }
  }
  [self.segments addObject:@{@"v": @3000, @"dir": @1, @"warm": @1}];
  [self.segments addObject:@{@"v": @3000, @"dir": @-1, @"warm": @1}];
  NSArray *speeds = [self.scenario isEqualToString:@"ladder"] ? @[@3000, @6000, @12000, @24000, @48000, @96000]
                                                              : @[@1000, @3000, @6000, @12000, @24000];
  for (NSNumber *v in speeds) {
    [self.segments addObject:@{@"v": v, @"dir": @1}];
    [self.segments addObject:@{@"v": v, @"dir": @-1}];
  }
  if ([self.scenario isEqualToString:@"still"]) {
    // The features benchmark: no scroll view; a 3 s warm-up then one 10 s measured segment.
    [self.segments removeAllObjects];
    [self.segments addObject:@{@"v": @0, @"dir": @1, @"rest": @1, @"warm": @1, @"dur": @3}];
    [self.segments addObject:@{@"v": @0, @"dir": @1, @"rest": @1, @"dur": @10}];
    self.runSnap = snapNow();
    self.lastBusy = busyNow();
    [self.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    return;
  }
  self.scroll = [self findScroll];
  if (!self.scroll) { [self finish:@"no scroll view with contentSize > 20000 found"]; return; }
  if ([self.scenario isEqualToString:@"rest"]) {
    // One 10 s segment, no scrolling, measured from the next display-link tick.
    [self.segments removeAllObjects];
    [self.segments addObject:@{@"v": @0, @"dir": @1, @"rest": @1}];
    self.runSnap = snapNow();
    self.lastBusy = busyNow();
    [self.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    return;
  }
  if (isInner(self.scenario)) {
    // Steps: every one is a segment (so analyze.py splits them); unmeasured ones are "warm".
    [self.segments removeAllObjects];
    for (NSString *kind in @[@"filmstrip", @"inbox"]) {
      [self.segments addObject:@{@"v": @0, @"dir": @1, @"warm": @1, @"find": kind, @"dur": @20}];
      if ([self.scenario isEqualToString:@"innerfling"]) {
        [self.segments addObject:@{@"v": @3000, @"dir": @1, @"warm": @1, @"inner": kind}];
        [self.segments addObject:@{@"v": @3000, @"dir": @-1, @"warm": @1, @"inner": kind}];
        for (NSNumber *v in @[@3000, @6000, @12000, @24000, @48000, @96000]) {
          [self.segments addObject:@{@"v": v, @"dir": @1, @"inner": kind}];
          [self.segments addObject:@{@"v": v, @"dir": @-1, @"inner": kind}];
        }
      } else {
        // near: 3,000 pt away and back; far: 24,000 pt (the row is surely recycled)
        NSArray *trips = @[@[@"near", @6000, @0.5, horizontalKind(kind) ? @12345 : @5432],
                           @[@"far", @12000, @2.0, horizontalKind(kind) ? @23456 : @8765]];
        for (NSArray *t in trips) {
          [self.segments addObject:@{@"v": @0, @"dir": @1, @"warm": @1, @"mark": kind, @"to": t[3], @"dur": @0.8}];
          [self.segments addObject:@{@"v": t[1], @"dir": @1, @"dur": t[2], @"trip": t[0], @"kind": kind}];
          [self.segments addObject:@{@"v": t[1], @"dir": @-1, @"dur": t[2], @"trip": t[0], @"kind": kind, @"returnTo": kind}];
          [self.segments addObject:@{@"v": @0, @"dir": @1, @"warm": @1, @"check": kind, @"trip": t[0], @"dur": @0.8}];
        }
      }
    }
  } else {
    CGPoint o = self.scroll.contentOffset;
    o.y = self.scroll.contentSize.height / 3;
    self.scroll.contentOffset = o;
  }
  // settle one second at the start offset before measuring
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), dispatch_get_main_queue(), ^{
    self.runSnap = snapNow();
    self.lastBusy = busyNow();
    [self.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
  });
}

static NSDictionary *delta(Snap a, Snap b, NSInteger frames) {
  double dur = b.t - a.t, busy = (b.busy - a.busy) * 1000, cpu = (b.cpu - a.cpu) * 1000, main = (b.main - a.main) * 1000;
  return @{@"sec": @(dur), @"frames": @(frames), @"busyMs": @(busy), @"cpuMs": @(cpu), @"mainCpuMs": @(main),
           @"busyMsPerSec": @(dur > 0 ? busy / dur : 0), @"busyMsPerFrame": @(frames ? busy / frames : 0),
           @"cpuMsPerSec": @(dur > 0 ? cpu / dur : 0), @"mainCpuMsPerSec": @(dur > 0 ? main / dur : 0)};
}

- (void)closeSegment {
  if (self.seg < 0 || self.seg >= (NSInteger)self.segments.count) return;
  NSMutableDictionary *d = [delta(self.segSnap, snapNow(), self.segFrames) mutableCopy];
  d[@"travelPt"] = @(self.segTravel);
  d[@"footprint"] = @(footprint(NULL));
  d[@"thermal"] = @(thermalNow()); d[@"lowPower"] = @(lowPowerNow());
  if (gVm) d[@"vm"] = vmBreakdown();
  [self.segStats addObject:d];
}

- (void)coldTick:(CADisplayLink *)l {
  CFTimeInterval now = CACurrentMediaTime();
  [self trackPeak];
  if (now - gProcStart > 30) { [self finish:@"coldstart: no blank-free list frame within 30 s of process start"]; return; }
  if (!self.scroll) self.scroll = [self findScroll];
  double b = self.scroll ? [self blankPoints] : -1;
  // At the top of the list, a leading band under 120 pt is the list's own top margin
  // (header whitespace), not missing content; it is recorded, not counted.
  double lead = 0;
  if (b > 0 && self.lastLeading > 0 && self.lastLeading < 120 &&
      self.scroll.contentOffset.y <= -self.scroll.adjustedContentInset.top + 1) { lead = self.lastLeading; b -= lead; }
  [self.timeline addObject:@[@((now - gProcStart) * 1000), @(self.scroll != nil), @(b), @(lead)]];
  if (self.timeline.count == 1) self.firstContentMs = (now - gProcStart) * 1000; // first display-link frame
  if (!self.scroll || b != 0) return;
  Snap s = snapNow();
  uint64_t lifetimePeak = 0; uint64_t foot = footprint(&lifetimePeak);
  NSUInteger foundAt = [self.timeline indexOfObjectPassingTest:^BOOL(NSArray *e, NSUInteger i, BOOL *stop) { return [e[1] boolValue]; }];
  self.cold = @{
    @"ms": @((now - gProcStart) * 1000),                 // observed blank-free, from process start
    @"targetMs": @((l.targetTimestamp - gProcStart) * 1000), // when this frame reaches the display
    @"source": gStartSource,
    @"procToCtorMs": @((gLoad - gProcStart) * 1000),
    @"firstTickMs": @(self.firstContentMs),
    @"scrollFoundMs": self.timeline[foundAt][0],
    @"frames": @(self.timeline.count),
    @"leadingGapPt": @(lead),
    // totals since process start: CPU from getrusage/thread_info are cumulative; busy from the constructor
    @"cpuMs": @(s.cpu * 1000), @"mainCpuMs": @(s.main * 1000), @"busyMsSinceCtor": @(s.busy * 1000),
    @"footprint": @(foot), @"lifetimePeak": @(lifetimePeak),
  };
  [self finish:nil];
}

- (void)tick:(CADisplayLink *)l {
  if (self.done) return;
  self.ticks++;
  if ([self.scenario isEqualToString:@"coldstart"]) { [self coldTick:l]; return; }
  CFTimeInterval now = l.timestamp;
  double exp = l.targetTimestamp - l.timestamp;
  double b0 = busyNow();
  if (self.last > 0) { [self.dts addObject:@(now - self.last)]; [self.expected addObject:@(exp)]; [self.busyMs addObject:@((b0 - self.lastBusy) * 1000)]; }
  self.lastBusy = b0;
  double dt = self.last > 0 ? now - self.last : 0;
  if (gHud) [gHud frame:dt expected:exp now:now];
  self.last = now;
  [self trackPeak];
  UIScrollView *s = self.scroll;
  CGFloat maxY = s.contentSize.height - s.bounds.size.height;
  if ([self.scenario isEqualToString:@"jump"]) {
    if (self.jumpIndex < 0 || (self.cleanRun >= 3) || now - self.jumpAt > 3.0) {
      if (self.jumpIndex >= 0) {
        BOOL ok = self.cleanRun >= 3;
        NSMutableDictionary *jr = [@{@"target": @(s.contentOffset.y), @"ms": ok ? @((self.lastCleanAt - self.jumpAt) * 1000) : [NSNull null]} mutableCopy];
        if ((!ok || envOn("BENCH_JUMPDUMP_ALL")) && gJumpDump) {
          // Validation for a timeout: the layer sampler's last image beside the render server's view of
          // the same rect (drawViewHierarchyInRect draws Metal and MapKit content), scored by the same rule.
          NSString *dir = [NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES).firstObject stringByAppendingPathComponent:gJumpDump];
          [NSFileManager.defaultManager createDirectoryAtPath:dir withIntermediateDirectories:YES attributes:nil error:nil];
          [UIImagePNGRepresentation(self.lastSampleImage) writeToFile:[dir stringByAppendingFormat:@"/jump-%ld-layer.png", (long)self.jumpIndex] atomically:YES];
          self.forceHierarchy = YES; double sb = [self blankPoints]; self.forceHierarchy = NO;
          [UIImagePNGRepresentation(self.lastSampleImage) writeToFile:[dir stringByAppendingFormat:@"/jump-%ld-screen.png", (long)self.jumpIndex] atomically:YES];
          jr[@"screenBlank"] = @(sb); jr[@"layerBlank"] = self.blankPts.lastObject ?: [NSNull null];
        }
        [self.jumps addObject:jr];
      }
      self.jumpIndex++;
      if (self.jumpIndex >= 10) { [self finish:nil]; return; }
      // fixed pseudo-random targets across the content
      double frac = fmod(0.137 + self.jumpIndex * 0.618, 1.0);
      CGPoint o = s.contentOffset; o.y = frac * maxY; s.contentOffset = o;
      self.jumpAt = CACurrentMediaTime(); self.cleanRun = 0; self.firstDumped = NO;
      return;
    }
    double b = [self blankPoints];
    if (gJumpDump && envOn("BENCH_JUMPDUMP_ALL") && self.cleanRun == 0 && !self.firstDumped) { // the first sample after a jump
      NSString *dir = [NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES).firstObject stringByAppendingPathComponent:gJumpDump];
      [NSFileManager.defaultManager createDirectoryAtPath:dir withIntermediateDirectories:YES attributes:nil error:nil];
      [UIImagePNGRepresentation(self.lastSampleImage) writeToFile:[dir stringByAppendingFormat:@"/jump-%ld-first-%.0f.png", (long)self.jumpIndex, b] atomically:YES];
      self.firstDumped = YES;
    }
    [self.blankPts addObject:@(b)];
    if (b == 0) { if (self.cleanRun == 0) self.lastCleanAt = CACurrentMediaTime(); self.cleanRun++; }
    else self.cleanRun = 0;
    return;
  }
  // fling / ladder: constant-speed segments of 2 s, position follows wall time
  BOOL rest = [self.scenario isEqualToString:@"rest"] || [self.scenario isEqualToString:@"still"];
  NSDictionary *cur = self.seg >= 0 && self.seg < (NSInteger)self.segments.count ? self.segments[self.seg] : nil;
  double dur = cur[@"dur"] ? [cur[@"dur"] doubleValue] : (rest ? 10.0 : 2.0);
  if (cur[@"find"] && now - self.segStart >= dur && !self.stepDone) {
    [self finish:[NSString stringWithFormat:@"%@: no %@ inner list found within %.0f s", self.scenario, cur[@"find"], dur]]; return;
  }
  if (self.seg < 0 || now - self.segStart >= dur || (cur[@"find"] && self.stepDone)) {
    if (cur) [self exitStep:cur];
    [self closeSegment];
    if (self.seg >= 0) [self emit:nil final:NO];
    self.seg++;
    if (self.seg >= (NSInteger)self.segments.count) { [self finish:nil]; return; }
    self.segStart = now;
    self.segSnap = snapNow(); self.segFrames = 0; self.segTravel = 0;
    [self.dts addObject:@(-1)]; [self.expected addObject:@(-1)]; [self.busyMs addObject:@(-1)]; // segment marker
    [self enterStep:self.segments[self.seg]];
    if ([self.scenario isEqualToString:@"still"] && !self.segments[self.seg][@"warm"]) {
      settleInk();
      self.footStart = self.footPeak = footprint(NULL); // memory over the measured segment only
    }
  }
  NSDictionary *seg = self.segments[self.seg];
  if (seg[@"find"] || (isInner(self.scenario) && [seg[@"v"] doubleValue] == 0)) {
    if (seg[@"find"] && [self findStep:seg[@"find"] now:now]) self.stepDone = YES;
    self.segFrames++; self.frame++;
    if (self.sample > 0) [self.blankPts addObject:@(-1)];
    return;
  }
  if (seg[@"inner"]) {
    UIScrollView *v = self.driven;
    BOOL x = horizontalKind(seg[@"inner"]);
    CGFloat before = innerOffset(v, x);
    setInnerOffset(v, x, before + [seg[@"dir"] doubleValue] * [seg[@"v"] doubleValue] * dt);
    self.segTravel += fabs(innerOffset(v, x) - before); self.segFrames++; self.frame++;
    if (self.sample > 0 && self.frame % self.sample == 0) [self.blankPts addObject:@(v ? [self innerBlank:v horizontal:x] : -1)];
    else if (self.sample > 0) [self.blankPts addObject:@(-1)];
    return;
  }
  if (rest) {
    self.segFrames++; self.frame++;
    if (self.sample > 0 && self.frame % self.sample == 0) [self.blankPts addObject:@([self blankPoints])];
    else if (self.sample > 0) [self.blankPts addObject:@(-1)];
    return;
  }
  CGPoint o = s.contentOffset;
  CGFloat y0 = o.y;
  o.y = MAX(0, MIN(maxY, o.y + [seg[@"dir"] doubleValue] * [seg[@"v"] doubleValue] * dt));
  CFTimeInterval set0 = CACurrentMediaTime();
  s.contentOffset = o;
  [self.setCost addObject:@((CACurrentMediaTime() - set0) * 1000)];
  self.segTravel += fabs(o.y - y0); self.segFrames++;
  self.frame++;
  if (self.sample > 0 && self.frame % self.sample == 0) [self.blankPts addObject:@([self blankPoints])];
  else if (self.sample > 0) [self.blankPts addObject:@(-1)];
}

- (void)finish:(NSString *)error {
  if (self.done) return;
  self.done = YES;
  [self.link invalidate];
  // BENCH_REST=N: also record the footprint N seconds after scrolling stops ("rest"); "end" stays the
  // footprint at the last frame.
  double rest = [NSProcessInfo.processInfo.environment[@"BENCH_REST"] doubleValue];
  if (rest > 0 && !error) {
    self.footEnd = footprint(NULL); if (gVm) self.vmEndNow = vmBreakdown();
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(rest * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{ [self emit:nil final:YES]; });
    return;
  }
  [self emit:error final:YES];
}

// A checkpoint at each segment boundary: an app the watchdog kills mid-run
// still leaves how far it got (`<BENCH_OUT>.partial`, "partial": true).
- (void)emit:(NSString *)error final:(BOOL)final {
  NSMutableDictionary *out = [NSMutableDictionary dictionary];
  out[@"scenario"] = self.scenario; out[@"sample"] = @(self.sample);
  out[@"bundle"] = NSBundle.mainBundle.bundleIdentifier ?: @"";
  // What was measured (2026-09-29): the build stamp resign.sh put in Info.plist (BENCH_BUILD), and the main
  // executable's LC_UUID (dwarfdump --uuid on the local bundle gives the same), so a stale install shows.
  out[@"build"] = [NSBundle.mainBundle objectForInfoDictionaryKey:@"BenchBuild"] ?: @"";
  out[@"exeUUID"] = exeUUID();
  out[@"maxFps"] = @(UIScreen.mainScreen.maximumFramesPerSecond);
  out[@"live"] = @(self.live);
  if (error) out[@"error"] = error;
  out[@"scrollClass"] = NSStringFromClass(self.scroll.class) ?: @"";
  out[@"contentHeight"] = @(self.scroll.contentSize.height);
  // copies, so a checkpoint serialized off the main thread sees this moment's arrays
  out[@"dts"] = [self.dts copy]; out[@"expected"] = [self.expected copy]; out[@"busyFrameMs"] = [self.busyMs copy];
  out[@"blank"] = [self.blankPts copy]; out[@"setCost"] = [self.setCost copy]; out[@"jumps"] = [self.jumps copy];
  out[@"segments"] = [self.segments copy]; out[@"segStats"] = [self.segStats copy];
  out[@"run"] = delta(self.runSnap, snapNow(), self.ticks); // every display-link tick since measuring began
  uint64_t lifetimePeak = 0; uint64_t end = footprint(&lifetimePeak);
  uint64_t restFoot = 0;
  if (self.footEnd) { restFoot = end; end = self.footEnd; }
  if (end > self.footPeak) self.footPeak = end;
  out[@"mem"] = @{@"load": @(gFootLoad), @"start": @(self.footStart), @"peak": @(self.footPeak), @"end": @(end),
                  @"lifetimePeak": @(lifetimePeak), @"rest": @(restFoot)};
  if (gVm) { out[@"vmEnd"] = self.vmEndNow ?: vmBreakdown(); if (self.vmEndNow) out[@"vmRest"] = vmBreakdown(); if (self.vmPeak) out[@"vmPeak"] = self.vmPeak; }
  if (gMlog && final) { malloc_logger = NULL; out[@"mallocLog"] = mlogDump(); }
  out[@"startSource"] = gStartSource;
  out[@"procToCtorMs"] = @((gLoad - gProcStart) * 1000);
  out[@"thermalStart"] = @(self.thermalStart);
  out[@"lowPowerStart"] = self.lowPowerStart < 0 ? (id)NSNull.null : (id)@((BOOL)(self.lowPowerStart != 0));
  out[@"thermalEnd"] = @(thermalNow()); out[@"lowPowerEnd"] = @(lowPowerNow());
  @synchronized(gThermalChanges) { out[@"thermalChanges"] = [gThermalChanges copy] ?: @[]; }
  if (self.cold) out[@"coldstart"] = self.cold;
  if (gInkMs > 0) out[@"firstInkMs"] = @(gInkMs);
  if (gInkFrames > 0) out[@"inkFrames"] = @(gInkFrames);
  if (isInner(self.scenario)) { out[@"keep"] = [self.keep copy]; out[@"innerLists"] = [self.innerInfo copy]; }
  if (self.timeline.count) out[@"timeline"] = [self.timeline copy];
  out[@"firstContentMs"] = @(self.firstContentMs);
  out[@"sampleMs"] = @(self.sampleCount ? 1000 * self.sampleCost / self.sampleCount : 0);
  out[@"render"] = self.useLayer ? @"layer" : @"hierarchy";
  NSString *path = NSProcessInfo.processInfo.environment[@"BENCH_OUT"];
  if (path && ![path hasPrefix:@"/"]) {
    NSString *docs = NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES).firstObject;
    path = [docs stringByAppendingPathComponent:path];
  }
  if (!final) { out[@"partial"] = @YES; out[@"segmentsDone"] = @(self.seg); path = [path stringByAppendingString:@".partial"]; }
  if (!final) {
    // serializing and writing ~250 KB took 6–12 ms; on the main thread it made the app
    // under test miss a frame at every segment boundary
    static dispatch_queue_t q; static dispatch_once_t once;
    dispatch_once(&once, ^{ q = dispatch_queue_create("heavybench.checkpoint", DISPATCH_QUEUE_SERIAL); });
    dispatch_async(q, ^{
      NSData *d = [NSJSONSerialization dataWithJSONObject:out options:0 error:nil];
      if (path) [d writeToFile:path atomically:YES];
    });
    return;
  }
  NSData *d = [NSJSONSerialization dataWithJSONObject:out options:0 error:nil];
  if (path) [d writeToFile:path atomically:YES];
  NSLog(@"[heavybench] done %@ frames=%lu error=%@", self.scenario, (unsigned long)(self.dts.count ?: self.timeline.count), error);
}
@end



// `still` (the features benchmark, revised 2026-09-29 for fairness): from launch, sample the key window every
// display-link frame (layer render at 1/8 scale, Metal content marked) until the ink reaches the feature's
// threshold, then stop: `firstInkMs` is that frame's time from process start. The threshold is 90 % of the
// feature scene's own ink: the scene's area (from the SPEC, computed from the live window: F1/F2 the
// window, F3 min(W-32,800) x 0.75 of that, F4 (W-32) x 400) times its in-scene ink density (the lower of the
// two apps' frozen screenshots: F1 0.948, F2 0.971, F3 1.0, F4 0.435), over the window's area. Sampling
// stops at the first hit or 5 s after the probe loads (firstInkMs null), always before the measured
// segment (BENCH_DELAY 5 s + 3 s warm-up), and the measured segment's memory is taken from its own start.
static double inkThreshold(CGSize win) {
  NSString *f = NSProcessInfo.processInfo.environment[@"BENCH_FEATURE"] ?: @"";
  double W = win.width, H = win.height, area = W * H, density = 0.9;
  if ([f isEqualToString:@"svg"]) density = 0.948;
  else if ([f isEqualToString:@"canvas"]) density = 0.971;
  else if ([f isEqualToString:@"filter"]) { double w = MIN(W - 32, 800); area = w * 0.75 * w; density = 1.0; }
  else if ([f isEqualToString:@"chart"]) { area = (W - 32) * 400; density = 0.435; }
  return 0.9 * density * MIN(1.0, area / (W * H));
}
@interface LBInk : NSObject
@property (nonatomic, strong) CADisplayLink *link;
@end
@implementation LBInk
- (void)tick:(CADisplayLink *)l {
  CFTimeInterval now = CACurrentMediaTime();
  gInkFrames++;
  if (now - gLoad > 5) { [self.link invalidate]; gInkLink = nil; return; } // 5 s after the probe loads
  UIWindow *w = keyWindow();
  if (!w) return;
  const CGFloat scale = 0.125;
  size_t W = (size_t)(w.bounds.size.width * scale), H = (size_t)(w.bounds.size.height * scale);
  if (W < 8 || H < 8) return;
  uint8_t *px = calloc(W * H * 4, 1);
  CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
  CGContextRef ctx = CGBitmapContextCreate(px, W, H, 8, W * 4, cs, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
  CGColorSpaceRelease(cs);
  CGContextSetGrayFillColor(ctx, 1, 1); CGContextFillRect(ctx, CGRectMake(0, 0, W, H));
  CGContextTranslateCTM(ctx, 0, H); CGContextScaleCTM(ctx, scale, -scale);
  UIGraphicsPushContext(ctx);
  [w.layer.presentationLayer ?: w.layer renderInContext:ctx];
  markMetal(ctx, w);
  UIGraphicsPopContext();
  size_t ink = 0;
  for (size_t i = 0; i < W * H; i++) { uint8_t *p = px + i * 4; if (p[0] < 245 || p[1] < 245 || p[2] < 245) ink++; }
  CGContextRelease(ctx); free(px);
  if (getenv("BENCH_INKDBG") && gInkFrames % 60 == 1) {
    for (UIScene *sc in UIApplication.sharedApplication.connectedScenes) if ([sc isKindOfClass:UIWindowScene.class])
      for (UIWindow *x in ((UIWindowScene *)sc).windows) NSLog(@"[probe-ink] window %@ key=%d hidden=%d level=%.0f subviews=%lu sublayers=%lu", x.class, x.isKeyWindow, x.hidden, x.windowLevel, (unsigned long)x.subviews.count, (unsigned long)x.layer.sublayers.count);
  }
  if (getenv("BENCH_INKDBG") && gInkFrames % 60 == 1) {
    UIGraphicsImageRenderer *r = [[UIGraphicsImageRenderer alloc] initWithSize:w.bounds.size];
    UIImage *img = [r imageWithActions:^(UIGraphicsImageRendererContext *c) { [w drawViewHierarchyInRect:w.bounds afterScreenUpdates:NO]; }];
    CGImageRef ci = img.CGImage; size_t iw = CGImageGetWidth(ci)/8, ih = CGImageGetHeight(ci)/8;
    uint8_t *q = calloc(iw*ih*4,1); CGColorSpaceRef cs2 = CGColorSpaceCreateDeviceRGB();
    CGContextRef c2 = CGBitmapContextCreate(q, iw, ih, 8, iw*4, cs2, (CGBitmapInfo)kCGImageAlphaPremultipliedLast); CGColorSpaceRelease(cs2);
    CGContextDrawImage(c2, CGRectMake(0,0,iw,ih), ci); size_t k2=0; for (size_t i=0;i<iw*ih;i++){uint8_t *p=q+i*4; if (p[0]<245||p[1]<245||p[2]<245) k2++;}
    CGContextRelease(c2); free(q);
    NSLog(@"[probe-ink] hierarchy ink %.3f; layer %@ sublayer %@ %@", (double)k2/(iw*ih), w.layer.class, w.layer.sublayers.firstObject.class, w.layer.sublayers.firstObject.sublayers.firstObject.class);
  }
  if (getenv("BENCH_INKDBG") && gInkFrames % 60 == 1) NSLog(@"[probe-ink] %.0f ms ink %.3f (%zux%zu)", (now - gProcStart) * 1000, (double)ink / (W * H), W, H);
  if (!gInkSamples) gInkSamples = [NSMutableArray array];
  double frac = (double)ink / (W * H);
  [gInkSamples addObject:@[@((now - gProcStart) * 1000), @(frac)]];
  if (frac >= inkThreshold(w.bounds.size)) { gInkMs = (now - gProcStart) * 1000; [self.link invalidate]; gInkLink = nil; }
}
// At the measured segment's start: the sampler is already stopped (first hit or 5 s); stop it regardless.
static void settleInk(void) { [gInkLink invalidate]; gInkLink = nil; }
@end
static LBInk *gInk;

static LBProbe *gProbe;

__attribute__((constructor)) static void heavybench_init(void) {
  installMetalHook();
  gJumpDump = NSProcessInfo.processInfo.environment[@"BENCH_JUMPDUMP"]; if (!gJumpDump.length) gJumpDump = nil;
  gLoad = CACurrentMediaTime();
  gFootLoad = footprint(NULL);
  gVm = [NSProcessInfo.processInfo.environment[@"BENCH_VM"] isEqualToString:@"1"];
  if ([NSProcessInfo.processInfo.environment[@"BENCH_MALLOCLOG"] isEqualToString:@"1"]) { gMlog = calloc(MLOG_N, sizeof(MRec)); malloc_logger = mlogHook; }
  gMainThread = pthread_mach_thread_np(pthread_self()); // constructors run on the main thread (pthread_main_np);
  // process start: kinfo_proc.p_starttime is wall-clock; map it onto the media clock
  gProcStart = gLoad;
  struct kinfo_proc kp; size_t len = sizeof kp;
  int mib[4] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()};
  if (sysctl(mib, 4, &kp, &len, NULL, 0) == 0 && len >= sizeof kp && kp.kp_proc.p_starttime.tv_sec > 0) {
    struct timeval nowWall; gettimeofday(&nowWall, NULL);
    double media = CACurrentMediaTime();
    double ago = (nowWall.tv_sec - kp.kp_proc.p_starttime.tv_sec) + (nowWall.tv_usec - kp.kp_proc.p_starttime.tv_usec) / 1e6;
    if (ago >= 0 && ago < 60) { gProcStart = media - ago; gStartSource = @"kern_proc"; }
  }
  gThermalChanges = [NSMutableArray array];
  [NSNotificationCenter.defaultCenter addObserverForName:NSProcessInfoThermalStateDidChangeNotification object:nil queue:nil
    usingBlock:^(NSNotification *n) { @synchronized(gThermalChanges) { [gThermalChanges addObject:@[@((CACurrentMediaTime() - gProcStart) * 1000), @(thermalNow())]]; } }];
  // the main thread is busy from launch until the run loop first sleeps
  gAwake = YES; gAwakeAt = gLoad;
  // wake is stamped by the first AfterWaiting observer, sleep by the last BeforeWaiting one,
  // so the Core Animation commit (a BeforeWaiting observer at order 2000000) counts as busy
  CFRunLoopObserverRef wake = CFRunLoopObserverCreate(NULL, kCFRunLoopAfterWaiting, true, LONG_MIN, busyObserver, NULL);
  CFRunLoopObserverRef sleep = CFRunLoopObserverCreate(NULL, kCFRunLoopBeforeWaiting, true, LONG_MAX, busyObserver, NULL);
  CFRunLoopAddObserver(CFRunLoopGetMain(), wake, kCFRunLoopCommonModes);
  CFRunLoopAddObserver(CFRunLoopGetMain(), sleep, kCFRunLoopCommonModes);
  NSDictionary *env = NSProcessInfo.processInfo.environment;
  if ([env[@"BENCH_SCENARIO"] isEqualToString:@"still"]) {
    dispatch_async(dispatch_get_main_queue(), ^{
      gInk = [LBInk new];
      gInk.link = [CADisplayLink displayLinkWithTarget:gInk selector:@selector(tick:)];
      gInkLink = gInk.link;
      [gInk.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    });
  }
  if ([env[@"BENCH_SCENARIO"] isEqualToString:@"coldstart"]) {
    dispatch_async(dispatch_get_main_queue(), ^{
      UIApplication.sharedApplication.idleTimerDisabled = YES;
      gProbe = [LBProbe new]; gProbe.thermalStart = gProbe.lowPowerStart = -1;
      [gProbe startColdstart];
    });
    return;
  }
  double delay = [env[@"BENCH_DELAY"] doubleValue] ?: 5;
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
    UIApplication.sharedApplication.idleTimerDisabled = YES;
    gProbe = [LBProbe new]; gProbe.thermalStart = gProbe.lowPowerStart = -1;
    [gProbe start];
  });
}
