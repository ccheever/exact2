// Linked into the native, GPUI, ink and egui benchmark copies. @ref LLP 1149 §4.1.
#include <assert.h>
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <Metal/Metal.h>
#import <objc/runtime.h>
#include <libproc.h>
#include <mach/mach.h>
#include <mach/mach_time.h>
#include <mach/task_policy.h>
#include <pthread.h>
#include <stdatomic.h>
#include <dlfcn.h>
#include <sys/resource.h>
#include <sys/sysctl.h>
#include <mach-o/dyld.h>

static double processStart, loaded, firstDrawable, firstComplete;
static BOOL tracing;
static struct rusage constructorUsage;
static struct rusage_info_v4 constructorRusage;
static struct rusage_info_v4 mainRusage;
static struct rusage completeUsage;
static struct rusage_info_v4 completeRusage;
static double completeObserved;
static struct proc_bsdinfo processInfo;
void g1_interval(const char *name, double start, double end, double value);
static double firstCpu, firstInk;
static double firstCpuComplete, firstInkComplete;
static void (^bootExpose)(void);
void g1_boot_expose(void (^callback)(void));
static void firstBootTick(void) {
    if (bootExpose) {
        void (^callback)(void) = bootExpose;
        bootExpose = nil;
        callback();
    }
}
static atomic_int painterKind = 1;
static atomic_bool cpuSubmitted;
void g1_painter(int ink) { atomic_store(&painterKind, ink); }
static mach_port_t mainThread;
static atomic_bool completeScene;
static atomic_uint drawableCalls, mainThreadDrawableCalls, drawableCount;
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static NSMutableArray<NSNumber *> *presented;
static NSString *output;
static BOOL painted;
static NSString *arm;
static double tickSeconds;
static atomic_size_t topItem;
static _Atomic(float) topOffset;
static NSString *workload;

// Charge a buffer to its commit phase, even if completion arrives in the next one.
static pthread_mutex_t gpuLock = PTHREAD_MUTEX_INITIALIZER;
static struct { double seconds; uint64_t committed, completed, invalid; } gpu[3];
static int gpuPhase;
static BOOL gpuHookEnabled;
static NSString *queueClass, *bufferClass;
static IMP originalCommit, originalBuffer, originalUnretainedBuffer, originalDescriptorBuffer;
static _Thread_local double lastCommitEnd;

static void commitBuffer(id<MTLCommandBuffer> buffer, SEL selector) {
    double start = tracing ? CACurrentMediaTime() : 0;
    if (!gpuHookEnabled) {
        ((void (*)(id, SEL))originalCommit)(buffer, selector);
    } else {
        pthread_mutex_lock(&gpuLock);
        int phase = gpuPhase;
        gpu[phase].committed++;
        pthread_mutex_unlock(&gpuLock);
        [buffer addCompletedHandler:^(id<MTLCommandBuffer> done) {
            double start = done.GPUStartTime, end = done.GPUEndTime;
            pthread_mutex_lock(&gpuLock);
            gpu[phase].completed++;
            if (done.status == MTLCommandBufferStatusCompleted && start > 0 && end >= start)
                gpu[phase].seconds += end - start;
            else gpu[phase].invalid++;
            pthread_mutex_unlock(&gpuLock);
        }];
        ((void (*)(id, SEL))originalCommit)(buffer, selector);
    }
    if (tracing) {
        lastCommitEnd = CACurrentMediaTime();
        g1_interval(objc_getAssociatedObject(buffer, @selector(g1_present))
            ? "probe present commit" : "probe command commit", start, lastCommitEnd, 0);
    }
}

static void presentDrawable(id buffer, SEL selector, id drawable) {
    objc_setAssociatedObject(drawable, @selector(g1_bufferPresent), @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    objc_setAssociatedObject(buffer, @selector(g1_present), @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    IMP original = class_getMethodImplementation(object_getClass(buffer), @selector(g1_presentDrawable:));
    ((void (*)(id, SEL, id))original)(buffer, selector, drawable);
}
static void presentDrawableAtTime(id buffer, SEL selector, id drawable, double time) {
    objc_setAssociatedObject(drawable, @selector(g1_bufferPresent), @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    objc_setAssociatedObject(buffer, @selector(g1_present), @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    SEL saved = selector == @selector(presentDrawable:atTime:)
        ? @selector(g1_presentDrawable:atTime:) : @selector(g1_presentDrawable:afterMinimumDuration:);
    IMP original = class_getMethodImplementation(object_getClass(buffer), saved);
    ((void (*)(id, SEL, id, double))original)(buffer, selector, drawable, time);
}

static id watchBuffer(id buffer) {
    if (!buffer) return nil;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        Class concrete = object_getClass(buffer);
        Method method = class_getInstanceMethod(concrete, @selector(commit));
        originalCommit = method_getImplementation(method);
        pthread_mutex_lock(&gpuLock);
        bufferClass = NSStringFromClass(concrete);
        pthread_mutex_unlock(&gpuLock);
        class_replaceMethod(concrete, @selector(commit), (IMP)commitBuffer, method_getTypeEncoding(method));
        if (tracing) {
            SEL selectors[] = {@selector(presentDrawable:), @selector(presentDrawable:atTime:), @selector(presentDrawable:afterMinimumDuration:)};
            SEL saved[] = {@selector(g1_presentDrawable:), @selector(g1_presentDrawable:atTime:), @selector(g1_presentDrawable:afterMinimumDuration:)};
            for (int i = 0; i < 3; i++) {
                Method present = class_getInstanceMethod(concrete, selectors[i]);
                if (!present) continue;
                class_addMethod(concrete, saved[i], method_getImplementation(present), method_getTypeEncoding(present));
                class_replaceMethod(concrete, selectors[i], i ? (IMP)presentDrawableAtTime : (IMP)presentDrawable, method_getTypeEncoding(present));
            }
        }
    });
    if (![buffer isKindOfClass:NSClassFromString(bufferClass)]) {
        fprintf(stderr, "probe: uninstrumented command-buffer class %s\n", object_getClassName(buffer));
        abort();
    }
    return buffer;
}

static id commandBuffer(id queue, SEL selector) {
    return watchBuffer(((id (*)(id, SEL))originalBuffer)(queue, selector));
}
static id unretainedCommandBuffer(id queue, SEL selector) {
    return watchBuffer(((id (*)(id, SEL))originalUnretainedBuffer)(queue, selector));
}
static id descriptorCommandBuffer(id queue, SEL selector, id descriptor) {
    return watchBuffer(((id (*)(id, SEL, id))originalDescriptorBuffer)(queue, selector, descriptor));
}

static void watchQueue(id<MTLCommandQueue> queue) {
    if (!queue) return;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        Class concrete = object_getClass(queue);
        queueClass = NSStringFromClass(concrete);
        SEL selectors[] = {@selector(commandBuffer), @selector(commandBufferWithUnretainedReferences), @selector(commandBufferWithDescriptor:)};
        IMP replacements[] = {(IMP)commandBuffer, (IMP)unretainedCommandBuffer, (IMP)descriptorCommandBuffer};
        IMP *originals[] = {&originalBuffer, &originalUnretainedBuffer, &originalDescriptorBuffer};
        for (int i = 0; i < 3; i++) {
            Method method = class_getInstanceMethod(concrete, selectors[i]);
            if (!method) { fprintf(stderr, "probe: missing queue method\n"); abort(); }
            *originals[i] = method_getImplementation(method);
            class_replaceMethod(concrete, selectors[i], replacements[i], method_getTypeEncoding(method));
        }
    });
    if (![queue isKindOfClass:NSClassFromString(queueClass)]) abort();
}
typedef id (*RetainedQueue)(id, SEL) __attribute__((ns_returns_retained));
typedef id (*RetainedQueueLimit)(id, SEL, NSUInteger) __attribute__((ns_returns_retained));
static id newQueue(id device, SEL selector) NS_RETURNS_RETAINED;
static id newQueue(id device, SEL selector) {
    IMP original = class_getMethodImplementation(object_getClass(device), @selector(g1_newCommandQueue));
    id queue = ((RetainedQueue)original)(device, selector);
    watchQueue(queue);
    return queue;
}
static id newQueueWithLimit(id device, SEL selector, NSUInteger count) NS_RETURNS_RETAINED;
static id newQueueWithLimit(id device, SEL selector, NSUInteger count) {
    IMP original = class_getMethodImplementation(object_getClass(device), @selector(g1_newCommandQueueWithMaxCommandBufferCount:));
    id queue = ((RetainedQueueLimit)original)(device, selector, count);
    watchQueue(queue);
    return queue;
}
static void watchDevice(id<MTLDevice> device) {
    if (!device) return;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        Class concrete = object_getClass(device);
        SEL selectors[] = {@selector(newCommandQueue), @selector(newCommandQueueWithMaxCommandBufferCount:)};
        SEL saved[] = {@selector(g1_newCommandQueue), @selector(g1_newCommandQueueWithMaxCommandBufferCount:)};
        IMP replacements[] = {(IMP)newQueue, (IMP)newQueueWithLimit};
        for (int i = 0; i < 2; i++) {
            Method method = class_getInstanceMethod(concrete, selectors[i]);
            if (!method) abort();
            class_addMethod(concrete, saved[i], method_getImplementation(method), method_getTypeEncoding(method));
            class_replaceMethod(concrete, selectors[i], replacements[i], method_getTypeEncoding(method));
        }
    });
}
// Observe the application's device calls; never create one to discover its class.
static id<MTLDevice> traceDevice(void) NS_RETURNS_RETAINED;
static id<MTLDevice> traceDevice(void) {
    double start = tracing ? CACurrentMediaTime() : 0;
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    if (tracing) {
        watchDevice(device);
        g1_interval("probe device creation", start, CACurrentMediaTime(), 0);
    }
    return device;
}
static NSArray<id<MTLDevice>> *traceDevices(void) NS_RETURNS_RETAINED;
static NSArray<id<MTLDevice>> *traceDevices(void) {
    double start = tracing ? CACurrentMediaTime() : 0;
    NSArray<id<MTLDevice>> *devices = MTLCopyAllDevices();
    if (tracing) {
        for (id<MTLDevice> device in devices) watchDevice(device);
        g1_interval("probe enumerate devices", start, CACurrentMediaTime(), 0);
    }
    return devices;
}
__attribute__((used, section("__DATA,__interpose")))
static const struct { const void *replacement, *original; } deviceObservers[] = {
    {(const void *)traceDevice, (const void *)MTLCreateSystemDefaultDevice},
    {(const void *)traceDevices, (const void *)MTLCopyAllDevices},
};
static void watchCommandQueues(void) {
    id<MTLCommandQueue> queue = [MTLCreateSystemDefaultDevice() newCommandQueue];
    if (!queue) { fprintf(stderr, "probe: no Metal queue\n"); abort(); }
    watchQueue(queue);
}

