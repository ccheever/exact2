// The scroll-feel recorder: a passive dylib inserted into any iOS simulator app
// (SIMCTL_CHILD_DYLD_INSERT_LIBRARIES). It never moves anything: touches come
// from the touch runner (host/apple/touches.mjs) and this only watches. It
// writes JSON lines to $FEEL_OUT (an absolute host path), flushed every 250 ms:
//   {"k":"hello", wall, media, maxFps, screen:[w,h], scale, bundle, pid, sample}
//   {"k":"scan", t, views:[…]}   every visible UIScrollView, when the set or a size changes
//   {"k":"touch", t, ph, x, y}   each touch phase the application dispatched (window points)
//   {"k":"f", …}                 one display-link frame (`tick:`), written while a
//                                finger is down, a scroll view moves, and 0.5 s after
// Times are CACurrentMediaTime seconds (the host's mach clock on a simulator);
// `hello` pairs it with the wall clock so the driver's gesture times map onto it.
//
// FEEL_SAMPLE=N: every Nth moving frame, once Core Animation has committed it,
// render the moving scroll view's layer tree and measure blank bands in it
// ({"k":"blank", t, blank, cost}; 0 turns it off; default 4). Rendered then,
// the tree is the one going on screen: rows laid out for this frame's offset.
// Sampling runs only while a file `$FEEL_OUT.sample` exists (checked every
// 250 ms), so the driver confines its cost to the gestures that measure
// blanks; a frame recorded while it runs carries "s":1, and `own` on each
// frame is the tick's own time. FEEL_DUMP=<dir> saves the first two samples
// and any with a blank band as PNGs.
//
// The display link asks for the screen's maximum rate, so every frame the
// app draws is seen; an app that asked for less is raised to it.
#import <UIKit/UIKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>
#include <sys/time.h>
#include <unistd.h>

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

@interface FeelProbe : NSObject
@property (nonatomic, strong) CADisplayLink *link;
@property (nonatomic, strong) NSMapTable<UIScrollView *, FeelView *> *views;
@property (nonatomic) NSInteger nextIdent, frame, movingFrames, sinceScan;
@property (nonatomic, weak) FeelView *focus;
@property (nonatomic) double lastStamp, lastBusy, lastMotion, lastScanT;
@property (nonatomic, copy) NSString *scanKey;
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
  [NSRunLoop.mainRunLoop addTimer:[NSTimer timerWithTimeInterval:0.25 repeats:YES block:^(NSTimer *t) { flush(); gSampling = access(gSwitch, F_OK) == 0; }] forMode:NSRunLoopCommonModes];
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

- (double)blankIn:(UIScrollView *)s {
  UIWindow *w = s.window;
  if (!w) return -1;
  UIEdgeInsets in = s.adjustedContentInset;
  CGRect vis = s.bounds;
  vis.origin.y += in.top; vis.size.height -= in.top + in.bottom;
  // Only the part on screen with content under it (an overscroll's gap is no blank), in the scroll view's own space.
  CGRect r = CGRectIntersection(vis, [s convertRect:w.bounds fromView:w]);
  r = CGRectIntersection(r, (CGRect){CGPointZero, s.contentSize});
  const CGFloat scale = 0.25, margin = 28;
  size_t W = (size_t)(r.size.width * scale), H = (size_t)(r.size.height * scale);
  if (W < 8 || H < 8) return -1;
  uint8_t *px = calloc(W * H * 4, 1);
  CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
  CGContextRef ctx = CGBitmapContextCreate(px, W, H, 8, W * 4, cs, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
  CGColorSpaceRelease(cs);
  CGContextTranslateCTM(ctx, 0, H);
  CGContextScaleCTM(ctx, scale, -scale);
  // Each sublayer that meets the visible rect, at its frame: the scroll view's own layer, rendered whole, was
  // clipped as if its bounds had no offset (a band as tall as the offset read blank), and the window's whole
  // tree cost about 9 ms a sample in the UIKit crypto list.
  for (CALayer *l in s.layer.sublayers) {
    CGRect f = l.frame;
    if (l.hidden || l.opacity < 0.01 || !CGRectIntersectsRect(f, r)) continue;
    CGContextSaveGState(ctx);
    CGContextTranslateCTM(ctx, f.origin.x - r.origin.x, f.origin.y - r.origin.y);
    [l renderInContext:ctx];
    CGContextRestoreGState(ctx);
  }
  // A band is a run of at least 60 pt of scanlines whose luminance range across the inner width is under 10.
  size_t x0 = (size_t)(margin * scale), x1 = W - x0, minRun = (size_t)(60 * scale), run = 0;
  double blank = 0;
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
    else { if (run >= minRun) blank += run / scale; run = 0; }
  }
  if (gDump && gDumped < 8 && (gDumped < 2 || blank > 0)) {
    CGImageRef img = CGBitmapContextCreateImage(ctx);
    [UIImagePNGRepresentation([UIImage imageWithCGImage:img]) writeToFile:[gDump stringByAppendingFormat:@"/sample-%ld-%.0f.png", (long)gDumped++, blank] atomically:NO];
    CGImageRelease(img);
  }
  CGContextRelease(ctx);
  free(px);
  return blank;
}

