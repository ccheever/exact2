// listbench probe — injected into both apps (SIMCTL_CHILD_DYLD_INSERT_LIBRARIES).
// Finds the tallest UIScrollView, drives it from a CADisplayLink on the main
// thread, and writes one JSON result to $BENCH_OUT.
//
// BENCH_SCENARIO: fling (constant-speed segments, down then up) | jump (ten
//                 absolute jumps, time until the viewport has no blank band)
// BENCH_SAMPLE:   1 = snapshot the list every frame and measure blank bands
//                 (perturbs timing; run timing and blank passes separately)
// BENCH_DELAY:    seconds after launch before starting (default 5)
#import <UIKit/UIKit.h>
#import <QuartzCore/QuartzCore.h>
#include <stdlib.h>

static CFTimeInterval gLoad;

@interface LBProbe : NSObject
@property (nonatomic, weak) UIScrollView *scroll;
@property (nonatomic, strong) CADisplayLink *link;
@property (nonatomic) NSInteger sample, frame, dumped;
@property (nonatomic, copy) NSString *scenario;
@property (nonatomic, strong) NSMutableArray<NSNumber *> *dts, *expected, *blankPts;
@property (nonatomic, strong) NSMutableArray *segments, *jumps;
@property (nonatomic) CFTimeInterval last, segStart, jumpAt, lastCleanAt;
@property (nonatomic) NSInteger seg, jumpIndex, cleanRun;
@property (nonatomic) double firstContentMs, sampleCost;
@property (nonatomic) NSInteger sampleCount;
@property (nonatomic, strong) NSMutableArray<NSNumber *> *setCost;
@property (nonatomic) BOOL useLayer;
@end

@implementation LBProbe