static void configureGPUHook(void) {
    const char *setting = getenv("G1_GPU_HOOK");
    if (setting && strcmp(setting, "0") && strcmp(setting, "1")) {
        fprintf(stderr, "probe: G1_GPU_HOOK must be 0 or 1\n"); exit(2);
    }
    gpuHookEnabled = setting && !strcmp(setting, "1");
    if (gpuHookEnabled) watchCommandQueues();
}

static void setGPUPhase(int phase) {
    pthread_mutex_lock(&gpuLock);
    gpuPhase = phase;
    pthread_mutex_unlock(&gpuLock);
}

static NSDictionary *gpuUsage(int phase) {
    pthread_mutex_lock(&gpuLock);
    typeof(gpu[0]) value = gpu[phase];
    NSDictionary *result = @{ @"seconds": @(value.seconds), @"committed": @(value.committed),
        @"completed": @(value.completed), @"invalid": @(value.invalid),
        @"queueClass": queueClass ?: @"", @"bufferClass": bufferClass ?: @"" };
    pthread_mutex_unlock(&gpuLock);
    return result;
}

// Opt-in fixed storage; serializing a trace happens after the measured scroll.
static struct { const char *name, *caller; double start, end, value; mach_port_t thread; } intervals[131072];
static size_t intervalCount, intervalDropped;
static unsigned traceSaves;
static BOOL dispatchTracing;
static BOOL wakeTracing;
static pthread_mutex_t traceLock = PTHREAD_MUTEX_INITIALIZER;
static const char *(*counterName)(size_t);
static uint64_t (*counterValue)(size_t, bool);
int g1_present_clear(void) { return getenv("G1_PRESENT_CLEAR") != NULL; }
void g1_set_counter_reader(const char *(*name)(size_t), uint64_t (*value)(size_t, bool)) {
    counterName = name; counterValue = value;
}

// Live socket diagnostics must not activate the app through winit's launch path.
static void backgroundActivate(id object, SEL selector) {}
static void backgroundActivateIgnoring(id object, SEL selector, BOOL ignoring) {}
static void backgroundOrder(id window, SEL selector, id sender) {
    [(NSWindow *)window orderFront:sender];
}
static void interval(const char *name, double start, double end, double value, const char *caller) {
    if (!tracing) return;
    pthread_mutex_lock(&traceLock);
    if (intervalCount < sizeof(intervals) / sizeof(intervals[0])) {
        intervals[intervalCount++] = (typeof(intervals[0])){name, caller, start, end, value, pthread_mach_thread_np(pthread_self())};
    } else intervalDropped++;
    pthread_mutex_unlock(&traceLock);
}
void g1_interval(const char *name, double start, double end, double value) {
    if (tracing && !strcmp(name, "boot main") &&
        proc_pid_rusage(getpid(), RUSAGE_INFO_V4, (rusage_info_t *)&mainRusage)) abort();
    interval(name, start, end, value, NULL);
}
void g1_startup_mark(const char *name) {
    if (!tracing) return;
    double now = CACurrentMediaTime();
    g1_interval(name, now, now, 0);
}
static const char *callerName(void *address) {
    Dl_info info;
    return dladdr(address, &info) ? (info.dli_sname ?: info.dli_fname) : "unresolved";
}

// Observe wake requests without changing their delivery or the timer deadline.
static void wakeLoop(CFRunLoopRef loop) {
    double start = wakeTracing ? CACurrentMediaTime() : 0;
    CFRunLoopWakeUp(loop);
    if (wakeTracing) interval("run loop wake request", start, CACurrentMediaTime(), loop == CFRunLoopGetMain(), callerName(__builtin_return_address(0)));
}
static void armTimer(CFRunLoopTimerRef timer, CFAbsoluteTime date) {
    double start = wakeTracing ? CACurrentMediaTime() : 0;
    CFRunLoopTimerSetNextFireDate(timer, date);
    if (wakeTracing) {
        double delay = date - CFAbsoluteTimeGetCurrent();
        // Winit uses extreme dates to stop or immediately fire its timer.
        interval("run loop timer arm", start, CACurrentMediaTime(), fmax(-1e12, fmin(1e12, delay)) * 1000, callerName(__builtin_return_address(0)));
    }
}
__attribute__((used, section("__DATA,__interpose")))
static const struct { const void *replacement, *original; } wakeObservers[] = {
    {(const void *)wakeLoop, (const void *)CFRunLoopWakeUp},
    {(const void *)armTimer, (const void *)CFRunLoopTimerSetNextFireDate},
};

void g1_boot_expose(void (^callback)(void)) {
    bootExpose = [callback copy];
    if (tracing && !CATransaction.completionBlock) {
        [CATransaction setCompletionBlock:^{
            double now = CACurrentMediaTime();
            g1_interval("layer first transaction completed", now, now, 0);
        }];
    }
}

