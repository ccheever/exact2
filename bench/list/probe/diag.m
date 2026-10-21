// diag.m — list tall layers under the tallest scroll view and time renderInContext
// of each against the visible rect. Output JSON to $BENCH_OUT.
#import <UIKit/UIKit.h>
#import <objc/runtime.h>

static void collect(UIView *v, NSMutableArray *out) {
  if ([v isKindOfClass:UIScrollView.class]) [out addObject:v];
  for (UIView *c in v.subviews) collect(c, out);
}

static BOOL overrides(Class c, SEL s) {
  return class_getMethodImplementation(c, s) != class_getMethodImplementation(UIView.class, s);
}

static double renderCost(CALayer *layer, CGRect visibleInLayer) {
  const CGFloat scale = 0.25;
  size_t W = MAX(1, (size_t)(visibleInLayer.size.width * scale)), H = MAX(1, (size_t)(visibleInLayer.size.height * scale));
  CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
  CGContextRef ctx = CGBitmapContextCreate(NULL, W, H, 8, W * 4, cs, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
  CGColorSpaceRelease(cs);
  CGContextScaleCTM(ctx, scale, scale);
  CGContextTranslateCTM(ctx, -visibleInLayer.origin.x, -visibleInLayer.origin.y);
  CFTimeInterval t0 = CACurrentMediaTime();
  for (int i = 0; i < 5; i++) [layer renderInContext:ctx];
  double ms = (CACurrentMediaTime() - t0) * 1000 / 5;
  CGContextRelease(ctx);
  return ms;
}

static void walk(CALayer *l, UIScrollView *s, int depth, NSMutableArray *out) {
  if (l.bounds.size.height > 5000 || depth <= 2) {
    id d = l.delegate;
    CGRect vis = [l convertRect:s.bounds fromLayer:s.layer];
    vis = CGRectIntersection(vis, l.bounds);
    NSMutableDictionary *e = [@{
      @"depth": @(depth), @"layerClass": NSStringFromClass(l.class),
      @"delegateClass": d ? NSStringFromClass([d class]) : @"",
      @"bounds": NSStringFromCGRect(l.bounds), @"hasContents": @(l.contents != nil),
      @"sublayers": @(l.sublayers.count),
      @"renderMs": CGRectIsNull(vis) || CGRectIsEmpty(vis) ? @0 : @(renderCost(l, vis)),
    } mutableCopy];
    if ([d isKindOfClass:UIView.class]) {
      e[@"overridesDrawRect"] = @(overrides([d class], @selector(drawRect:)));
      e[@"overridesDrawLayer"] = @(overrides([d class], @selector(drawLayer:inContext:)));
    }
    [out addObject:e];
  }
  for (CALayer *c in l.sublayers) walk(c, s, depth + 1, out);
}

__attribute__((constructor)) static void diag_init(void) {
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 6 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
    UIWindow *w = nil;
    for (UIScene *sc in UIApplication.sharedApplication.connectedScenes)
      for (UIWindow *x in ((UIWindowScene *)sc).windows) if (x.isKeyWindow) w = x;
    NSMutableArray *all = [NSMutableArray array]; collect(w, all);
    UIScrollView *s = nil;
    for (UIScrollView *x in all) if (x.bounds.size.height >= 300 && (!s || x.contentSize.height > s.contentSize.height)) s = x;
    CGPoint o = s.contentOffset; o.y = s.contentSize.height / 3; s.contentOffset = o;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), dispatch_get_main_queue(), ^{
      NSMutableArray *out = [NSMutableArray array];
      walk(s.layer, s, 0, out);
      NSDictionary *res = @{@"bundle": NSBundle.mainBundle.bundleIdentifier, @"scroll": NSStringFromClass(s.class),
                            @"contentSize": NSStringFromCGSize(s.contentSize),
                            @"wholeMs": @(renderCost(w.layer, [w.layer convertRect:s.bounds fromLayer:s.layer])), @"layers": out};
      NSString *path = NSProcessInfo.processInfo.environment[@"BENCH_OUT"];
      [[NSJSONSerialization dataWithJSONObject:res options:NSJSONWritingPrettyPrinted error:nil] writeToFile:path atomically:YES];
    });
  });
}
