// Linked into both benchmark copies. @ref LLP 1149 §4; heavy-list/probe/probe.m.
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <Metal/Metal.h>
#import <objc/runtime.h>
#include <libproc.h>
#include <mach/mach.h>
#include <pthread.h>
#include <stdatomic.h>
#include <sys/time.h>

static double processStart, loaded, firstDrawable, firstComplete;
static mach_port_t mainThread;
static atomic_bool completeScene;
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static NSMutableArray<NSNumber *> *presented;
static NSString *output;
static BOOL gpui;

void g1_ready(int complete) { atomic_store(&completeScene, complete != 0); }

static id nextDrawable(id layer, SEL selector) {
    static IMP original;
    if (!original) original = class_getMethodImplementation([CAMetalLayer class], @selector(g1_originalDrawable));
    id<MTLDrawable> drawable = ((id (*)(id, SEL))original)(layer, selector);
    BOOL ready = atomic_load(&completeScene);
    if (drawable) [drawable addPresentedHandler:^(id<MTLDrawable> shown) {
        double time = shown.presentedTime;
        if (time <= 0) return;
        pthread_mutex_lock(&lock);
        if (!firstDrawable) firstDrawable = time;
        if (ready && !firstComplete) firstComplete = time;
        [presented addObject:@(time)];
        pthread_mutex_unlock(&lock);
    }];
    return drawable;
}

static NSDictionary *usage(void) {
    struct rusage_info_v4 r = {0};
    if (proc_pid_rusage(getpid(), RUSAGE_INFO_V4, (rusage_info_t *)&r)) abort();
    thread_basic_info_data_t thread;
    mach_msg_type_number_t count = THREAD_BASIC_INFO_COUNT;
    if (thread_info(mainThread, THREAD_BASIC_INFO, (thread_info_t)&thread, &count) != KERN_SUCCESS) abort();
    double main = thread.user_time.seconds + thread.system_time.seconds
        + (thread.user_time.microseconds + thread.system_time.microseconds) / 1e6;
    return @{@"time": @(CACurrentMediaTime()), @"cpu": @((r.ri_user_time + r.ri_system_time) / 1e9),
             @"main": @(main), @"footprint": @(r.ri_phys_footprint), @"lifetimePeak": @(r.ri_lifetime_max_phys_footprint)};
}

static void writeJSON(NSDictionary *data, NSString *path) {
    NSError *error;
    NSData *bytes = [NSJSONSerialization dataWithJSONObject:data options:NSJSONWritingPrettyPrinted error:&error];
    if (!bytes || ![bytes writeToFile:path options:NSDataWritingAtomic error:&error]) {
        fprintf(stderr, "probe write: %s\n", error.description.UTF8String);
        exit(2);
    }
}

static NSScrollView *scrollView(NSView *root) {
    NSScrollView *largest = [root isKindOfClass:NSScrollView.class] ? (NSScrollView *)root : nil;
    for (NSView *child in root.subviews) {
        NSScrollView *found = scrollView(child);
        if (found.bounds.size.height > largest.bounds.size.height) largest = found;
    }
    return largest;
}

// A small layer snapshot observes cached pixels without asking AppKit to draw.
static NSData *signature(NSView *view) {
    const size_t side = 100;
    NSMutableData *bytes = [NSMutableData dataWithLength:side * side * 4];
    CGColorSpaceRef color = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef context = CGBitmapContextCreate(bytes.mutableBytes, side, side, 8, side * 4,
        color, kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big);
    CGContextSetRGBFillColor(context, 1, 1, 1, 1);
    CGContextFillRect(context, CGRectMake(0, 0, side, side));
    CGContextScaleCTM(context, side / view.bounds.size.width, side / view.bounds.size.height);
    [(view.layer.presentationLayer ?: view.layer) renderInContext:context];
    CGContextRelease(context);
    CGColorSpaceRelease(color);
    return bytes;
}

@interface G1Probe : NSObject
@property NSWindow *window;
@property CADisplayLink *link;
@property NSMutableArray *ticks;
@property NSMutableArray *coldPixels;
@property NSMutableArray *coldTimes;
@property NSMutableDictionary *result;
@property NSDictionary *start;
@property uint64_t peak;
@property int phase;
@property double windowAt;
@property dispatch_source_t signals;
@property dispatch_source_t memory;
@property id wheelMonitor;
@property NSUInteger wheels;
@property double wheelDelta;
@end