static void sendEvent(id app, SEL selector, NSEvent *event) {
    double start = CACurrentMediaTime();
    IMP original = class_getMethodImplementation(NSApplication.class, @selector(g1_originalSendEvent:));
    ((void (*)(id, SEL, NSEvent *))original)(app, selector, event);
    g1_interval("AppKit event delivery", start, CACurrentMediaTime(), event.type);
}

static void observeRunLoop(void) {
    static double active[32];
    static unsigned depth;
    CFRunLoopObserverRef observer = CFRunLoopObserverCreateWithHandler(NULL,
        kCFRunLoopEntry | kCFRunLoopExit | kCFRunLoopAfterWaiting | kCFRunLoopBeforeWaiting,
        true, 0, ^(CFRunLoopObserverRef o, CFRunLoopActivity activity) {
            double now = CACurrentMediaTime();
            if (activity == kCFRunLoopEntry) {
                if (depth && active[depth - 1]) {
                    g1_interval("run loop dispatch", active[depth - 1], now, depth);
                    active[depth - 1] = 0;
                }
                if (depth == 32) abort();
                active[depth++] = now;
            } else if (activity == kCFRunLoopAfterWaiting && depth) {
                active[depth - 1] = now;
                g1_interval("run loop wake", now, now, depth);
            } else if (depth) {
                if (active[depth - 1]) g1_interval("run loop dispatch", active[depth - 1], now, depth);
                active[depth - 1] = 0;
                if (activity == kCFRunLoopExit) {
                    depth--;
                    if (depth) active[depth - 1] = now;
                }
            }
        });
    CFRunLoopAddObserver(CFRunLoopGetMain(), observer, kCFRunLoopCommonModes);
    CFRelease(observer);
}

void g1_ready(int complete, size_t item, float offset) {
    g1_interval("scene ready", CACurrentMediaTime(), CACurrentMediaTime(), complete);
    atomic_store(&completeScene, complete != 0);
    topItem = item;
    topOffset = offset;
}

// winit activates NSApplication even with activateIgnoringOtherApps=false, and
// set_visible(true) makes the window key. Boot-only measurements suppress both.
static atomic_uint activationSuppressed, keySuppressed;
static void passiveActivate(id app, SEL selector) { atomic_fetch_add(&activationSuppressed, 1); }
static void passiveActivateIgnoring(id app, SEL selector, BOOL ignoring) { passiveActivate(app, selector); }
static void passiveKey(id window, SEL selector) { atomic_fetch_add(&keySuppressed, 1); }
static void passiveOrderFront(id window, SEL selector, id sender) {
    atomic_fetch_add(&keySuppressed, 1);
    [(NSWindow *)window orderFrontRegardless];
}
static void suppressBootFocus(void) {
    method_setImplementation(class_getInstanceMethod(NSApplication.class, @selector(activate)), (IMP)passiveActivate);
    method_setImplementation(class_getInstanceMethod(NSApplication.class, @selector(activateIgnoringOtherApps:)), (IMP)passiveActivateIgnoring);
    method_setImplementation(class_getInstanceMethod(NSWindow.class, @selector(makeKeyAndOrderFront:)), (IMP)passiveOrderFront);
    method_setImplementation(class_getInstanceMethod(NSWindow.class, @selector(makeKeyWindow)), (IMP)passiveKey);
    method_setImplementation(class_getInstanceMethod(NSWindow.class, @selector(makeMainWindow)), (IMP)passiveKey);
    if (![[NSApplication sharedApplication] setActivationPolicy:NSApplicationActivationPolicyAccessory]) {
        fprintf(stderr, "probe: passive activation policy refused\n"); exit(2);
    }
}

static void recordPresented(double time, BOOL ready, int painter) {
    if (time <= 0) return;
    g1_interval("probe presented", time, time, ready);
    pthread_mutex_lock(&lock);
    if ((firstDrawable == 0 || time < firstDrawable)) firstDrawable = time;
    if (painter && (firstInk == 0 || time < firstInk)) firstInk = time;
    if (!painter && (firstCpu == 0 || time < firstCpu)) firstCpu = time;
    if (ready && (firstComplete == 0 || time < firstComplete)) {
        firstComplete = time;
        if (tracing) {
            completeObserved = CACurrentMediaTime();
            if (getrusage(RUSAGE_SELF, &completeUsage) ||
                proc_pid_rusage(getpid(), RUSAGE_INFO_V4, (rusage_info_t *)&completeRusage)) abort();
        }
    }
    if (ready && painter && (firstInkComplete == 0 || time < firstInkComplete)) firstInkComplete = time;
    if (ready && !painter && (firstCpuComplete == 0 || time < firstCpuComplete)) firstCpuComplete = time;
    [presented addObject:@(time)];
    pthread_mutex_unlock(&lock);
}

// GPUI's transaction path commits, waits until scheduled, then presents directly.
// Retain both events so that the same-thread preceding-commit join is inspectable.
static void directPresentMark(id drawable) {
    if (objc_getAssociatedObject(drawable, @selector(g1_bufferPresent))) return;
    double now = CACurrentMediaTime();
    g1_interval("probe direct present request", now, now, 0);
    if (lastCommitEnd > 0 && lastCommitEnd <= now)
        g1_interval("probe direct present preceding commit", lastCommitEnd, lastCommitEnd, 0);
}
static void directPresent(id drawable, SEL selector) {
    directPresentMark(drawable);
    IMP original = class_getMethodImplementation(object_getClass(drawable), @selector(g1_directPresent));
    ((void (*)(id, SEL))original)(drawable, selector);
}
static void directPresentAtTime(id drawable, SEL selector, double time) {
    directPresentMark(drawable);
    SEL saved = selector == @selector(presentAtTime:)
        ? @selector(g1_directPresentAtTime:) : @selector(g1_directPresentAfterMinimumDuration:);
    IMP original = class_getMethodImplementation(object_getClass(drawable), saved);
    ((void (*)(id, SEL, double))original)(drawable, selector, time);
}
static void watchDrawable(id drawable) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        Class concrete = object_getClass(drawable);
        SEL selectors[] = {@selector(present), @selector(presentAtTime:), @selector(presentAfterMinimumDuration:)};
        SEL saved[] = {@selector(g1_directPresent), @selector(g1_directPresentAtTime:), @selector(g1_directPresentAfterMinimumDuration:)};
        for (int i = 0; i < 3; i++) {
            Method method = class_getInstanceMethod(concrete, selectors[i]);
            if (!method) continue;
            class_addMethod(concrete, saved[i], method_getImplementation(method), method_getTypeEncoding(method));
            class_replaceMethod(concrete, selectors[i], i ? (IMP)directPresentAtTime : (IMP)directPresent, method_getTypeEncoding(method));
        }
    });
    // Metal can reuse a drawable object after its previous presentation.
    objc_setAssociatedObject(drawable, @selector(g1_bufferPresent), nil, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
}

static id nextDrawable(id layer, SEL selector) {
    atomic_fetch_add(&drawableCalls, 1);
    if (NSThread.isMainThread) atomic_fetch_add(&mainThreadDrawableCalls, 1);
    atomic_store(&drawableCount, (unsigned)[(CAMetalLayer *)layer maximumDrawableCount]);
    static IMP original;
    if (!original) original = class_getMethodImplementation([CAMetalLayer class], @selector(g1_originalDrawable));
    id<MTLDrawable> drawable = ((id (*)(id, SEL))original)(layer, selector);
    if (drawable && tracing) watchDrawable(drawable);
    int painter = atomic_load(&painterKind);
    if (!painter) atomic_store(&cpuSubmitted, true);
    BOOL ready = atomic_load(&completeScene) || getenv("G1_ALLOW_IMAGE_REFUSALS");
    if (drawable) [drawable addPresentedHandler:^(id<MTLDrawable> shown) {
        recordPresented(shown.presentedTime, ready, painter);
    }];
    return drawable;
}

