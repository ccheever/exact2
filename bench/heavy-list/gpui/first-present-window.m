// Silver-Mac diagnostic fixture; no ordinary test run opens this window.
// @ref LLP 1149.001 M10, cut 1c.
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <Metal/Metal.h>
#import <objc/runtime.h>
#include <pthread.h>
#include <stdatomic.h>
#include <dlfcn.h>
static const char *callerName(void *address) {
    Dl_info info;
    return dladdr(address, &info) ? (info.dli_sname ?: info.dli_fname) : "unresolved";
}
#include "first-present.m"

@interface FirstPresent : NSObject <NSApplicationDelegate>
@property NSWindow *window;
@property CAMetalLayer *layer;
@property id<MTLCommandQueue> queue;
@property CADisplayLink *link;
@property NSString *variation;
@property NSString *output;
@property unsigned attempts;
@property BOOL callback;
@property BOOL displayed;
@end
@implementation FirstPresent
- (void)save {
    [self.link invalidate];
    NSDictionary *result = @{@"variation":self.variation, @"rows":fpSnapshot(),
        @"attempts":@(self.attempts), @"displayed":@(self.displayed)};
    NSError *error;
    NSData *data = [NSJSONSerialization dataWithJSONObject:result options:NSJSONWritingPrettyPrinted error:&error];
    if (!data || ![data writeToFile:self.output options:NSDataWritingAtomic error:&error]) abort();
    [NSApp stop:nil];
}
- (void)present {
    self.attempts++;
    self.callback = NO;
    fpAll(@"fixture before acquire", nil);
    __block id<CAMetalDrawable> drawable;
    dispatch_sync(dispatch_get_global_queue(QOS_CLASS_USER_INTERACTIVE, 0), ^{
        drawable = [self.layer nextDrawable];
        fpAcquire(self.layer, drawable);
        id<MTLCommandBuffer> command = [self.queue commandBuffer];
        MTLRenderPassDescriptor *pass = [MTLRenderPassDescriptor renderPassDescriptor];
        pass.colorAttachments[0].texture = drawable.texture;
        pass.colorAttachments[0].loadAction = MTLLoadActionClear;
        pass.colorAttachments[0].storeAction = MTLStoreActionStore;
        pass.colorAttachments[0].clearColor = MTLClearColorMake(0.1, 0.4, 0.8, 1);
        [[command renderCommandEncoderWithDescriptor:pass] endEncoding];
        [command commit]; [command waitUntilScheduled];
    });
    if (!drawable) { [self save]; return; }
    if ([self.variation isEqual:@"scale-change"] && self.attempts == 1) {
        self.layer.contentsScale = self.window.backingScaleFactor;
        self.layer.drawableSize = CGSizeMake(800*self.window.backingScaleFactor,768*self.window.backingScaleFactor);
    }
    [drawable addPresentedHandler:^(id<MTLDrawable> shown) {
        double time = shown.presentedTime;
        dispatch_async(dispatch_get_main_queue(), ^{
            self.callback = YES;
            self.displayed |= time > 0;
        });
    }];
    fpSubmit(drawable, @"fixture present");
    [drawable present];
}
- (void)tick:(CADisplayLink *)link {
    fpAll(@"fixture display callback", @{@"paused":@(link.paused), @"timestamp":@(link.timestamp), @"targetTimestamp":@(link.targetTimestamp)});
    if (self.displayed) { [self save]; return; }
    if (!self.attempts || (self.callback && self.attempts < 3)) [self present];
}
- (void)applicationDidFinishLaunching:(NSNotification *)notification {
    (void)notification;
    self.window = [[NSWindow alloc] initWithContentRect:NSMakeRect(100,100,800,768)
        styleMask:NSWindowStyleMaskTitled|NSWindowStyleMaskClosable|NSWindowStyleMaskResizable
        backing:NSBackingStoreBuffered defer:NO];
    // Winit makes its NSView layer-backed before the carrier supplies its layer.
    NSView *view = self.window.contentView;
    view.wantsLayer = YES;
    self.layer = [CAMetalLayer layer];
    self.layer.device = MTLCreateSystemDefaultDevice();
    self.layer.pixelFormat = MTLPixelFormatBGRA8Unorm;
    self.layer.framebufferOnly = YES;
    self.layer.opaque = YES;
    self.layer.maximumDrawableCount = 3;
    self.layer.presentsWithTransaction = YES;
    view.layer = self.layer; view.wantsLayer = YES;
    double scale = [self.variation isEqual:@"scale-change"] ? 1 : self.window.backingScaleFactor;
    self.layer.contentsScale = scale;
    self.layer.drawableSize = CGSizeMake(800*scale,768*scale);
    self.queue = [self.layer.device newCommandQueue];
    fpAll(@"fixture layer ready", nil);
    BOOL transaction = [self.variation isEqual:@"same-transaction"];
    if (transaction) [CATransaction begin];
    [self.window makeKeyAndOrderFront:nil];
    fpAll(@"fixture show returned", nil);
    if ([self.variation isEqual:@"flush"]) [CATransaction flush];
    if (![self.variation isEqual:@"first-tick"]) [self present];
    if ([self.variation isEqual:@"replace"]) [self present];
    if (transaction) [CATransaction commit];
    self.link = [view displayLinkWithTarget:self selector:@selector(tick:)];
    [self.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 2*NSEC_PER_SEC), dispatch_get_main_queue(), ^{ [self save]; });
}
@end
int main(int argc, char **argv) {
    if (argc != 3) return 2;
    @autoreleasepool {
        setenv("G1_FIRST_PRESENT", "1", 1); fpInit();
        FirstPresent *fixture = [FirstPresent new];
        fixture.variation = @(argv[1]); fixture.output = @(argv[2]);
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        NSApp.delegate = fixture;
        [NSApp run];
    }
    return 0;
}
