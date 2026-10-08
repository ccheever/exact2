// xheavy macOS probe — the heavy-list iOS probe (../../../heavy-list/probe/probe.m) ported to AppKit.
// Injected into each app with DYLD_INSERT_LIBRARIES (both apps are ad-hoc signed without the hardened
// runtime, so dyld honours it). Finds the NSScrollView with the tallest document (> 20,000 pt) in the
// app's window, drives its clip view from a display link on the main thread (NSView
// -displayLinkWithTarget:selector:, macOS 14), and writes one JSON result to $BENCH_OUT (absolute path).
// Scenario names, segment layout and the JSON keys are the iOS probe's, so summarize.py and agg.py read
// both. Differences from iOS are marked "macOS:".
//
// BENCH_SCENARIO: fling | ladder | jump | coldstart | rest | innerfling | innerkeep  (see the iOS probe)
// BENCH_SAMPLE:   N = measure blank bands every Nth frame (perturbs timing)
// BENCH_RENDER:   layer = -[CALayer renderInContext:] of the window's content layer (the iOS "layer" sampler;
//                 default for coldstart), else -[NSView cacheDisplayInRect:toBitmapImageRep:] (AppKit's
//                 drawViewHierarchy)
// BENCH_DELAY:    seconds after launch before starting (default 5; coldstart ignores it)
// BENCH_EXIT=1:   exit(0) after the final JSON is written (the runner also kills the recorded PID)
// BENCH_DUMP:     directory (absolute) for PNGs of sampled frames that had a blank band (at most 6)
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/sysctl.h>
#include <sys/time.h>
#include <mach/mach.h>
#include <pthread.h>
#include <dlfcn.h>

static CFTimeInterval gLoad, gProcStart;
static NSString *gStartSource = @"constructor";
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
  struct rusage ru; getrusage(RUSAGE_SELF, &ru);
  return ru.ru_utime.tv_sec + ru.ru_stime.tv_sec + (ru.ru_utime.tv_usec + ru.ru_stime.tv_usec) / 1e6;
}
static double mainCpu(void) {
  thread_basic_info_data_t bi; mach_msg_type_number_t n = THREAD_BASIC_INFO_COUNT;
  if (thread_info(gMainThread, THREAD_BASIC_INFO, (thread_info_t)&bi, &n) != KERN_SUCCESS) return -1;
  return bi.user_time.seconds + bi.system_time.seconds + (bi.user_time.microseconds + bi.system_time.microseconds) / 1e6;
}
static uint64_t footprint(uint64_t *lifetimePeak) {
  task_vm_info_data_t vi; mach_msg_type_number_t n = TASK_VM_INFO_COUNT;
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
  NSMutableDictionary *zoneUse = [NSMutableDictionary dictionary];
  if (malloc_get_all_zones(mach_task_self(), NULL, &zones, &count) == KERN_SUCCESS)
    for (unsigned i = 0; i < count; i++) { malloc_statistics_t st; malloc_zone_statistics((malloc_zone_t *)zones[i], &st); inUse += st.size_in_use; alloc += st.size_allocated;
      const char *zn = malloc_get_zone_name((malloc_zone_t *)zones[i]); NSString *k = [NSString stringWithFormat:@"%s#%u", zn ?: "?", i];
      zoneUse[k] = @(st.size_in_use); }
  return @{@"footprint": @(vi.phys_footprint), @"internal": @(vi.internal), @"compressed": @(vi.compressed),
           @"graphics": @(vi.ledger_tag_graphics_footprint), @"media": @(vi.ledger_tag_media_footprint),
           @"purgeableNonvolatile": @(vi.ledger_purgeable_nonvolatile), @"mallocInUse": @(inUse), @"mallocAllocated": @(alloc), @"tags": tags, @"zones": zoneUse};
}



typedef struct { double t, busy, cpu, main; } Snap;
static Snap snapNow(void) { return (Snap){CACurrentMediaTime(), busyNow(), procCpu(), mainCpu()}; }

// ---- scroll geometry (macOS: NSScrollView + its clip view) ----------------------------
// Offsets are measured from the document's top, whether or not the document view is flipped;
// sizes are the document view's frame (contentSize) and the clip view's bounds (visible).
static CGSize contentSizeOf(NSScrollView *s) { return s.documentView ? s.documentView.frame.size : CGSizeZero; }
static NSEdgeInsets insetsOf(NSScrollView *s) { return s.contentView.contentInsets; }
static CGFloat offY(NSScrollView *s) {
  NSClipView *c = s.contentView; NSView *d = s.documentView;
  if (!d || d.isFlipped) return c.bounds.origin.y;
  return d.frame.size.height - c.bounds.origin.y - c.bounds.size.height;
}
static CGFloat offX(NSScrollView *s) { return s.contentView.bounds.origin.x; }
static void setOff(NSScrollView *s, CGFloat x, CGFloat y) {
  NSClipView *c = s.contentView; NSView *d = s.documentView;
  NSPoint p = NSMakePoint(x, (!d || d.isFlipped) ? y : d.frame.size.height - y - c.bounds.size.height);
  [c scrollToPoint:p];
  [s reflectScrolledClipView:c];
}
static void setOffY(NSScrollView *s, CGFloat y) { setOff(s, offX(s), y); }

static NSWindow *appWindow(void) {
  NSWindow *best = nil; CGFloat area = 0;
  for (NSWindow *w in NSApp.windows) {
    if (!w.isVisible || [w isKindOfClass:NSPanel.class] || !w.contentView) continue;
    CGFloat a = w.frame.size.width * w.frame.size.height;
    if (a > area) { area = a; best = w; }
  }
  return best;
}