static NSDictionary *usage(void) {
    double began = dispatchTracing ? CACurrentMediaTime() : 0;
    struct rusage_info_v4 r = {0};
    if (proc_pid_rusage(getpid(), RUSAGE_INFO_V4, (rusage_info_t *)&r)) abort();
    thread_basic_info_data_t thread;
    mach_msg_type_number_t count = THREAD_BASIC_INFO_COUNT;
    if (thread_info(mainThread, THREAD_BASIC_INFO, (thread_info_t)&thread, &count) != KERN_SUCCESS) abort();
    double main = thread.user_time.seconds + thread.system_time.seconds
        + (thread.user_time.microseconds + thread.system_time.microseconds) / 1e6;
    NSMutableDictionary *sample = [@{@"time": @(CACurrentMediaTime()), @"cpu": @((r.ri_user_time + r.ri_system_time) * tickSeconds),
             @"main": @(main), @"cycles": @(r.ri_cycles), @"instructions": @(r.ri_instructions),
             @"pageins": @(r.ri_pageins), @"footprint": @(r.ri_phys_footprint), @"lifetimePeak": @(r.ri_lifetime_max_phys_footprint)} mutableCopy];
    if (getenv("G1_CLOCK_DIAGNOSTIC")) {
        // XNU's thread_selfcounts kind 4 reports Mach time by hw.perflevel index.
        // This diagnostic is deliberately absent from ordinary timing samples.
        struct { uint64_t instructions, cycles, user, system; } levels[8] = {0};
        unsigned n = 0; size_t size = sizeof(n);
        int (*counts)(int, void *, size_t) = dlsym(RTLD_DEFAULT, "thread_selfcounts");
        if (!pthread_main_np() || sysctlbyname("hw.nperflevels", &n, &size, NULL, 0)
            || !n || n > 8 || !counts || counts(4, levels, n * sizeof(levels[0]))) abort();
        NSMutableArray *perf = [NSMutableArray array];
        for (unsigned i = 0; i < n; i++) {
            char key[64], name[64]; size = sizeof(name);
            snprintf(key, sizeof(key), "hw.perflevel%u.name", i);
            if (sysctlbyname(key, name, &size, NULL, 0)) abort();
            [perf addObject:@{@"name": @(name), @"cpu": @((levels[i].user + levels[i].system) * tickSeconds),
                @"cycles": @(levels[i].cycles), @"instructions": @(levels[i].instructions)}];
        }
        sample[@"mainPerf"] = perf;
        sample[@"mainRequestedQos"] = @(qos_class_self());
        task_category_policy_data_t category = {0};
        mach_msg_type_number_t count = TASK_CATEGORY_POLICY_COUNT;
        boolean_t defaults = false;
        kern_return_t roleStatus = task_policy_get(mach_task_self(), TASK_CATEGORY_POLICY,
            (task_policy_t)&category, &count, &defaults);
        // Diagnostic-only XNU task_policy_state ABI: retain raw bits as well as
        // the current effective-policy role/clamp; this is not a runtime policy.
        uint64_t policy[8] = {0}; count = sizeof(policy) / (sizeof(integer_t)); defaults = false;
        kern_return_t policyStatus = task_policy_get(mach_task_self(), TASK_POLICY_STATE,
            (task_policy_t)policy, &count, &defaults);
        sample[@"taskPolicy"] = @{@"roleStatus": @(roleStatus),
            @"role": roleStatus == KERN_SUCCESS ? @(category.role) : NSNull.null,
            @"stateStatus": @(policyStatus),
            @"effectiveRaw": policyStatus == KERN_SUCCESS ? [NSString stringWithFormat:@"%016llx", policy[7]] : NSNull.null,
            @"effectiveRole": policyStatus == KERN_SUCCESS ? @((policy[7] >> 20) & 15) : NSNull.null,
            @"qosClamp": policyStatus == KERN_SUCCESS ? @((policy[7] >> 27) & 7) : NSNull.null,
            @"darwinBackground": policyStatus == KERN_SUCCESS ? @((policy[7] & 1) != 0) : NSNull.null};
        sample[@"processQos"] = @{
            @"default": @(r.ri_cpu_time_qos_default * tickSeconds),
            @"maintenance": @(r.ri_cpu_time_qos_maintenance * tickSeconds),
            @"background": @(r.ri_cpu_time_qos_background * tickSeconds),
            @"utility": @(r.ri_cpu_time_qos_utility * tickSeconds),
            @"legacy": @(r.ri_cpu_time_qos_legacy * tickSeconds),
            @"userInitiated": @(r.ri_cpu_time_qos_user_initiated * tickSeconds),
            @"userInteractive": @(r.ri_cpu_time_qos_user_interactive * tickSeconds)};
        sample[@"forcedInteractiveDiagnostic"] = @(getenv("G1_CLOCK_QOS") != NULL);
    }
    if (counterName && counterValue) {
        NSMutableDictionary *values = [NSMutableDictionary dictionary], *high = [NSMutableDictionary dictionary];
        size_t i = 0;
        for (; counterName(i); i++) {
            NSString *name = @(counterName(i));
            uint64_t value = counterValue(i, false), peak = counterValue(i, true);
            values[name] = value == UINT64_MAX ? NSNull.null : @(value);
            high[name] = peak == UINT64_MAX ? NSNull.null : @(peak);
        }
        sample[@"counters"] = @{@"values": values, @"highWater": high, @"displayedFrames": values[@"displayed_frames"] ?: NSNull.null};
    }
    if (dispatchTracing) g1_interval("probe usage", began, CACurrentMediaTime(), 0);
    return sample;
}

static NSDictionary *endpointUsage(void) {
    NSMutableDictionary *sample = [usage() mutableCopy];
    if (!getenv("G1_THREAD_CPU")) return sample;
    thread_act_array_t threads = NULL;
    mach_msg_type_number_t length = 0;
    if (task_threads(mach_task_self(), &threads, &length) != KERN_SUCCESS) abort();
    NSMutableArray *values = [NSMutableArray array];
    for (mach_msg_type_number_t i = 0; i < length; i++) {
        thread_basic_info_data_t basic;
        thread_identifier_info_data_t identity;
        thread_extended_info_data_t extended = {0};
        mach_msg_type_number_t count = THREAD_BASIC_INFO_COUNT;
        kern_return_t a = thread_info(threads[i], THREAD_BASIC_INFO, (thread_info_t)&basic, &count);
        count = THREAD_IDENTIFIER_INFO_COUNT;
        kern_return_t b = thread_info(threads[i], THREAD_IDENTIFIER_INFO, (thread_info_t)&identity, &count);
        count = THREAD_EXTENDED_INFO_COUNT;
        thread_info(threads[i], THREAD_EXTENDED_INFO, (thread_info_t)&extended, &count);
        if (a == KERN_SUCCESS && b == KERN_SUCCESS) {
            double cpu = basic.user_time.seconds + basic.system_time.seconds
                + (basic.user_time.microseconds + basic.system_time.microseconds) / 1e6;
            [values addObject:@{@"id":@(identity.thread_id), @"port":@(threads[i]),
                @"name":@(extended.pth_name), @"cpu":@(cpu), @"time":@(CACurrentMediaTime())}];
        }
        mach_port_deallocate(mach_task_self(), threads[i]);
    }
    vm_deallocate(mach_task_self(), (vm_address_t)threads, length * sizeof(thread_t));
    sample[@"threads"] = values;
    return sample;
}

static void writeJSON(NSDictionary *data, NSString *path) {
    NSError *error;
    NSData *bytes = [NSJSONSerialization dataWithJSONObject:data options:NSJSONWritingPrettyPrinted error:&error];
    if (!bytes || ![bytes writeToFile:path options:NSDataWritingAtomic error:&error]) {
        fprintf(stderr, "probe write: %s\n", error.description.UTF8String);
        exit(2);
    }
}