@implementation G1Probe
- (void)attach {
    for (NSWindow *window in NSApp.windows) {
        if (window.visible && window.contentView.bounds.size.height > 400) { self.window = window; break; }
    }
    if (!self.window) {
        if (CACurrentMediaTime() - processStart > 30) { fprintf(stderr, "probe: no window\n"); exit(2); }
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 1000000), dispatch_get_main_queue(), ^{ [self attach]; });
        return;
    }
    self.windowAt = CACurrentMediaTime();
    self.ticks = [NSMutableArray array];
    self.coldPixels = [NSMutableArray array];
    self.coldTimes = [NSMutableArray array];
    self.result = [NSMutableDictionary dictionary];
    self.link = [self.window.contentView displayLinkWithTarget:self selector:@selector(tick:)];
    float hz = self.window.screen.maximumFramesPerSecond;
    self.link.preferredFrameRateRange = CAFrameRateRangeMake(hz, hz, hz);
    [self.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    self.wheelMonitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskScrollWheel handler:^NSEvent *(NSEvent *event) {
        if (self.phase == 1) { self.wheels++; self.wheelDelta += event.scrollingDeltaY; }
        return event;
    }];
    signal(SIGUSR2, SIG_IGN);
    self.signals = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGUSR2, 0, dispatch_get_main_queue());
    dispatch_source_set_event_handler(self.signals, ^{ [self advance]; });
    dispatch_resume(self.signals);
}
- (void)tick:(CADisplayLink *)link {
    double now = CACurrentMediaTime();
    if (self.phase == 0) {
        if (!gpui) {
            [self.coldPixels addObject:signature(self.window.contentView)];
            [self.coldTimes addObject:@(link.timestamp)];
        }
        pthread_mutex_lock(&lock);
        BOOL ready = firstComplete > 0;
        pthread_mutex_unlock(&lock);
        if (now - self.windowAt >= 2 && (!gpui || ready)) [self ready];
        if (now - processStart > 30) { fprintf(stderr, "probe: first complete frame timed out\n"); exit(2); }
    } else if (self.phase == 1) {
        [self.ticks addObject:@{@"timestamp": @(link.timestamp), @"delivered": @(now),
            @"period": @(link.targetTimestamp - link.timestamp)}];
        self.peak = MAX(self.peak, [usage()[@"footprint"] unsignedLongLongValue]);
    }
}
- (void)ready {
    if (!gpui) {
        NSData *final = self.coldPixels.lastObject;
        const unsigned char *b = final.bytes;
        NSUInteger ink = 0;
        for (NSUInteger i = 0; i < final.length; i += 4) if (b[i] + b[i+1] + b[i+2] < 690) ink++;
        if (ink < 100 || !scrollView(self.window.contentView)) {
            fprintf(stderr, "probe: native list has no ink or scroll view\n"); exit(2);
        }
        for (NSUInteger sample = 0; sample < self.coldPixels.count; sample++) {
            const unsigned char *a = [self.coldPixels[sample] bytes];
            uint64_t difference = 0;
            for (NSUInteger i = 0; i < final.length; i++) difference += abs((int)a[i] - b[i]);
            if ((double)difference / final.length <= 1.0) { firstComplete = [self.coldTimes[sample] doubleValue]; break; }
        }
        self.result[@"coldCriterion"] = @"first display-link snapshot within mean 1/255 of the settled 100x100 native layer snapshot at 2 s";
    } else {
        self.result[@"coldCriterion"] = @"MTLDrawable.presentedTime for the first scene with all requested images loaded";
    }
    self.result[@"coldMs"] = @((firstComplete - processStart) * 1000);
    self.result[@"preMainMs"] = @((loaded - processStart) * 1000);
    if (firstDrawable) self.result[@"firstDrawableMs"] = @((firstDrawable - processStart) * 1000);
    self.result[@"arm"] = gpui ? @"gpui" : @"native";
    self.result[@"pid"] = @(getpid());
    self.result[@"scale"] = @(self.window.backingScaleFactor);
    self.result[@"size"] = @[@(self.window.contentView.bounds.size.width), @(self.window.contentView.bounds.size.height)];
    self.result[@"thermalStart"] = @(NSProcessInfo.processInfo.thermalState);
    self.result[@"lowPower"] = @(NSProcessInfo.processInfo.lowPowerModeEnabled);
    self.result[@"screenHz"] = @(self.window.screen.maximumFramesPerSecond);
    self.coldPixels = nil;
    self.coldTimes = nil;
    self.link.paused = YES;
    self.phase = -1;
    NSPoint point = NSMakePoint(400, 400);
    NSPoint screen = [self.window convertPointToScreen:point];
    writeJSON(@{@"pid": @(getpid()), @"window": @(self.window.windowNumber),
        @"point": @[@(point.x), @(self.window.frame.size.height - point.y)],
        @"screen": @[@(screen.x), @(NSScreen.screens.firstObject.frame.size.height - screen.y)],
        @"mark": @11490401, @"form": @"fling", @"wheel": @[@0, @600]},
        [output stringByAppendingString:@".ready"]);
}
- (void)advance {
    if (self.phase == -1) {
        self.start = usage(); self.peak = [self.start[@"footprint"] unsignedLongLongValue];
        self.phase = 1; self.link.paused = NO;
        writeJSON(self.start, [output stringByAppendingString:@".fling"]);
    } else if (self.phase == 1) {
        NSDictionary *end = usage();
        self.result[@"fling"] = @{ @"start": self.start, @"end": end, @"peak": @(self.peak), @"displayLink": self.ticks,
            @"wheelEvents": @(self.wheels), @"wheelDeltaY": @(self.wheelDelta) };
        pthread_mutex_lock(&lock);
        self.result[@"presented"] = [presented copy];
        pthread_mutex_unlock(&lock);
        self.ticks = nil; self.link.paused = YES;
        NSScrollView *scroll = scrollView(self.window.contentView);
        if (scroll) self.result[@"nativeOffsetEnd"] = @(scroll.contentView.bounds.origin.y);
        self.start = usage(); self.peak = [self.start[@"footprint"] unsignedLongLongValue];
        self.phase = 2;
        self.memory = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
        dispatch_source_set_timer(self.memory, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), NSEC_PER_SEC, NSEC_PER_MSEC);
        dispatch_source_set_event_handler(self.memory, ^{ self.peak = MAX(self.peak, [usage()[@"footprint"] unsignedLongLongValue]); });
        dispatch_resume(self.memory);
        writeJSON(self.start, [output stringByAppendingString:@".rest"]);
    } else if (self.phase == 2) {
        self.result[@"rest"] = @{ @"start": self.start, @"end": usage(), @"peak": @(self.peak) };
        self.result[@"thermalEnd"] = @(NSProcessInfo.processInfo.thermalState);
        writeJSON(self.result, output);
        exit(0);
    }
}
@end