static UIWindow *keyWindow(void) {
  for (UIScene *s in UIApplication.sharedApplication.connectedScenes) {
    if (![s isKindOfClass:UIWindowScene.class]) continue;
    for (UIWindow *w in ((UIWindowScene *)s).windows) if (w.isKeyWindow) return w;
  }
  return nil;
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
// luminance range across the inner width is < 10.
- (double)blankPoints {
  UIScrollView *s = self.scroll;
  UIWindow *w = s.window;
  if (!w) return -1;
  CGRect r = [s convertRect:s.bounds toView:w];
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
  if (self.useLayer) [w.layer.presentationLayer ?: w.layer renderInContext:ctx];
  else [w drawViewHierarchyInRect:w.bounds afterScreenUpdates:NO];
  self.sampleCost += CACurrentMediaTime() - t0; self.sampleCount++;
  UIGraphicsPopContext();
  size_t x0 = (size_t)(margin * scale), x1 = W - x0;
  double blank = 0; size_t run = 0;
  const size_t minRun = (size_t)(60 * scale);
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

- (void)start {
  NSDictionary *env = NSProcessInfo.processInfo.environment;
  self.scenario = env[@"BENCH_SCENARIO"] ?: @"fling";
  self.sample = [env[@"BENCH_SAMPLE"] integerValue];
  self.useLayer = [env[@"BENCH_RENDER"] isEqualToString:@"layer"];
  self.dts = [NSMutableArray array]; self.expected = [NSMutableArray array];
  self.blankPts = [NSMutableArray array]; self.setCost = [NSMutableArray array]; self.jumps = [NSMutableArray array];
  self.segments = [NSMutableArray array];
  [self.segments addObject:@{@"v": @3000, @"dir": @1, @"warm": @1}];
  [self.segments addObject:@{@"v": @3000, @"dir": @-1, @"warm": @1}];
  for (NSNumber *v in @[@1000, @3000, @6000, @12000, @24000]) {
    [self.segments addObject:@{@"v": v, @"dir": @1}];
    [self.segments addObject:@{@"v": v, @"dir": @-1}];
  }
  self.scroll = [self findScroll];
  if (!self.scroll) { [self finish:@"no scroll view with contentSize > 20000 found"]; return; }
  CGPoint o = self.scroll.contentOffset;
  o.y = self.scroll.contentSize.height / 3;
  self.scroll.contentOffset = o;
  self.link = [CADisplayLink displayLinkWithTarget:self selector:@selector(tick:)];
  float maxFps = (float)UIScreen.mainScreen.maximumFramesPerSecond;
  self.link.preferredFrameRateRange = CAFrameRateRangeMake(maxFps, maxFps, maxFps);
  self.last = 0; self.seg = -1; self.jumpIndex = -1;
  // settle one second at the start offset before measuring
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), dispatch_get_main_queue(), ^{
    [self.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
  });
}

- (void)tick:(CADisplayLink *)l {
  CFTimeInterval now = l.timestamp;
  double exp = l.targetTimestamp - l.timestamp;
  if (self.last > 0) { [self.dts addObject:@(now - self.last)]; [self.expected addObject:@(exp)]; }
  double dt = self.last > 0 ? now - self.last : 0;
  self.last = now;
  UIScrollView *s = self.scroll;
  CGFloat maxY = s.contentSize.height - s.bounds.size.height;
  if ([self.scenario isEqualToString:@"jump"]) {
    if (self.jumpIndex < 0 || (self.cleanRun >= 3) || now - self.jumpAt > 3.0) {
      if (self.jumpIndex >= 0) {
        BOOL ok = self.cleanRun >= 3;
        [self.jumps addObject:@{@"target": @(s.contentOffset.y), @"ms": ok ? @((self.lastCleanAt - self.jumpAt) * 1000) : [NSNull null]}];
      }
      self.jumpIndex++;
      if (self.jumpIndex >= 10) { [self finish:nil]; return; }
      // fixed pseudo-random targets across the content
      double frac = fmod(0.137 + self.jumpIndex * 0.618, 1.0);
      CGPoint o = s.contentOffset; o.y = frac * maxY; s.contentOffset = o;
      self.jumpAt = CACurrentMediaTime(); self.cleanRun = 0;
      return;
    }
    double b = [self blankPoints];
    [self.blankPts addObject:@(b)];
    if (b == 0) { if (self.cleanRun == 0) self.lastCleanAt = CACurrentMediaTime(); self.cleanRun++; }
    else self.cleanRun = 0;
    return;
  }
  // fling: constant-speed segments of 2 s, position follows wall time
  if (self.seg < 0 || now - self.segStart >= 2.0) {
    self.seg++;
    if (self.seg >= (NSInteger)self.segments.count) { [self finish:nil]; return; }
    self.segStart = now;
    [self.dts addObject:@(-1)]; [self.expected addObject:@(-1)]; // segment marker
  }
  NSDictionary *seg = self.segments[self.seg];
  CGPoint o = s.contentOffset;
  o.y = MAX(0, MIN(maxY, o.y + [seg[@"dir"] doubleValue] * [seg[@"v"] doubleValue] * dt));
  CFTimeInterval set0 = CACurrentMediaTime();
  s.contentOffset = o;
  [self.setCost addObject:@((CACurrentMediaTime() - set0) * 1000)];
  self.frame++;
  if (self.sample > 0 && self.frame % self.sample == 0) [self.blankPts addObject:@([self blankPoints])];
  else if (self.sample > 0) [self.blankPts addObject:@(-1)];
}

- (void)finish:(NSString *)error {
  [self.link invalidate];
  NSMutableDictionary *out = [NSMutableDictionary dictionary];
  out[@"scenario"] = self.scenario; out[@"sample"] = @(self.sample);
  out[@"bundle"] = NSBundle.mainBundle.bundleIdentifier ?: @"";
  out[@"maxFps"] = @(UIScreen.mainScreen.maximumFramesPerSecond);
  if (error) out[@"error"] = error;
  out[@"scrollClass"] = NSStringFromClass(self.scroll.class) ?: @"";
  out[@"contentHeight"] = @(self.scroll.contentSize.height);
  out[@"dts"] = self.dts; out[@"expected"] = self.expected;
  out[@"blank"] = self.blankPts; out[@"setCost"] = self.setCost; out[@"jumps"] = self.jumps;
  out[@"segments"] = self.segments;
  out[@"firstContentMs"] = @(self.firstContentMs);
  out[@"sampleMs"] = @(self.sampleCount ? 1000 * self.sampleCost / self.sampleCount : 0);
  out[@"render"] = self.useLayer ? @"layer" : @"hierarchy";
  NSString *path = NSProcessInfo.processInfo.environment[@"BENCH_OUT"];
  if (path && ![path hasPrefix:@"/"]) {
    NSString *docs = NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES).firstObject;
    path = [docs stringByAppendingPathComponent:path];
  }
  NSData *d = [NSJSONSerialization dataWithJSONObject:out options:0 error:nil];
  if (path) [d writeToFile:path atomically:YES];
  NSLog(@"[listbench] done %@ frames=%lu error=%@", self.scenario, (unsigned long)self.dts.count, error);
}
@end

static LBProbe *gProbe;

__attribute__((constructor)) static void listbench_init(void) {
  gLoad = CACurrentMediaTime();
  double delay = NSProcessInfo.processInfo.environment[@"BENCH_DELAY"].doubleValue ?: 5;
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
    UIApplication.sharedApplication.idleTimerDisabled = YES;
    gProbe = [LBProbe new];
    [gProbe start];
  });
}