static void saveTrace(void) {
    double began = dispatchTracing ? CACurrentMediaTime() : 0;
    NSMutableArray *events = [NSMutableArray array];
    pthread_mutex_lock(&traceLock);
    for (size_t i = 0; i < intervalCount; i++) {
        typeof(intervals[0]) e = intervals[i];
        NSMutableDictionary *args = [@{@"value": @(e.value)} mutableCopy];
        if (e.caller) args[@"caller"] = @(e.caller);
        [events addObject:@{@"name": @(e.name), @"cat": @"ink", @"ph": @"X",
            @"ts": @(e.start * 1e6), @"dur": @((e.end - e.start) * 1e6),
            @"pid": @(getpid()), @"tid": @(e.thread), @"args": args}];
    }
    size_t dropped = intervalDropped;
    intervalCount = intervalDropped = 0;
    pthread_mutex_unlock(&traceLock);
    pthread_mutex_lock(&lock);
    NSArray *times = [presented copy];
    pthread_mutex_unlock(&lock);
    NSDictionary *trace = @{@"identity": @{@"host": @"apple-painted", @"app": @"exact-heavylist",
        @"trust": @"benchmark", @"pid": @(getpid()), @"savedAt": @([NSDate.date timeIntervalSince1970] * 1000)},
        @"proxies": @{@"interval": @"MTLDrawable.presentedTime", @"phases": @"inclusive spans on CACurrentMediaTime; overlaps are not additive"},
        @"traceEvents": events, @"presented": times, @"mainThread": @(mainThread), @"dropped": @(dropped)};
    writeJSON(trace, [NSTemporaryDirectory() stringByAppendingPathComponent:[NSString stringWithFormat:@"trace-pid-%d.json", getpid()]]);
    NSString *suffix = traceSaves++ == 0 ? @".trace.json" : [NSString stringWithFormat:@".trace-%u.json", traceSaves];
    writeJSON(trace, [output stringByAppendingString:suffix]);
    if (dispatchTracing) g1_interval("probe export", began, CACurrentMediaTime(), 0);
}