__attribute__((constructor)) static void initialize(void) {
    @autoreleasepool {
        const char *path = getenv("G1_OUTPUT");
        if (!path) return;
        loaded = CACurrentMediaTime();
        output = @(path);
        gpui = strcmp(getenv("G1_ARM") ?: "", "gpui") == 0;
        mainThread = mach_thread_self();
        struct proc_bsdinfo info;
        if (proc_pidinfo(getpid(), PROC_PIDTBSDINFO, 0, &info, sizeof(info)) != sizeof(info)) abort();
        struct timeval wall; gettimeofday(&wall, NULL);
        processStart = loaded - ((double)wall.tv_sec + wall.tv_usec / 1e6 - info.pbi_start_tvsec - info.pbi_start_tvusec / 1e6);
        presented = [NSMutableArray array];
        if (gpui) {
            Method method = class_getInstanceMethod(CAMetalLayer.class, @selector(nextDrawable));
            class_addMethod(CAMetalLayer.class, @selector(g1_originalDrawable), method_getImplementation(method), method_getTypeEncoding(method));
            method_setImplementation(method, (IMP)nextDrawable);
        }
        dispatch_async(dispatch_get_main_queue(), ^{ static G1Probe *probe; probe = [G1Probe new]; [probe attach]; });
    }
}