// ---- Metal content under the layer sampler (as the iOS probe) ------------------------------
// renderInContext does not draw a CAMetalLayer's drawable; every CAMetalLayer that vended one is
// remembered, and after a layer render each one visible in the window is painted as a 2x2 checker.
static NSHashTable<CALayer *> *gDrewMetal;
static IMP gOrigNextDrawable;
static id probe_nextDrawable(id self, SEL _cmd) {
  id d = ((id (*)(id, SEL))gOrigNextDrawable)(self, _cmd);
  if (d) { @synchronized(gDrewMetal) { [gDrewMetal addObject:self]; } }
  return d;
}
static void installMetalHook(void) {
  gDrewMetal = [NSHashTable weakObjectsHashTable];
  Class c = NSClassFromString(@"CAMetalLayer");
  Method m = c ? class_getInstanceMethod(c, @selector(nextDrawable)) : NULL;
  if (m) gOrigNextDrawable = method_setImplementation(m, (IMP)probe_nextDrawable);
}
static void markMetal(CGContextRef ctx, CALayer *root) {
  NSArray<CALayer *> *layers;
  @synchronized(gDrewMetal) { layers = gDrewMetal.allObjects; }
  static int dbg = 0;
  BOOL log = getenv("BENCH_METALDBG") && dbg++ < 40;
  if (log && !layers.count) NSLog(@"[probe-metal] no drawn Metal layers");
  for (CALayer *l in layers) {
    BOOL shown = YES, inRoot = NO;
    for (CALayer *a = l; a; a = a.superlayer) {
      CALayer *pa = a.presentationLayer ?: a;
      if (a.hidden || pa.opacity <= 0.01) { shown = NO; break; }
      if (a == root) { inRoot = YES; break; }
    }
    if (log) NSLog(@"[probe-metal] %@ shown=%d inRoot=%d frame=%@", l.class, shown, inRoot, NSStringFromRect([l convertRect:l.bounds toLayer:root]));
    if (!shown || !inRoot) continue;
    CGRect f = [l convertRect:l.bounds toLayer:root];
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

// BENCH_MAPCOUNT: every MKMapView made (weakly held), so a segment can say how many are alive.
static NSHashTable *gMaps; static IMP gOrigMapInit;
// Map work since launch, the same count for both apps: MKMapViews made, and places given to a map (each of
// MapKit's own ways to move one: region, camera, visible rect, centre). A place given is a load of tiles.
static long gMapInits, gMapRegion, gMapCamera, gMapRect, gMapCenter;
static IMP gOrigRegion, gOrigCamera, gOrigRect, gOrigCenter;
static id probe_mapInit(id self, SEL _cmd, NSRect f) {
  id v = ((id (*)(id, SEL, NSRect))gOrigMapInit)(self, _cmd, f);
  if (v) { @synchronized(gMaps) { [gMaps addObject:v]; gMapInits++; } }
  return v;
}
// The arguments are structs passed as the originals take them; each hook only counts and forwards.
typedef struct { double lat, lon, dlat, dlon; } PRegion;
typedef struct { double x, y, w, h; } PRect;
typedef struct { double lat, lon; } PCoord;
static void probe_mapRegion(id self, SEL _cmd, PRegion r, BOOL a) { gMapRegion++; ((void (*)(id, SEL, PRegion, BOOL))gOrigRegion)(self, _cmd, r, a); }
static void probe_mapCamera(id self, SEL _cmd, id c, BOOL a) { gMapCamera++; ((void (*)(id, SEL, id, BOOL))gOrigCamera)(self, _cmd, c, a); }
static void probe_mapRect(id self, SEL _cmd, PRect r, BOOL a) { gMapRect++; ((void (*)(id, SEL, PRect, BOOL))gOrigRect)(self, _cmd, r, a); }
static void probe_mapCenter(id self, SEL _cmd, PCoord c, BOOL a) { gMapCenter++; ((void (*)(id, SEL, PCoord, BOOL))gOrigCenter)(self, _cmd, c, a); }
static IMP hookMap(Class c, SEL s, IMP with) { Method m = c ? class_getInstanceMethod(c, s) : NULL; return m ? method_setImplementation(m, with) : NULL; }
static void installMapHook(void) {
  gMaps = [NSHashTable weakObjectsHashTable];
  Class c = NSClassFromString(@"MKMapView");
  Method m = c ? class_getInstanceMethod(c, @selector(initWithFrame:)) : NULL;
  if (m) gOrigMapInit = method_setImplementation(m, (IMP)probe_mapInit);
  gOrigRegion = hookMap(c, NSSelectorFromString(@"setRegion:animated:"), (IMP)probe_mapRegion);
  gOrigCamera = hookMap(c, NSSelectorFromString(@"setCamera:animated:"), (IMP)probe_mapCamera);
  gOrigRect = hookMap(c, NSSelectorFromString(@"setVisibleMapRect:animated:"), (IMP)probe_mapRect);
  gOrigCenter = hookMap(c, NSSelectorFromString(@"setCenterCoordinate:animated:"), (IMP)probe_mapCenter);
}
static NSInteger aliveMaps(void) { @synchronized(gMaps) { return gMaps.allObjects.count; } }

static void collect(NSView *v, NSMutableArray *out) {
  if ([v isKindOfClass:NSScrollView.class]) [out addObject:v];
  for (NSView *c in v.subviews) collect(c, out);
}

@interface LBProbe : NSObject
@property (nonatomic, weak) NSScrollView *scroll;
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
@property (nonatomic) BOOL useLayer, useWindow, live, done;
@property (nonatomic) Snap runSnap, segSnap;
@property (nonatomic) uint64_t footStart, footPeak, footEnd, vmPeakAt;
@property (nonatomic, strong) NSDictionary *vmPeak;
@property (nonatomic, strong) NSDictionary *cold;
@property (nonatomic) double maxFps;
@property (nonatomic, weak) NSScrollView *driven;
@property (nonatomic) double placedAt;
@property (nonatomic) BOOL stepDone;
@property (nonatomic, strong) NSMutableArray *keep, *innerInfo;
@property (nonatomic, strong) NSMutableDictionary *marks;
@property (nonatomic, strong) NSMapTable<NSString *, NSScrollView *> *markViews;
@end

@implementation LBProbe

- (NSScrollView *)findScroll {
  NSWindow *w = appWindow();
  if (!w) return nil;
  NSMutableArray *all = [NSMutableArray array];
  collect(w.contentView, all);
  NSScrollView *best = nil;
  for (NSScrollView *s in all) {
    if (s.frame.size.height < 300) continue;
    if (!best || contentSizeOf(s).height > contentSizeOf(best).height) best = s;
  }
  return contentSizeOf(best).height > 20000 ? best : nil;
}

/// Renders the window's content into an RGBA bitmap covering `r` (content-view coordinates, top-left
/// origin after conversion) at `scale`. Row 0 of the bitmap is the rect's top.
- (uint8_t *)renderRect:(CGRect)r scale:(CGFloat)scale w:(size_t *)outW h:(size_t *)outH ctx:(CGContextRef *)outCtx {
  NSWindow *win = self.scroll.window;
  NSView *cv = win.contentView;
  size_t W = (size_t)(r.size.width * scale), H = (size_t)(r.size.height * scale);
  if (W < 8 || H < 8) return NULL;
  uint8_t *px = calloc(W * H * 4, 1);
  CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
  CGContextRef ctx = CGBitmapContextCreate(px, W, H, 8, W * 4, cs, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
  CGColorSpaceRelease(cs);
  CFTimeInterval t0 = CACurrentMediaTime();
  if (self.useWindow) {
    // The window server's composited pixels of this app's own window (no screen-recording permission is
    // needed for a process's own windows). CGWindowListCreateImage is looked up at run time: the macOS 15+
    // SDK marks it unavailable, and the function is still exported.
    typedef CGImageRef (*CreateFn)(CGRect, uint32_t, uint32_t, uint32_t);
    static CreateFn create; static dispatch_once_t once;
    dispatch_once(&once, ^{ create = (CreateFn)dlsym(RTLD_DEFAULT, "CGWindowListCreateImage"); });
    NSRect sr = [win convertRectToScreen:[cv convertRect:(cv.isFlipped ? r : NSMakeRect(r.origin.x, cv.bounds.size.height - r.origin.y - r.size.height, r.size.width, r.size.height)) toView:nil]];
    CGFloat mainH = NSScreen.screens.firstObject.frame.size.height;   // CG global space is y-down from the main screen's top
    CGRect gr = CGRectMake(sr.origin.x, mainH - NSMaxY(sr), sr.size.width, sr.size.height);
    CGImageRef img = create ? create(gr, 8 /* kCGWindowListOptionIncludingWindow */, (uint32_t)win.windowNumber, 1 /* BoundsIgnoreFraming */) : NULL;
    // Area-averaged, as the iOS probe's layer render at quarter scale draws a hairline (faint, not gone):
    // the default interpolation picks pixels, so a 1 px border between sampled columns vanished and a
    // pale box bounded by it read as a blank band (found 2026-09-29: exact2's web view rows).
    if (img) { CGContextSetInterpolationQuality(ctx, kCGInterpolationHigh); CGContextDrawImage(ctx, CGRectMake(0, 0, W, H), img); CGImageRelease(img); }
    else { static int warned; if (!warned++) NSLog(@"[probe] window capture unavailable (fn %p)", create); }
  } else if (self.useLayer) {
    // CG's bitmap is y-up with memory row 0 at the highest y; a geometry-flipped root is drawn flipped.
    CALayer *root = cv.layer;
    // `r` is top-down; the context wants the rect y-up in the root layer's space.
    CGRect lr = CGRectMake(r.origin.x, root.bounds.size.height - r.origin.y - r.size.height, r.size.width, r.size.height);
    CGContextScaleCTM(ctx, scale, scale);
    CGContextTranslateCTM(ctx, -lr.origin.x, -lr.origin.y);
    if (root.geometryFlipped) { CGContextTranslateCTM(ctx, 0, root.bounds.size.height); CGContextScaleCTM(ctx, 1, -1); }
    [(root.presentationLayer ?: root) renderInContext:ctx];
    markMetal(ctx, root);
  } else {
    NSRect vr = cv.isFlipped ? r : NSMakeRect(r.origin.x, cv.bounds.size.height - r.origin.y - r.size.height, r.size.width, r.size.height);
    NSBitmapImageRep *rep = [cv bitmapImageRepForCachingDisplayInRect:vr];
    [cv cacheDisplayInRect:vr toBitmapImageRep:rep];
    CGContextDrawImage(ctx, CGRectMake(0, 0, W, H), rep.CGImage);
  }
  self.sampleCost += CACurrentMediaTime() - t0; self.sampleCount++;
  *outW = W; *outH = H; *outCtx = ctx;
  return px;
}

/// The feed's visible rect in content-view coordinates, top-left origin (y down).
- (CGRect)visibleTopDown:(NSScrollView *)s {
  NSView *cv = s.window.contentView;
  NSEdgeInsets in = insetsOf(s);
  NSRect b = s.contentView.bounds;
  NSRect r = [s.contentView convertRect:b toView:cv];  // cv coordinates (y-up unless flipped)
  r = NSIntersectionRect(r, cv.bounds);
  if (!cv.isFlipped) r.origin.y = cv.bounds.size.height - r.origin.y - r.size.height;
  r.origin.y += in.top; r.size.height -= in.top + in.bottom;
  return r;
}

- (void)dump:(CGContextRef)ctx blank:(double)blank {
  NSString *dir = NSProcessInfo.processInfo.environment[@"BENCH_DUMP"];
  if (!dir.length || blank <= 0 || self.dumped >= 6) return;
  [NSFileManager.defaultManager createDirectoryAtPath:dir withIntermediateDirectories:YES attributes:nil error:nil];
  CGImageRef img = CGBitmapContextCreateImage(ctx);
  NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithCGImage:img];
  [[rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:[dir stringByAppendingFormat:@"/blank-%ld-%.0f.png", (long)self.dumped, blank] atomically:YES];
  CGImageRelease(img); self.dumped++;
}

/// Sum of ink-free horizontal bands >= 60 pt inside the feed's visible rect (quarter scale); a band is a
/// run of scanlines whose luminance range across the inner width is < 10 (the iOS probe's rule).
- (double)blankPoints {
  NSScrollView *s = self.scroll;
  if (!s.window) return -1;
  CGRect r = [self visibleTopDown:s];
  const CGFloat scale = getenv("BENCH_BLANKSCALE") ? atof(getenv("BENCH_BLANKSCALE")) : 0.25, margin = 28;
  size_t W, H; CGContextRef ctx;
  uint8_t *px = [self renderRect:r scale:scale w:&W h:&H ctx:&ctx];
  if (!px) return -1;
  size_t x0 = (size_t)(margin * scale), x1 = W - x0;
  double blank = 0; size_t run = 0;
  const size_t minRun = (size_t)(60 * scale);
  self.lastLeading = 0;
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
  [self dump:ctx blank:blank];
  static int fullDumps = 0;
  if (blank > 0 && getenv("BENCH_DUMPFULL") && fullDumps < 6) {
    // The window server's picture right after, at full scale.
    size_t W2, H2; CGContextRef c2 = NULL; uint8_t *p2 = [self renderRect:r scale:1 w:&W2 h:&H2 ctx:&c2];
    if (p2 && c2) {
      CGImageRef img = CGBitmapContextCreateImage(c2);
      NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithCGImage:img];
      NSString *dir = NSProcessInfo.processInfo.environment[@"BENCH_DUMP"];
      [NSFileManager.defaultManager createDirectoryAtPath:dir withIntermediateDirectories:YES attributes:nil error:nil];
      [[rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:[dir stringByAppendingFormat:@"/full-%d-%.0f.png", fullDumps, blank] atomically:YES];
      CGImageRelease(img); CGContextRelease(c2); free(p2); fullDumps++;
    }
  }
  if (getenv("BENCH_DUMPALL") && self.dumped < 3) { self.dumped++; [self dumpAll:ctx]; }
  CGContextRelease(ctx); free(px);
  return blank;
}
- (void)dumpAll:(CGContextRef)ctx {
  NSString *dir = NSProcessInfo.processInfo.environment[@"BENCH_DUMP"] ?: @"/tmp";
  [NSFileManager.defaultManager createDirectoryAtPath:dir withIntermediateDirectories:YES attributes:nil error:nil];
  CGImageRef img = CGBitmapContextCreateImage(ctx);
  NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithCGImage:img];
  [[rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:[dir stringByAppendingFormat:@"/frame-%ld.png", (long)self.dumped] atomically:YES];
  CGImageRelease(img);
}

// ---- inner lists (innerfling, innerkeep) --------------------------------------------
static BOOL isInner(NSString *sc) { return [sc isEqualToString:@"innerfling"] || [sc isEqualToString:@"innerkeep"]; }
static BOOL horizontalKind(NSString *kind) { return [kind isEqualToString:@"filmstrip"]; }
static CGFloat innerOffset(NSScrollView *v, BOOL x) { return x ? offX(v) : offY(v); }
static void setInnerOffset(NSScrollView *v, BOOL x, CGFloat value) {
  CGSize c = contentSizeOf(v), b = v.contentView.bounds.size;
  if (x) setOff(v, MAX(0, MIN(MAX(0, c.width - b.width), value)), offY(v));
  else setOff(v, offX(v), MAX(0, MIN(MAX(0, c.height - b.height), value)));
}
/// A view's rect in the feed's document, top-down (y from the document's top).
static CGRect rectInFeed(NSView *v, NSScrollView *s) {
  NSView *d = s.documentView;
  NSRect f = [v convertRect:v.bounds toView:d];
  if (!d.isFlipped) f.origin.y = d.frame.size.height - f.origin.y - f.size.height;
  return f;
}
static CGRect rectInWindow(NSView *v) { return [v convertRect:v.bounds toView:nil]; }

- (NSArray<NSScrollView *> *)innerOfKind:(NSString *)kind {
  NSMutableArray *all = [NSMutableArray array], *out = [NSMutableArray array];
  collect(self.scroll.documentView, all);
  CGFloat W = self.scroll.frame.size.width;
  for (NSScrollView *v in all) {
    if (v == self.scroll || !v.window || v.isHiddenOrHasHiddenAncestor || v.alphaValue < 0.01) continue;
    CGSize c = contentSizeOf(v), b = v.frame.size;
    BOOL ok = horizontalKind(kind) ? (c.width >= 20000 && b.height < 300)
                                   : (c.height >= 20000 && b.height >= 300 && b.height <= 600 && b.width < W);
    if (ok) [out addObject:v];
  }
  return out;
}

- (BOOL)findStep:(NSString *)kind now:(double)now {
  NSScrollView *s = self.scroll;
  CGFloat top = offY(s), visH = s.contentView.bounds.size.height;
  CGRect vis = CGRectMake(0, top, s.contentView.bounds.size.width, visH);
  NSScrollView *inView = nil, *nearest = nil; CGRect nf = CGRectZero;
  for (NSScrollView *v in [self innerOfKind:kind]) {
    CGRect f = rectInFeed(v, s);
    if (CGRectContainsRect(CGRectInset(vis, -1, -1), f)) { if (!inView || v == self.driven) inView = v; }
    else if (!nearest || fabs(CGRectGetMidY(f) - CGRectGetMidY(vis)) < fabs(CGRectGetMidY(nf) - CGRectGetMidY(vis))) { nearest = v; nf = f; }
  }
  if (inView) {
    if (inView != self.driven || self.placedAt <= 0) { self.driven = inView; self.placedAt = now; }
    return now - self.placedAt >= 0.8;
  }
  self.placedAt = 0;
  CGFloat hi = contentSizeOf(s).height - visH;
  CGFloat y = nearest ? nf.origin.y - 0.25 * visH : top + 0.6 * visH;
  setOffY(s, MAX(0, MIN(hi, y)));
  return NO;
}

- (double)innerBlank:(NSScrollView *)v horizontal:(BOOL)x {
  if (!v.window) return -1;
  NSView *cv = v.window.contentView;
  NSRect r = NSIntersectionRect([v convertRect:v.bounds toView:cv], cv.bounds);
  if (!cv.isFlipped) r.origin.y = cv.bounds.size.height - r.origin.y - r.size.height;
  const CGFloat scale = 0.5, margin = 4;
  size_t W, H; CGContextRef ctx;
  uint8_t *px = [self renderRect:r scale:scale w:&W h:&H ctx:&ctx];
  if (!px) return -1;
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
  CGContextRelease(ctx); free(px);
  return blank;
}

- (void)enterStep:(NSDictionary *)seg {
  if (seg[@"find"]) { self.driven = nil; self.placedAt = 0; self.stepDone = NO; }
  NSString *kind = seg[@"mark"];
  if (kind && self.driven) {
    NSScrollView *v = self.driven; BOOL x = horizontalKind(kind);
    setInnerOffset(v, x, [seg[@"to"] doubleValue]);
    self.marks[kind] = @{@"outer": @(offY(self.scroll)), @"rect": [NSValue valueWithRect:rectInWindow(v)],
                         @"value": @(innerOffset(v, x))};
    [self.markViews setObject:v forKey:kind];
  }
}
- (void)exitStep:(NSDictionary *)seg {
  if (seg[@"find"] && self.driven) {
    NSScrollView *v = self.driven;
    [self.innerInfo addObject:@{@"kind": seg[@"find"], @"class": NSStringFromClass(v.class),
                                @"bounds": NSStringFromSize(v.frame.size), @"content": NSStringFromSize(contentSizeOf(v)),
                                @"outerOffset": @(offY(self.scroll))}];
  }
  NSString *kind = seg[@"mark"];
  if (kind && self.marks[kind] && self.driven) {
    NSMutableDictionary *m = [self.marks[kind] mutableCopy];
    m[@"read"] = @(innerOffset(self.driven, horizontalKind(kind)));
    self.marks[kind] = m;
  }
  kind = seg[@"returnTo"];
  if (kind && self.marks[kind]) setOffY(self.scroll, [self.marks[kind][@"outer"] doubleValue]);
  kind = seg[@"check"];
  if (kind && self.marks[kind]) {
    NSDictionary *m = self.marks[kind];
    CGRect want = [m[@"rect"] rectValue];
    NSScrollView *best = nil; double bd = 1e9;
    for (NSScrollView *v in [self innerOfKind:kind]) {
      CGRect f = rectInWindow(v);
      double d = hypot(f.origin.x - want.origin.x, f.origin.y - want.origin.y);
      if (d < bd) { bd = d; best = v; }
    }
    NSMutableDictionary *r = [@{@"kind": kind, @"trip": seg[@"trip"] ?: @"", @"mark": m[@"value"],
                                @"markRead": m[@"read"] ?: [NSNull null]} mutableCopy];
    if (best && bd < 100) {
      double got = innerOffset(best, horizontalKind(kind));
      r[@"got"] = @(got); r[@"err"] = @(got - [m[@"value"] doubleValue]); r[@"rowShiftPt"] = @(bd);
      r[@"sameView"] = @(best == [self.markViews objectForKey:kind]);
      r[@"kept"] = @(fabs(got - [m[@"value"] doubleValue]) < 1);
      self.driven = best;
    } else { r[@"got"] = [NSNull null]; r[@"kept"] = @NO; r[@"note"] = @"no inner list at the marked place"; }
    [self.keep addObject:r];
  }
}

- (void)trackPeak {
  uint64_t f = footprint(NULL);
  if (f > self.footPeak) {
    self.footPeak = f;
    if (gVm && f > self.vmPeakAt + 64000000) { self.vmPeakAt = f; self.vmPeak = vmBreakdown(); }
  }
}

- (CADisplayLink *)makeLink {
  NSWindow *w = appWindow();
  NSView *v = w.contentView;
  NSScreen *screen = w.screen ?: NSScreen.mainScreen;
  self.maxFps = screen.maximumFramesPerSecond ?: 60;
  CADisplayLink *l = v ? [v displayLinkWithTarget:self selector:@selector(tick:)]
                       : [screen displayLinkWithTarget:self selector:@selector(tick:)];
  l.preferredFrameRateRange = CAFrameRateRangeMake(self.maxFps, self.maxFps, self.maxFps);
  return l;
}

- (void)setup {
  NSDictionary *env = NSProcessInfo.processInfo.environment;
  self.scenario = env[@"BENCH_SCENARIO"] ?: @"fling";
  self.sample = [env[@"BENCH_SAMPLE"] integerValue];
  NSString *render = env[@"BENCH_RENDER"];
  self.useWindow = [render isEqualToString:@"window"];
  self.useLayer = [render isEqualToString:@"layer"] || (!render.length && [self.scenario isEqualToString:@"coldstart"]);
  self.live = [env[@"BENCH_LIVE"] length] && ![env[@"BENCH_LIVE"] isEqualToString:@"0"];
  self.dts = [NSMutableArray array]; self.expected = [NSMutableArray array]; self.busyMs = [NSMutableArray array];
  self.blankPts = [NSMutableArray array]; self.setCost = [NSMutableArray array]; self.jumps = [NSMutableArray array];
  self.segments = [NSMutableArray array]; self.segStats = [NSMutableArray array]; self.timeline = [NSMutableArray array];
  self.keep = [NSMutableArray array]; self.innerInfo = [NSMutableArray array]; self.marks = [NSMutableDictionary dictionary];
  self.markViews = [NSMapTable strongToWeakObjectsMapTable];
  self.footStart = self.footPeak = footprint(NULL);
  self.last = 0; self.seg = -1; self.jumpIndex = -1;
}

- (void)startColdstart {
  [self setup];
  self.sample = 1;
  self.runSnap = snapNow();
  [self coldWait];
}
// macOS: the display link belongs to a view, so coldstart polls (every ~2 ms) until the app's window
// exists, then samples every display-link frame from there. The window's appearance time is recorded.
- (void)coldWait {
  if (!appWindow().contentView) {
    if (CACurrentMediaTime() - gProcStart > 30) { [self finish:@"coldstart: no window within 30 s"]; return; }
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 2 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{ [self coldWait]; });
    return;
  }
  self.link = [self makeLink];
  [self.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
}

- (void)start {
  [self setup];
  [self.segments addObject:@{@"v": @3000, @"dir": @1, @"warm": @1}];
  [self.segments addObject:@{@"v": @3000, @"dir": @-1, @"warm": @1}];
  NSArray *speeds = [self.scenario isEqualToString:@"ladder"] ? @[@3000, @6000, @12000, @24000, @48000, @96000]
                                                              : @[@1000, @3000, @6000, @12000, @24000];
  for (NSNumber *v in speeds) {
    [self.segments addObject:@{@"v": v, @"dir": @1}];
    [self.segments addObject:@{@"v": v, @"dir": @-1}];
  }
  self.scroll = [self findScroll];
  if ([self.scenario isEqualToString:@"layers"]) {
    // Debug: every view whose window rect meets BENCH_LAYERS_Y0..Y1, with its layer and sublayers.
    NSDictionary *env = NSProcessInfo.processInfo.environment;
    CGFloat y0 = [env[@"BENCH_LAYERS_Y0"] doubleValue], y1 = [env[@"BENCH_LAYERS_Y1"] doubleValue] ?: 400;
    NSMutableString *out = [NSMutableString string];
    NSView *cv = appWindow().contentView;
    __block void (^walk)(NSView *, int);
    walk = ^(NSView *v, int depth) {
      NSRect r = [v convertRect:v.bounds toView:nil];
      CGFloat top = cv.bounds.size.height - NSMaxY(r);
      BOOL meets = top < y1 && top + r.size.height > y0;
      if (meets && depth < 40) {
        CALayer *l = v.layer;
        [out appendFormat:@"%*s%@ %@ top=%.1f wul=%d layer=%@ bg=%@ cr=%.1f mtb=%d contents=%@ hidden=%d op=%.2f\n", depth * 2, "", v.class, NSStringFromRect(v.frame), top,
          [v respondsToSelector:@selector(wantsUpdateLayer)] ? (int)v.wantsUpdateLayer : -1, l.class, l.backgroundColor, l.cornerRadius, l.masksToBounds, l.contents ? @"yes" : @"no", l.hidden, l.opacity];
        for (CALayer *s in l.sublayers) {
          BOOL isView = NO; for (NSView *c in v.subviews) if (c.layer == s) isView = YES;
          if (!isView) [out appendFormat:@"%*s  ~sub %@ frame=%@ bg=%@ contents=%@ hidden=%d op=%.2f mask=%@\n", depth * 2, "", s.class, NSStringFromRect(s.frame), s.backgroundColor, s.contents ? @"yes" : @"no", s.hidden, s.opacity, s.mask];
        }
      }
      for (NSView *c in v.subviews) walk(c, depth + 1);
    };
    walk(cv, 0);
    [out writeToFile:env[@"BENCH_OUT"] atomically:YES encoding:NSUTF8StringEncoding error:nil];
    exit(0);
  }
  if ([self.scenario isEqualToString:@"rendertest"]) {
    // Task 4 diagnosis: the window's content rendered by -renderInContext: from the model layer tree and
    // from the presentation tree, and the window server's picture; BENCH_OUT is a directory.
    NSString *dir = NSProcessInfo.processInfo.environment[@"BENCH_OUT"];
    [NSFileManager.defaultManager createDirectoryAtPath:dir withIntermediateDirectories:YES attributes:nil error:nil];
    NSView *cv = appWindow().contentView;
    if (!self.scroll) self.scroll = (NSScrollView *)cv;
    CGRect r = CGRectMake(0, 0, cv.bounds.size.width, cv.bounds.size.height);
    for (NSString *mode in @[@"model", @"presentation", @"window"]) {
      size_t W = (size_t)r.size.width, H = (size_t)r.size.height;
      uint8_t *px = calloc(W * H * 4, 1);
      CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
      CGContextRef ctx = CGBitmapContextCreate(px, W, H, 8, W * 4, cs, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
      CGColorSpaceRelease(cs);
      if ([mode isEqualToString:@"window"]) {
        self.useWindow = YES; free(px); CGContextRelease(ctx);
        px = [self renderRect:r scale:1 w:&W h:&H ctx:&ctx];
        self.useWindow = NO;
      } else {
        CALayer *root = cv.layer;
        if (root.geometryFlipped) { CGContextTranslateCTM(ctx, 0, H); CGContextScaleCTM(ctx, 1, -1); }
        [([mode isEqualToString:@"model"] ? root : (root.presentationLayer ?: root)) renderInContext:ctx];
      }
      CGImageRef img = CGBitmapContextCreateImage(ctx);
      NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithCGImage:img];
      [[rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:[dir stringByAppendingFormat:@"/%@.png", mode] atomically:YES];
      CGImageRelease(img); CGContextRelease(ctx); free(px);
    }
    exit(0);
  }
  if ([self.scenario isEqualToString:@"shot"]) {
    // A full-scale capture of the window's content (the list's viewport and the top bar) to BENCH_OUT (PNG).
    self.useWindow = YES;
    NSView *cv = appWindow().contentView;
    if (!self.scroll) self.scroll = (NSScrollView *)cv; // renderRect only needs .window
    CGRect r = CGRectMake(0, 0, cv.bounds.size.width, cv.bounds.size.height);
    size_t W, H; CGContextRef ctx;
    CGFloat scale = appWindow().backingScaleFactor;
    uint8_t *px = [self renderRect:r scale:scale w:&W h:&H ctx:&ctx];
    CGImageRef img = CGBitmapContextCreateImage(ctx);
    NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithCGImage:img];
    [[rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:NSProcessInfo.processInfo.environment[@"BENCH_OUT"] atomically:YES];
    CGImageRelease(img); CGContextRelease(ctx); free(px);
    exit(0);
  }
  NSLog(@"[probe] start %@ window=%@ scroll=%@ content=%@ doc=%@", self.scenario, appWindow(), self.scroll, NSStringFromSize(contentSizeOf(self.scroll)), self.scroll.documentView.class);
  if (!self.scroll) { [self finish:@"no scroll view with contentSize > 20000 found"]; return; }
  self.link = [self makeLink];
  NSLog(@"[probe] link %@ maxFps %.0f", self.link, self.maxFps);
  if ([self.scenario isEqualToString:@"rest"]) {
    [self.segments removeAllObjects];
    [self.segments addObject:@{@"v": @0, @"dir": @1, @"rest": @1}];
    self.runSnap = snapNow(); self.lastBusy = busyNow();
    [self.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    return;
  }
  if (isInner(self.scenario)) {
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
    setOffY(self.scroll, contentSizeOf(self.scroll).height / 3);
  }
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), dispatch_get_main_queue(), ^{
    self.runSnap = snapNow(); self.lastBusy = busyNow();
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
  if (getenv("BENCH_MAPCOUNT")) {
    Class mk = NSClassFromString(@"MKMapView"); NSMutableArray *views = [NSMutableArray array];
    __block void (^walk)(NSView *);
    walk = ^(NSView *v) { if (mk && [v isKindOfClass:mk]) [views addObject:v]; for (NSView *c in v.subviews) walk(c); };
    NSView *root = appWindow().contentView; if (root) walk(root);
    NSInteger hidden = 0, shown = 0;
    for (NSView *v in views) { if (v.isHiddenOrHasHiddenAncestor || v.alphaValue < 0.01) hidden++; else if (NSIntersectsRect([v convertRect:v.bounds toView:nil], root.bounds)) shown++; }
    d[@"maps"] = @[@(views.count), @(hidden), @(shown), @(aliveMaps())];
    d[@"mapWork"] = @{@"inits": @(gMapInits), @"region": @(gMapRegion), @"camera": @(gMapCamera), @"rect": @(gMapRect), @"center": @(gMapCenter)};
  }
  [self.segStats addObject:d];
}

- (void)coldTick:(CADisplayLink *)l {
  CFTimeInterval now = CACurrentMediaTime();
  [self trackPeak];
  if (now - gProcStart > 30) { [self finish:@"coldstart: no blank-free list frame within 30 s of process start"]; return; }
  if (!self.scroll) self.scroll = [self findScroll];
  double b = self.scroll ? [self blankPoints] : -1;
  double lead = 0;
  if (b > 0 && self.lastLeading > 0 && self.lastLeading < 120 && offY(self.scroll) <= 1) { lead = self.lastLeading; b -= lead; }
  [self.timeline addObject:@[@((now - gProcStart) * 1000), @(self.scroll != nil), @(b), @(lead)]];
  if (self.timeline.count == 1) self.firstContentMs = (now - gProcStart) * 1000;
  if (!self.scroll || b != 0) return;
  Snap s = snapNow();
  uint64_t lifetimePeak = 0; uint64_t foot = footprint(&lifetimePeak);
  NSUInteger foundAt = [self.timeline indexOfObjectPassingTest:^BOOL(NSArray *e, NSUInteger i, BOOL *stop) { return [e[1] boolValue]; }];
  self.cold = @{
    @"ms": @((now - gProcStart) * 1000), @"targetMs": @((l.targetTimestamp - gProcStart) * 1000),
    @"source": gStartSource, @"procToCtorMs": @((gLoad - gProcStart) * 1000),
    @"firstTickMs": @(self.firstContentMs), @"scrollFoundMs": self.timeline[foundAt][0],
    @"frames": @(self.timeline.count), @"leadingGapPt": @(lead),
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
  self.last = now;
  [self trackPeak];
  NSScrollView *s = self.scroll;
  CGFloat maxY = contentSizeOf(s).height - s.contentView.bounds.size.height;
  if ([self.scenario isEqualToString:@"jump"]) {
    if (self.jumpIndex < 0 || (self.cleanRun >= 3) || now - self.jumpAt > 3.0) {
      if (self.jumpIndex >= 0) {
        BOOL ok = self.cleanRun >= 3;
        [self.jumps addObject:@{@"target": @(offY(s)), @"ms": ok ? @((self.lastCleanAt - self.jumpAt) * 1000) : [NSNull null]}];
      }
      self.jumpIndex++;
      if (self.jumpIndex >= 10) { [self finish:nil]; return; }
      double frac = fmod(0.137 + self.jumpIndex * 0.618, 1.0);
      setOffY(s, frac * maxY);
      self.jumpAt = CACurrentMediaTime(); self.cleanRun = 0;
      return;
    }
    double b = [self blankPoints];
    [self.blankPts addObject:@(b)];
    if (b == 0) { if (self.cleanRun == 0) self.lastCleanAt = CACurrentMediaTime(); self.cleanRun++; }
    else self.cleanRun = 0;
    return;
  }
  BOOL rest = [self.scenario isEqualToString:@"rest"];
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
    [self.dts addObject:@(-1)]; [self.expected addObject:@(-1)]; [self.busyMs addObject:@(-1)];
    [self enterStep:self.segments[self.seg]];
  }
  NSDictionary *seg = self.segments[self.seg];
  if (seg[@"mark"] && self.driven) {
    // The marked inner list's offset through the settle (task 4: macOS moved it after the mark).
    [self.timeline addObject:@[seg[@"mark"], @((now - self.segStart) * 1000), @(innerOffset(self.driven, horizontalKind(seg[@"mark"])))]];
  }
  if (seg[@"find"] || (isInner(self.scenario) && [seg[@"v"] doubleValue] == 0)) {
    if (seg[@"find"] && [self findStep:seg[@"find"] now:now]) self.stepDone = YES;
    self.segFrames++; self.frame++;
    if (self.sample > 0) [self.blankPts addObject:@(-1)];
    return;
  }
  if (seg[@"inner"]) {
    NSScrollView *v = self.driven;
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
  CGFloat y0 = offY(s);
  CGFloat y = MAX(0, MIN(maxY, y0 + [seg[@"dir"] doubleValue] * [seg[@"v"] doubleValue] * dt));
  CFTimeInterval set0 = CACurrentMediaTime();
  setOffY(s, y);
  [self.setCost addObject:@((CACurrentMediaTime() - set0) * 1000)];
  self.segTravel += fabs(y - y0); self.segFrames++;
  self.frame++;
  if (self.sample > 0 && self.frame % self.sample == 0) [self.blankPts addObject:@([self blankPoints])];
  else if (self.sample > 0) [self.blankPts addObject:@(-1)];
}

- (void)finish:(NSString *)error {
  if (self.done) return;
  self.done = YES;
  [self.link invalidate];
  [self emit:error final:YES];
  if ([NSProcessInfo.processInfo.environment[@"BENCH_EXIT"] isEqualToString:@"1"]) exit(0);
}

- (void)emit:(NSString *)error final:(BOOL)final {
  NSMutableDictionary *out = [NSMutableDictionary dictionary];
  out[@"scenario"] = self.scenario; out[@"sample"] = @(self.sample);
  out[@"bundle"] = NSBundle.mainBundle.bundleIdentifier ?: @"";
  out[@"maxFps"] = @(self.maxFps);
  out[@"live"] = @(self.live);
  out[@"platform"] = @"macos";
  NSWindow *w = self.scroll.window ?: appWindow();
  if (w) {
    out[@"window"] = @{@"content": NSStringFromSize(w.contentView.frame.size), @"backingScale": @(w.backingScaleFactor),
                       @"screen": NSStringFromSize(w.screen.frame.size), @"screenMaxFps": @(w.screen.maximumFramesPerSecond),
                       @"visible": @((w.occlusionState & NSWindowOcclusionStateVisible) != 0), @"key": @(w.isKeyWindow),
                       @"appActive": @(NSApp.isActive), @"frame": NSStringFromRect(w.frame)};
  }
  if (error) out[@"error"] = error;
  out[@"scrollClass"] = NSStringFromClass(self.scroll.class) ?: @"";
  out[@"docClass"] = NSStringFromClass(self.scroll.documentView.class) ?: @"";
  out[@"contentHeight"] = @(contentSizeOf(self.scroll).height);
  if (self.scroll) out[@"viewport"] = NSStringFromRect([self visibleTopDown:self.scroll]);
  out[@"dts"] = [self.dts copy]; out[@"expected"] = [self.expected copy]; out[@"busyFrameMs"] = [self.busyMs copy];
  out[@"blank"] = [self.blankPts copy]; out[@"setCost"] = [self.setCost copy]; out[@"jumps"] = [self.jumps copy];
  out[@"segments"] = [self.segments copy]; out[@"segStats"] = [self.segStats copy];
  out[@"run"] = delta(self.runSnap, snapNow(), self.ticks);
  uint64_t lifetimePeak = 0; uint64_t end = footprint(&lifetimePeak);
  if (end > self.footPeak) self.footPeak = end;
  out[@"mem"] = @{@"load": @(gFootLoad), @"start": @(self.footStart), @"peak": @(self.footPeak), @"end": @(end),
                  @"lifetimePeak": @(lifetimePeak), @"rest": @0};
  if (gVm) { out[@"vmEnd"] = vmBreakdown(); if (self.vmPeak) out[@"vmPeak"] = self.vmPeak; }
  out[@"startSource"] = gStartSource;
  out[@"procToCtorMs"] = @((gLoad - gProcStart) * 1000);
  if (self.cold) out[@"coldstart"] = self.cold;
  if (isInner(self.scenario)) { out[@"keep"] = [self.keep copy]; out[@"innerLists"] = [self.innerInfo copy]; }
  if (self.timeline.count) out[@"timeline"] = [self.timeline copy];
  out[@"firstContentMs"] = @(self.firstContentMs);
  out[@"sampleMs"] = @(self.sampleCount ? 1000 * self.sampleCost / self.sampleCount : 0);
  out[@"render"] = self.useWindow ? @"window" : self.useLayer ? @"layer" : @"hierarchy";
  {
    // Map views in the window at the end: all, hidden (or under a hidden ancestor), and on screen.
    NSMutableArray *views = [NSMutableArray array];
    __block void (^walk)(NSView *);
    Class mk = NSClassFromString(@"MKMapView");
    walk = ^(NSView *v) { if (mk && [v isKindOfClass:mk]) [views addObject:v]; for (NSView *c in v.subviews) walk(c); };
    NSView *root = appWindow().contentView; if (root) walk(root);
    NSInteger hidden = 0, shown = 0;
    for (NSView *v in views) {
      if (v.isHiddenOrHasHiddenAncestor || v.alphaValue < 0.01) hidden++;
      else if (NSIntersectsRect([v convertRect:v.bounds toView:nil], root.bounds)) shown++;
    }
    out[@"mapViews"] = @{@"inWindow": @(views.count), @"hidden": @(hidden), @"onScreen": @(shown)};
  }
  NSString *path = NSProcessInfo.processInfo.environment[@"BENCH_OUT"];
  if (!final) {
    out[@"partial"] = @YES; out[@"segmentsDone"] = @(self.seg); path = [path stringByAppendingString:@".partial"];
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

static LBProbe *gProbe;

// macOS: both apps get the same window FRAME (BENCH_WIN_W x BENCH_WIN_H points, default 1366 x 940, at the
// top-left of the screen's visible area), set by the probe as soon as the app's window exists, so the
// list's viewport is the same whatever each app's own sizing (SwiftUI's windows have a full-size content
// view, exact2's a titled one: the viewport is the frame minus the title bar in both). Recorded as "window".
static void sizeWindow(int tries) {
  NSWindow *w = appWindow();
  if (!w) { if (tries < 3000) dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{ sizeWindow(tries + 1); }); return; }
  NSDictionary *env = NSProcessInfo.processInfo.environment;
  CGFloat W = [env[@"BENCH_WIN_W"] doubleValue] ?: 1366, H = [env[@"BENCH_WIN_H"] doubleValue] ?: 940;
  NSRect vf = (w.screen ?: NSScreen.mainScreen).visibleFrame;
  [w setFrame:NSMakeRect(vf.origin.x, NSMaxY(vf) - H, W, H) display:YES];
}

__attribute__((constructor)) static void heavybench_init(void) {
  installMetalHook();
  if (getenv("BENCH_MAPCOUNT")) dispatch_async(dispatch_get_main_queue(), ^{ installMapHook(); });
  gLoad = CACurrentMediaTime();
  gFootLoad = footprint(NULL);
  gVm = [NSProcessInfo.processInfo.environment[@"BENCH_VM"] isEqualToString:@"1"];
  gMainThread = pthread_mach_thread_np(pthread_self());
  gProcStart = gLoad;
  struct kinfo_proc kp; size_t len = sizeof kp;
  int mib[4] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()};
  if (sysctl(mib, 4, &kp, &len, NULL, 0) == 0 && len >= sizeof kp && kp.kp_proc.p_starttime.tv_sec > 0) {
    struct timeval nowWall; gettimeofday(&nowWall, NULL);
    double media = CACurrentMediaTime();
    double ago = (nowWall.tv_sec - kp.kp_proc.p_starttime.tv_sec) + (nowWall.tv_usec - kp.kp_proc.p_starttime.tv_usec) / 1e6;
    if (ago >= 0 && ago < 60) { gProcStart = media - ago; gStartSource = @"kern_proc"; }
  }
  gAwake = YES; gAwakeAt = gLoad;
  CFRunLoopObserverRef wake = CFRunLoopObserverCreate(NULL, kCFRunLoopAfterWaiting, true, LONG_MIN, busyObserver, NULL);
  CFRunLoopObserverRef sleep = CFRunLoopObserverCreate(NULL, kCFRunLoopBeforeWaiting, true, LONG_MAX, busyObserver, NULL);
  CFRunLoopAddObserver(CFRunLoopGetMain(), wake, kCFRunLoopCommonModes);
  CFRunLoopAddObserver(CFRunLoopGetMain(), sleep, kCFRunLoopCommonModes);
  NSDictionary *env = NSProcessInfo.processInfo.environment;
  // macOS: keep the display and system awake while a run is in progress (App Nap would throttle an
  // occluded or inactive app's timers and display link).
  static id activity;
  activity = [NSProcessInfo.processInfo beginActivityWithOptions:NSActivityUserInitiated | NSActivityLatencyCritical | NSActivityIdleDisplaySleepDisabled
                                                          reason:@"xheavy probe"];
  dispatch_async(dispatch_get_main_queue(), ^{ sizeWindow(0); });
  if ([env[@"BENCH_SCENARIO"] isEqualToString:@"coldstart"]) {
    dispatch_async(dispatch_get_main_queue(), ^{ gProbe = [LBProbe new]; [gProbe startColdstart]; });
    return;
  }
  double delay = [env[@"BENCH_DELAY"] doubleValue] ?: 5;
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
    gProbe = [LBProbe new]; [gProbe start];
  });
}