static NSScrollView *scrollView(NSView *root) {
    NSScrollView *largest = [root isKindOfClass:NSScrollView.class] ? (NSScrollView *)root : nil;
    if (largest.documentView.bounds.size.height <= largest.contentView.bounds.size.height) largest = nil;
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
@property NSMutableArray *samples;
@property NSMutableDictionary *result;
@property NSDictionary *start;
@property uint64_t peak;
@property NSDictionary *peakSample;
@property int phase;
@property double windowAt;
@property dispatch_source_t signals;
@property dispatch_source_t focusSignals;
@property dispatch_source_t traceExit;
@property dispatch_source_t memory;
@property id wheelMonitor;
@property NSUInteger wheels;
@property double wheelDelta;
@property NSMutableArray *wheelPositions;
@property NSUInteger inactiveTicks;
@property NSMutableArray *focusLosses;
@property BOOL hadFocus;
@property BOOL wasFocused;
@property pid_t lastFrontmost;
@end

@implementation G1Probe
- (NSDictionary *)focusState {
    NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
    return @{@"time": @(CACurrentMediaTime()), @"phase": @(self.phase),
        @"active": @(NSApp.active), @"keyWindow": @(NSApp.keyWindow.windowNumber),
        @"window": @(self.window.windowNumber), @"frontmostPid": @(front.processIdentifier),
        @"frontmostName": front.localizedName ?: @"unknown",
        @"frontmostBundle": front.bundleIdentifier ?: @"unknown"};
}
- (void)observeFocus {
    NSDictionary *state = [self focusState];
    BOOL focused = NSApp.active && self.window.keyWindow
        && [state[@"frontmostPid"] intValue] == getpid();
    if (!focused && self.hadFocus
        && (self.wasFocused || self.lastFrontmost != [state[@"frontmostPid"] intValue])) {
        [self.focusLosses addObject:state];
        writeJSON(@{@"events": self.focusLosses}, [output stringByAppendingString:@".focus-losses"]);
    }
    self.hadFocus |= focused;
    self.wasFocused = focused;
    self.lastFrontmost = [state[@"frontmostPid"] intValue];
}
- (void)confirmFocus {
    [self.window makeKeyAndOrderFront:nil];
    dispatch_async(dispatch_get_main_queue(), ^{
        [self observeFocus];
        writeJSON([self focusState], [output stringByAppendingString:@".focus"]);
    });
}
- (void)attach {
    for (NSWindow *window in NSApp.windows) {
        if (window.visible && window.contentView.bounds.size.height > 400) { self.window = window; break; }
    }
    if (!self.window) {
        if (CACurrentMediaTime() - processStart > 30) { fprintf(stderr, "probe: no window\n"); exit(2); }
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 1000000), dispatch_get_main_queue(), ^{ [self attach]; });
        return;
    }
    if (!getenv("EXACT_AGENT_LIVE") && !getenv("G1_BOOT_ONLY")) {
        [NSApp activate];
        [self.window makeKeyAndOrderFront:nil];
    }
    if (getenv("G1_BOOT_ONLY")) [self.window orderFrontRegardless];
    self.windowAt = CACurrentMediaTime();
    g1_interval("probe visible window", self.windowAt, self.windowAt, 0);
    self.ticks = [NSMutableArray array];
    self.coldPixels = [NSMutableArray array];
    self.coldTimes = [NSMutableArray array];
    self.result = [NSMutableDictionary dictionary];
    self.focusLosses = [NSMutableArray array];
    [self observeFocus];
    [NSWorkspace.sharedWorkspace.notificationCenter addObserverForName:NSWorkspaceDidActivateApplicationNotification
        object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) { [self observeFocus]; }];
    for (NSString *name in @[NSWindowDidBecomeKeyNotification, NSWindowDidResignKeyNotification,
                            NSApplicationDidBecomeActiveNotification, NSApplicationDidResignActiveNotification]) {
        [NSNotificationCenter.defaultCenter addObserverForName:name object:nil queue:NSOperationQueue.mainQueue
            usingBlock:^(NSNotification *note) { [self observeFocus]; }];
    }
    self.wheelPositions = [NSMutableArray array];
    self.link = [self.window.contentView displayLinkWithTarget:self selector:@selector(tick:)];
    float hz = self.window.screen.maximumFramesPerSecond;
    self.link.preferredFrameRateRange = CAFrameRateRangeMake(hz, hz, hz);
    [self.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    self.wheelMonitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskScrollWheel handler:^NSEvent *(NSEvent *event) {
        if (self.phase == 1) {
            self.wheels++; self.wheelDelta += event.scrollingDeltaY;
            if (self.wheelPositions.count < 3) [self.wheelPositions addObject:@[@(event.locationInWindow.x), @(event.locationInWindow.y)]];
        }
        return event;
    }];
    signal(SIGUSR2, SIG_IGN);
    self.signals = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGUSR2, 0, dispatch_get_main_queue());
    dispatch_source_set_event_handler(self.signals, ^{
        double began = dispatchTracing ? CACurrentMediaTime() : 0;
        [self advance];
        if (dispatchTracing) g1_interval("probe phase transition", began, CACurrentMediaTime(), self.phase);
    });
    dispatch_resume(self.signals);
    signal(SIGUSR1, SIG_IGN);
    self.focusSignals = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGUSR1, 0, dispatch_get_main_queue());
    dispatch_source_set_event_handler(self.focusSignals, ^{ if (tracing && getenv("EXACT_AGENT_LIVE")) saveTrace(); else [self confirmFocus]; });
    dispatch_resume(self.focusSignals);
    if (tracing) {
        signal(SIGTERM, SIG_IGN);
        self.traceExit = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGTERM, 0, dispatch_get_main_queue());
        dispatch_source_set_event_handler(self.traceExit, ^{ saveTrace(); exit(0); });
        dispatch_resume(self.traceExit);
    }
    if (tracing) [@"benchmark probe" writeToFile:[NSTemporaryDirectory() stringByAppendingPathComponent:
        [NSString stringWithFormat:@"exact-trace-%d.ready", getpid()]] atomically:YES encoding:NSUTF8StringEncoding error:nil];
    writeJSON([self focusState], [output stringByAppendingString:@".window"]);
}
- (void)tick:(CADisplayLink *)link {
    double now = CACurrentMediaTime();
    firstBootTick();
    if (!NSApp.active || !self.window.keyWindow || !self.hadFocus) [self observeFocus];
    if (self.phase == 0) {
        if (!painted) {
            [self.coldPixels addObject:signature(self.window.contentView)];
            [self.coldTimes addObject:@(link.timestamp)];
        }
        pthread_mutex_lock(&lock);
        BOOL ready = (firstComplete > 0 || (getenv("G1_AGENT_CPU_ONLY") && atomic_load(&completeScene))) && (!getenv("G1_BOOT_ONLY") || !atomic_load(&cpuSubmitted) || firstInkComplete > 0);
        pthread_mutex_unlock(&lock);
        if ((getenv("G1_BOOT_ONLY") || now - self.windowAt >= 2) && (!painted || ready)) [self ready];
        if (now - processStart > 30) {
            writeJSON(@{@"usage":usage(), @"completeScene":@(atomic_load(&completeScene)),
                @"firstComplete":@(firstComplete), @"drawableCalls":@(atomic_load(&drawableCalls)),
                @"visible":@(self.window.visible), @"occlusion":@(self.window.occlusionState)},
                [output stringByAppendingString:@".timeout"]);
            if (tracing) saveTrace();
            fprintf(stderr, "probe: first complete frame timed out\n"); exit(2);
        }
    } else if (self.phase == 1) {
        if (!NSApp.active || !self.window.keyWindow) self.inactiveTicks++;
        [self.ticks addObject:@{@"timestamp": @(link.timestamp), @"delivered": @(now),
            @"period": @(link.targetTimestamp - link.timestamp)}];
        NSDictionary *sample = usage();
        if ([sample[@"footprint"] unsignedLongLongValue] > self.peak) {
            self.peak = [sample[@"footprint"] unsignedLongLongValue]; self.peakSample = sample;
        }
    }
    if (dispatchTracing) g1_interval("probe display tick", now, CACurrentMediaTime(), self.phase);
}
- (void)ready {
    if (!painted) {
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
        self.result[@"coldCriterion"] = getenv("G1_ALLOW_IMAGE_REFUSALS")
            ? @"image-disabled control: first MTLDrawable.presentedTime; not a complete-scene startup measurement"
            : @"MTLDrawable.presentedTime for the first scene with all requested images loaded";
    }
    self.result[@"coldMs"] = @((firstComplete - processStart) * 1000);
    self.result[@"preMainMs"] = @((loaded - processStart) * 1000);
    if (tracing) {
        struct rusage finalUsage = {0};
        if (getrusage(RUSAGE_SELF, &finalUsage)) abort();
        self.result[@"startup"] = @{@"processStart": @(processStart), @"constructor": @(loaded),
            @"windowObserved": @(self.windowAt), @"firstPresented": @(firstDrawable), @"complete": @(firstComplete),
            @"firstCpuPresented": @(firstCpu), @"firstInkPresented": @(firstInk),
            @"firstCpuComplete": @(firstCpuComplete), @"firstInkComplete": @(firstInkComplete),
            @"constructorMinorFaults": @(constructorUsage.ru_minflt), @"constructorMajorFaults": @(constructorUsage.ru_majflt),
            @"endMinorFaults": @(finalUsage.ru_minflt), @"endMajorFaults": @(finalUsage.ru_majflt),
            @"loadedImages": @(_dyld_image_count()), @"registeredObjcClasses": @(objc_getClassList(NULL, 0)),
            @"constructorPageins": @(constructorRusage.ri_pageins),
            @"constructorInstructions": @(constructorRusage.ri_instructions),
            @"constructorCycles": @(constructorRusage.ri_cycles),
            @"constructorCpuSeconds": @((constructorRusage.ri_user_time + constructorRusage.ri_system_time) * tickSeconds),
            @"mainPageins": @(mainRusage.ri_pageins), @"mainCycles": @(mainRusage.ri_cycles),
            @"mainInstructions": @(mainRusage.ri_instructions),
            @"mainCpuSeconds": @((mainRusage.ri_user_time + mainRusage.ri_system_time) * tickSeconds),
            @"dyldStatisticsRequested": @(getenv("DYLD_PRINT_STATISTICS") != NULL),
            @"dyldStatisticsDetailsRequested": @(getenv("DYLD_PRINT_STATISTICS_DETAILS") != NULL),
            @"completeObserved": @(completeObserved),
            @"completeMinorFaults": @(completeUsage.ru_minflt), @"completeMajorFaults": @(completeUsage.ru_majflt),
            @"completePageins": @(completeRusage.ri_pageins),
            @"completeInstructions": @(completeRusage.ri_instructions), @"completeCycles": @(completeRusage.ri_cycles),
            @"completeCpuSeconds": @((completeRusage.ri_user_time + completeRusage.ri_system_time) * tickSeconds),
            @"processStartWallSeconds": @(processInfo.pbi_start_tvsec + processInfo.pbi_start_tvusec / 1e6),
            @"lazyQueueObserver": @(queueClass != nil), @"usageAtReport": usage()};
    }
    if (firstDrawable) self.result[@"firstDrawableMs"] = @((firstDrawable - processStart) * 1000);
    self.result[@"arm"] = arm;
    self.result[@"workload"] = workload;
    self.result[@"gpuHookEnabled"] = @(gpuHookEnabled);
    self.result[@"imageRefusalsAllowed"] = @(getenv("G1_ALLOW_IMAGE_REFUSALS") != NULL);
    if (painted) {
        self.result[@"drawableCalls"] = @(atomic_load(&drawableCalls));
        self.result[@"mainThreadDrawableCalls"] = @(atomic_load(&mainThreadDrawableCalls));
        self.result[@"maximumDrawableCount"] = @(atomic_load(&drawableCount));
    }
    self.result[@"pid"] = @(getpid());
    self.result[@"scale"] = @(self.window.backingScaleFactor);
    self.result[@"size"] = @[@(self.window.contentView.bounds.size.width), @(self.window.contentView.bounds.size.height)];
    self.result[@"frameSize"] = @[@(self.window.frame.size.width), @(self.window.frame.size.height)];
    self.result[@"thermalStart"] = @(NSProcessInfo.processInfo.thermalState);
    self.result[@"lowPower"] = @(NSProcessInfo.processInfo.lowPowerModeEnabled);
    self.result[@"activeAtStart"] = @(NSApp.active && self.window.keyWindow);
    self.result[@"screenHz"] = @(self.window.screen.maximumFramesPerSecond);
    if (getenv("G1_BOOT_ONLY")) {
        self.result[@"passiveStartup"] = @YES;
        self.result[@"activationSuppressed"] = @(atomic_load(&activationSuppressed));
        self.result[@"keySuppressed"] = @(atomic_load(&keySuppressed));
        self.result[@"cpuSubmitted"] = @(atomic_load(&cpuSubmitted));
        if (firstCpu) self.result[@"firstCpuMs"] = @((firstCpu - processStart) * 1000);
        if (firstInk) self.result[@"firstInkMs"] = @((firstInk - processStart) * 1000);
        if (firstCpuComplete) self.result[@"firstCpuCompleteMs"] = @((firstCpuComplete - processStart) * 1000);
        if (firstInkComplete) self.result[@"firstInkCompleteMs"] = @((firstInkComplete - processStart) * 1000);
        self.result[@"thermalEnd"] = @(NSProcessInfo.processInfo.thermalState);
        writeJSON(self.result, output);
        if (tracing) saveTrace();
        exit(0);
    }
    self.coldPixels = nil;
    self.coldTimes = nil;
    if (getenv("G1_AGENT_CPU_ONLY")) {
        self.result[@"cpuOnly"] = @YES;
        self.result[@"coldMs"] = NSNull.null;
        self.result[@"coldCriterion"] = @"CPU-only diagnostic: scene complete; no displayed-startup claim";
    }
    self.link.paused = YES;
    self.phase = -1;
    NSPoint point = NSMakePoint(400, 400);
    NSPoint screen = [self.window convertPointToScreen:point];
    writeJSON(@{@"pid": @(getpid()), @"window": @(self.window.windowNumber),
        @"windowAt": @(self.windowAt), @"firstComplete": @(firstComplete),
        @"completeAfterWindowMs": @((firstComplete - self.windowAt) * 1000),
        @"coldMs": self.result[@"coldMs"],
        @"point": @[@(point.x), @(self.window.frame.size.height - point.y)],
        @"screen": @[@(screen.x), @(NSScreen.screens.firstObject.frame.size.height - screen.y)],
        @"mark": @11490401, @"form": @"fling", @"wheel": @[@0, @600]},
        [output stringByAppendingString:@".ready"]);
}
- (void)advance {
    [self observeFocus];
    if (self.phase == -1) {
        if (getenv("G1_COLD_ONLY")) {
            self.result[@"thermalEnd"] = @(NSProcessInfo.processInfo.thermalState);
            self.result[@"activeAtEnd"] = @(NSApp.active && self.window.keyWindow);
            self.result[@"focusLosses"] = self.focusLosses;
            self.result[@"coldOnly"] = @YES;
            writeJSON(self.result, output);
            if (tracing) saveTrace();
            exit(0);
        }
        self.start = endpointUsage(); self.peak = [self.start[@"footprint"] unsignedLongLongValue]; self.peakSample = self.start;
        self.samples = [NSMutableArray arrayWithObject:self.start];
        if (!getenv("G1_NO_PROBE_TIMER")) {
            self.memory = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
            dispatch_source_set_timer(self.memory, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), NSEC_PER_SEC, NSEC_PER_MSEC);
            dispatch_source_set_event_handler(self.memory, ^{
                double began = dispatchTracing ? CACurrentMediaTime() : 0;
                [self observeFocus];
                NSDictionary *sample = usage();
                [self.samples addObject:sample];
                if ([sample[@"footprint"] unsignedLongLongValue] > self.peak) {
                    self.peak = [sample[@"footprint"] unsignedLongLongValue]; self.peakSample = sample;
                }
                if (dispatchTracing) g1_interval("probe timer", began, CACurrentMediaTime(), self.phase);
            });
            dispatch_resume(self.memory);
        }
        self.result[@"probeDisplayTicks"] = @(getenv("G1_NO_PROBE_DISPLAY") == NULL);
        self.result[@"probeMemoryTimer"] = @(getenv("G1_NO_PROBE_TIMER") == NULL);
        self.phase = 1; self.link.paused = getenv("G1_NO_PROBE_DISPLAY") != NULL; setGPUPhase(1);
        writeJSON(self.start, [output stringByAppendingFormat:@".%@", workload]);
    } else if (self.phase == 1) {
        setGPUPhase(2);
        NSDictionary *end = endpointUsage();
        if ([end[@"footprint"] unsignedLongLongValue] > self.peak) {
            self.peak = [end[@"footprint"] unsignedLongLongValue]; self.peakSample = end;
        }
        self.result[workload] = @{ @"start": self.start, @"end": end, @"peak": @(self.peak), @"peakSample": self.peakSample, @"displayLink": self.ticks,
            @"wheelEvents": @(self.wheels), @"wheelDeltaY": @(self.wheelDelta), @"wheelPositions": self.wheelPositions,
            @"inactiveTicks": @(self.inactiveTicks), @"samples": self.samples };
        if (tracing) writeJSON(self.result[workload], [output stringByAppendingFormat:@".%@-data", workload]);
        self.ticks = nil; self.link.paused = YES;
        NSScrollView *scroll = scrollView(self.window.contentView);
        if (scroll) self.result[@"nativeOffsetEnd"] = @(scroll.contentView.bounds.origin.y);
        if (painted) self.result[[arm stringByAppendingString:@"OffsetEnd"]] = @{@"item": @(topItem), @"withinItem": @(topOffset)};
        self.start = endpointUsage(); self.peak = [self.start[@"footprint"] unsignedLongLongValue]; self.peakSample = self.start;
        self.samples = [NSMutableArray arrayWithObject:self.start];
        self.phase = 2;
        writeJSON(self.start, [output stringByAppendingString:@".rest"]);
        if (tracing && !getenv("G1_REST_TRACE")) saveTrace();
    } else if (self.phase == 2) {
        setGPUPhase(0);
        self.result[@"rest"] = @{ @"start": self.start, @"end": endpointUsage(), @"peak": @(self.peak), @"peakSample": self.peakSample, @"samples": self.samples };
        if (tracing) saveTrace();
        self.result[@"thermalEnd"] = @(NSProcessInfo.processInfo.thermalState);
        self.result[@"activeAtEnd"] = @(NSApp.active && self.window.keyWindow);
        self.result[@"focusLosses"] = self.focusLosses;
        self.phase = 3;
        if (self.memory) dispatch_source_cancel(self.memory);
        // Retain completions and presentations for buffers committed at the phase boundary.
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 250 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{
            self.result[@"gpu"] = @{workload: gpuUsage(1), @"rest": gpuUsage(2), @"outside": gpuUsage(0)};
            pthread_mutex_lock(&lock);
            self.result[@"presented"] = [presented copy];
            pthread_mutex_unlock(&lock);
            writeJSON(self.result, output);
            exit(0);
        });
    }
}
@end