- (void)tick:(CADisplayLink *)l {
  double t0 = CACurrentMediaTime();
  self.frame++;
  BOOL touching = gDown > 0;
  // Rescan often enough to meet a pushed screen's list, rarely while moving.
  if (++self.sinceScan >= (t0 - self.lastMotion < 0.5 ? 30 : 8)) { self.sinceScan = 0; [self rescan:t0]; }
  // The moving view: the innermost one a finger or momentum drives, else the one whose offset changed.
  FeelView *active = nil, *shifted = nil;
  double shiftBy = 0;
  for (FeelView *fv in self.views.objectEnumerator) {
    UIScrollView *s = fv.view;
    if (!s || !s.window) continue;
    CGPoint o = s.contentOffset;
    double d = fabs(o.x - fv.last.x) + fabs(o.y - fv.last.y);
    fv.last = o;
    if ((s.isTracking || s.isDragging || s.isDecelerating) && (!active || fv.depth > active.depth)) active = fv;
    if (d > 0.01 && d > shiftBy) { shifted = fv; shiftBy = d; }
  }
  FeelView *now = active ?: shifted;
  if (now) { self.focus = now; self.lastMotion = t0; }
  BOOL record = touching || now || t0 - self.lastMotion < 0.5 || t0 - gLastTouchT < 0.5;
  double busy = busyNow(), busyMs = (busy - self.lastBusy) * 1000;
  self.lastBusy = busy;
  double stamp = l.timestamp, dt = self.lastStamp > 0 ? (stamp - self.lastStamp) * 1000 : 0;
  self.lastStamp = stamp;
  if (!record) return;
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
    [line appendFormat:@",\"v\":%ld,\"ox\":%.3f,\"oy\":%.3f,\"py\":%.3f,\"cw\":%.1f,\"ch\":%.1f,\"bw\":%.1f,\"bh\":%.1f,\"it\":%.1f,\"ib\":%.1f,\"tr\":%d,\"dr\":%d,\"de\":%d",
      (long)fv.ident, o.x, o.y, pl ? pl.bounds.origin.y : o.y, s.contentSize.width, s.contentSize.height, s.bounds.size.width, s.bounds.size.height,
      in.top, in.bottom, s.isTracking, s.isDragging, s.isDecelerating];
  }
  if (gSampling) [line appendString:@",\"s\":1"];
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
  double t0 = CACurrentMediaTime(), blank = [gProbe blankIn:s];
  if (blank >= 0) emit([NSString stringWithFormat:@"{\"k\":\"blank\",\"t\":%.6f,\"blank\":%.1f,\"cost\":%.3f}", gSampleT, blank, (CACurrentMediaTime() - t0) * 1000]);
}

__attribute__((constructor)) static void feel_init(void) {
  NSDictionary *env = NSProcessInfo.processInfo.environment;
  NSString *path = env[@"FEEL_OUT"];
  if (!path.length) return;
  if (env[@"FEEL_SAMPLE"]) gSample = [env[@"FEEL_SAMPLE"] integerValue];
  if ([env[@"FEEL_DUMP"] length]) gDump = env[@"FEEL_DUMP"];
  gOut = fopen(path.fileSystemRepresentation, "w");
  gSwitch = strdup([path stringByAppendingString:@".sample"].fileSystemRepresentation);
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
    gProbe = [FeelProbe new];
    [gProbe start];
  });
}