__attribute__((constructor)) static void initialize(void) {
    @autoreleasepool {
        const char *path = getenv("G1_OUTPUT");
        if (!path) return;
        if (getenv("G1_CLOCK_DIAGNOSTIC")) {
            [[NSString stringWithFormat:@"%d\n", getpid()]
                writeToFile:[@(path) stringByAppendingString:@".pid"] atomically:YES
                encoding:NSUTF8StringEncoding error:NULL];
        }
        if (getenv("G1_CLOCK_QOS")) {
            if (!getenv("G1_CLOCK_DIAGNOSTIC")) abort();
            int role = setpriority(PRIO_DARWIN_PROCESS, 0, 0);
            int qos = pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0);
            fprintf(stderr, "clock diagnostic: clear background=%d; interactive QoS=%d\n", role, qos);
            if (role || qos) abort();
        }
        loaded = CACurrentMediaTime();
        if (getrusage(RUSAGE_SELF, &constructorUsage)) abort();
        if (getenv("G1_AGENT_CPU_ONLY") && !getenv("EXACT_AGENT_LIVE")) abort();
        tracing = getenv("G1_TRACE") != NULL;
        if (tracing) {
            if (proc_pid_rusage(getpid(), RUSAGE_INFO_V4, (rusage_info_t *)&constructorRusage)) abort();
            if (proc_pidinfo(getpid(), PROC_PIDTBSDINFO, 0, &processInfo, sizeof(processInfo)) != sizeof(processInfo)) abort();
        }
        dispatchTracing = tracing && getenv("G1_DISPATCH_TRACE");
        wakeTracing = tracing && getenv("G1_WAKE_TRACE");
        if (dispatchTracing) {
            Method method = class_getInstanceMethod(NSApplication.class, @selector(sendEvent:));
            class_addMethod(NSApplication.class, @selector(g1_originalSendEvent:), method_getImplementation(method), method_getTypeEncoding(method));
            method_setImplementation(method, (IMP)sendEvent);
            observeRunLoop();
        }
        if (getenv("EXACT_AGENT_LIVE")) {
            method_setImplementation(class_getInstanceMethod(NSApplication.class, @selector(activate)), (IMP)backgroundActivate);
            method_setImplementation(class_getInstanceMethod(NSApplication.class, @selector(activateIgnoringOtherApps:)), (IMP)backgroundActivateIgnoring);
            method_setImplementation(class_getInstanceMethod(NSWindow.class, @selector(makeKeyAndOrderFront:)), (IMP)backgroundOrder);
        }
        mach_timebase_info_data_t timebase;
        mach_timebase_info(&timebase);
        tickSeconds = (double)timebase.numer / timebase.denom / 1e9;
        output = @(path);
        if (getenv("G1_BOOT_ONLY")) suppressBootFocus();
        arm = @(getenv("G1_ARM") ?: "native");
        workload = @(getenv("BENCH_MOTION") ?: "fling");
        if (![@[@"fling", @"bar", @"caret"] containsObject:workload]) {
            fprintf(stderr, "probe: BENCH_MOTION must be bar or caret\n"); exit(2);
        }
        if (![@[@"native", @"gpui", @"ink", @"egui"] containsObject:arm]) {
            fprintf(stderr, "probe: unknown arm %s\n", arm.UTF8String); exit(2);
        }
        painted = ![arm isEqual:@"native"];
        mainThread = mach_thread_self();
        struct rusage_info_v4 initial = {0};
        if (proc_pid_rusage(getpid(), RUSAGE_INFO_V4, (rusage_info_t *)&initial) || !initial.ri_proc_start_abstime) abort();
        processStart = initial.ri_proc_start_abstime * tickSeconds;
        presented = [NSMutableArray array];
        if (tracing) {
            g1_interval("probe constructor entry", loaded, loaded, 0);
            [NSNotificationCenter.defaultCenter addObserverForName:NSApplicationDidFinishLaunchingNotification
                object:nil queue:nil usingBlock:^(NSNotification *note) {
                    double now = CACurrentMediaTime();
                    g1_interval("AppKit did finish launching", now, now, 0);
                }];
            [NSNotificationCenter.defaultCenter addObserverForName:NSWindowDidUpdateNotification
                object:nil queue:nil usingBlock:^(NSNotification *note) {
                    static BOOL recorded;
                    NSWindow *window = note.object;
                    if (!recorded && window.contentView.bounds.size.height > 400) {
                        recorded = YES;
                        double now = CACurrentMediaTime();
                        g1_interval("probe first window update", now, now, 0);
                    }
                }];
        }
        configureGPUHook();
        {
            Method method = class_getInstanceMethod(CAMetalLayer.class, @selector(nextDrawable));
            class_addMethod(CAMetalLayer.class, @selector(g1_originalDrawable), method_getImplementation(method), method_getTypeEncoding(method));
            method_setImplementation(method, (IMP)nextDrawable);
        }
        dispatch_async(dispatch_get_main_queue(), ^{ static G1Probe *probe; probe = [G1Probe new]; [probe attach]; });
        g1_interval("probe constructor", loaded, CACurrentMediaTime(), 0);
    }
}

#ifdef G1_PROBE_TEST
@interface G1TestDrawable : NSObject
@property unsigned calls;
@end
@implementation G1TestDrawable
- (void)present { self.calls++; }
- (void)presentAtTime:(double)time { self.calls++; }
- (void)presentAfterMinimumDuration:(double)duration { self.calls++; }
@end

int main(void) {
    @autoreleasepool {
        mach_timebase_info_data_t timebase;
        mach_timebase_info(&timebase);
        tickSeconds = (double)timebase.numer / timebase.denom / 1e9;
        mainThread = mach_thread_self();
        NSDictionary *sample = usage();
        assert(sample[@"pageins"] && sample[@"cycles"] && sample[@"instructions"] && sample[@"cpu"]);
        if (getenv("G1_CLOCK_DIAGNOSTIC")) {
            assert([sample[@"mainPerf"] count] > 0 && sample[@"mainRequestedQos"]);
            assert(sample[@"taskPolicy"] && sample[@"processQos"]);
            puts("probe: clock placement, QoS, task policy and startup page-ins coexist");
        }
        presented = [NSMutableArray array];
        recordPresented(0, YES, 0);
        recordPresented(1, NO, 0);
        recordPresented(2, NO, 1);
        assert(firstComplete == 0 && firstCpuComplete == 0 && firstInkComplete == 0);
        recordPresented(3, YES, 0);
        recordPresented(4, YES, 1);
        recordPresented(5, YES, 1);
        assert(firstCpu == 1 && firstInk == 2 && firstComplete == 3);
        assert(firstCpuComplete == 3 && firstInkComplete == 4 && presented.count == 5);
        recordPresented(3.5, YES, 1);
        assert(firstInkComplete == 3.5 && firstComplete == 3);
        __block int wakes = 0;
        g1_boot_expose(^{ wakes++; });
        firstBootTick();
        firstBootTick();
        assert(wakes == 1);
        puts("probe: painter-specific completeness and one boot-expose wake passed");
        tracing = getenv("G1_TRACE") != NULL;
        assert(!queueClass && !bufferClass && !originalCommit);
        configureGPUHook();
        id<MTLDevice> device = traceDevice();
        if (!gpuHookEnabled) assert(!queueClass && !bufferClass && !originalCommit);
        id<MTLCommandQueue> queue = [device newCommandQueue];
        assert(queue);
        NSArray<id<MTLCommandBuffer>> *buffers = @[[queue commandBuffer],
            [queue commandBufferWithUnretainedReferences],
            [queue commandBufferWithDescriptor:[MTLCommandBufferDescriptor new]]];
        setGPUPhase(1);
        for (id<MTLCommandBuffer> buffer in buffers) {
            id<MTLBlitCommandEncoder> encoder = [buffer blitCommandEncoder];
            [encoder endEncoding];
            [buffer commit];
        }
        setGPUPhase(2);
        double seconds = 0;
        for (id<MTLCommandBuffer> buffer in buffers) {
            [buffer waitUntilCompleted];
            assert(buffer.status == MTLCommandBufferStatusCompleted);
            seconds += buffer.GPUEndTime - buffer.GPUStartTime;
        }
        NSDictionary *usage = gpuUsage(1);
        assert([usage[@"committed"] intValue] == (gpuHookEnabled ? 3 : 0));
        assert([usage[@"completed"] intValue] == (gpuHookEnabled ? 3 : 0));
        assert([usage[@"invalid"] intValue] == 0);
        assert(fabs([usage[@"seconds"] doubleValue] - (gpuHookEnabled ? seconds : 0)) < 1e-9);
        assert([gpuUsage(2)[@"committed"] intValue] == 0);
        if (!gpuHookEnabled && !tracing) assert(!queueClass && !bufferClass && !originalCommit);
        if (tracing) {
            assert(queueClass && bufferClass && originalCommit && intervalCount >= 4);
            puts("probe: lazy trace observes application queues without pre-creating a device or queue");
            G1TestDrawable *drawable = [G1TestDrawable new];
            watchDrawable(drawable);
            size_t before = intervalCount;
            [drawable present];
            [drawable presentAtTime:0];
            assert(intervalCount == before + 4 && drawable.calls == 2);
            assert(intervals[before + 1].start == lastCommitEnd);
            objc_setAssociatedObject(drawable, @selector(g1_bufferPresent), @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
            [drawable presentAfterMinimumDuration:0];
            assert(intervalCount == before + 4 && drawable.calls == 3);
            watchDrawable(drawable);
            [drawable presentAfterMinimumDuration:0];
            assert(intervalCount == before + 6 && drawable.calls == 4);
            puts("probe: direct presentation preserves methods, joins preceding commit, excludes buffer path and resets reused drawables");
        }
        printf("probe: GPU hook %s; three buffers completed, commit-phase accounting passed\n",
            gpuHookEnabled ? "on" : "off");
    }
    return 0;
}
#endif
