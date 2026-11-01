// Opt-in first-present diagnosis. AppKit facts are sampled only on main.
// @ref LLP 1149.001 M10, cut 1c.
static BOOL fpEnabled;
static pthread_mutex_t fpLock = PTHREAD_MUTEX_INITIALIZER;
static NSMutableArray *fpRows;
static NSMutableDictionary *fpViews;
static NSMapTable<CAMetalLayer *, NSView *> *fpLayers;
static atomic_uint fpSequence, fpSubmissions, fpCallbacks, fpFlushes, fpCommits;
static atomic_bool fpLinkRunning;
static double fpStarted;
static _Thread_local BOOL fpScanning;

static NSString *fpKey(id object) { return [NSString stringWithFormat:@"%p", object]; }
static BOOL fpActive(void) { return fpEnabled && CACurrentMediaTime() - fpStarted < 2; }
static void fpAppend(NSDictionary *row) {
    pthread_mutex_lock(&fpLock);
    if (fpRows.count < 4096) [fpRows addObject:row];
    pthread_mutex_unlock(&fpLock);
}
static void fpScan(NSView *view) {
    if ([view.layer isKindOfClass:CAMetalLayer.class]) [fpLayers setObject:view forKey:(CAMetalLayer *)view.layer];
    for (NSView *child in view.subviews) fpScan(child);
}
static void fpMain(void) {
    if (!fpActive() || !NSThread.isMainThread || fpScanning) return;
    fpScanning = YES;
    for (NSWindow *window in NSApp.windows) fpScan(window.contentView);
    for (CAMetalLayer *layer in fpLayers.keyEnumerator.allObjects) {
        NSView *view = [fpLayers objectForKey:layer];
        NSWindow *window = view.window;
        BOOL inTree = NO;
        CALayer *root = window.contentView.layer;
        for (CALayer *node = layer; node; node = node.superlayer) if (node == root) inTree = YES;
        NSRect bounds = view.bounds;
        NSDictionary *facts = @{@"sampledAt":@(CACurrentMediaTime()), @"view":fpKey(view),
            @"viewClass":NSStringFromClass(view.class), @"window":@(window.windowNumber),
            @"bounds":@[@(bounds.origin.x),@(bounds.origin.y),@(bounds.size.width),@(bounds.size.height)],
            @"backingScale":@(window.backingScaleFactor), @"visible":@(window.visible),
            @"occlusionState":@(window.occlusionState), @"inWindowLayerTree":@(inTree),
            @"isViewLayer":@(view.layer == layer), @"wantsLayer":@(view.wantsLayer),
            @"delegateClass":layer.delegate ? NSStringFromClass([(id)layer.delegate class]) : @"nil"};
        pthread_mutex_lock(&fpLock);
        fpViews[fpKey(layer)] = facts;
        pthread_mutex_unlock(&fpLock);
    }
    fpScanning = NO;
}
static void fpEvent(NSString *event, CAMetalLayer *layer, id<CAMetalDrawable> drawable, NSDictionary *extra) {
    if (!fpActive()) return;
    fpMain();
    NSMutableDictionary *row = [@{@"event":event, @"time":@(CACurrentMediaTime()),
        @"thread":@(pthread_mach_thread_np(pthread_self())), @"main":@(NSThread.isMainThread),
        @"submissions":@(atomic_load(&fpSubmissions)), @"callbacks":@(atomic_load(&fpCallbacks)),
        @"explicitFlushes":@(atomic_load(&fpFlushes)), @"explicitCommits":@(atomic_load(&fpCommits)),
        @"carrierLinkRunning":@(atomic_load(&fpLinkRunning))} mutableCopy];
    if (layer && objc_getAssociatedObject(layer, @selector(fpSized))) {
        CGSize size = layer.drawableSize;
        row[@"layer"] = fpKey(layer);
        row[@"drawableSize"] = @[@(size.width),@(size.height)];
        row[@"contentsScale"] = @(layer.contentsScale);
        row[@"presentsWithTransaction"] = @(layer.presentsWithTransaction);
        row[@"layerBounds"] = @[@(layer.bounds.size.width),@(layer.bounds.size.height)];
        row[@"superlayer"] = fpKey(layer.superlayer);
        pthread_mutex_lock(&fpLock);
        row[@"appKit"] = fpViews[fpKey(layer)] ?: @{};
        pthread_mutex_unlock(&fpLock);
    }
    if (drawable) {
        row[@"drawable"] = fpKey(drawable);
        row[@"sequence"] = objc_getAssociatedObject(drawable, @selector(fpSequence)) ?: @0;
        row[@"textureSize"] = @[@(drawable.texture.width),@(drawable.texture.height)];
    }
    [row addEntriesFromDictionary:extra ?: @{}];
    fpAppend(row);
}
static void fpAll(NSString *event, NSDictionary *extra) {
    if (!fpActive()) return;
    fpMain();
    // Only main enumerates the NSView map. Worker events retain their layer below.
    if (NSThread.isMainThread) {
        for (CAMetalLayer *layer in fpLayers.keyEnumerator.allObjects) fpEvent(event, layer, nil, extra);
    } else fpEvent(event, nil, nil, extra);
}
static void fpMark(const char *name, double value) {
    if (!fpActive()) return;
    if (!strcmp(name, "display running")) atomic_store(&fpLinkRunning, value != 0);
    if (!strcmp(name, "display callback") || !strcmp(name, "display wake") ||
        !strcmp(name, "display running") || !strcmp(name, "boot layer attached") ||
        !strcmp(name, "boot window shown") || !strcmp(name, "layer first transaction completed"))
        fpAll(@(name), @{@"value":@(value)});
}
static void fpAcquire(CAMetalLayer *layer, id<CAMetalDrawable> drawable) {
    if (!fpActive() || !drawable) return;
    unsigned sequence = atomic_fetch_add(&fpSequence, 1) + 1;
    objc_setAssociatedObject(drawable, @selector(fpSequence), @(sequence), OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    objc_setAssociatedObject(drawable, @selector(fpSubmitted), nil, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    fpEvent(@"acquire", layer, drawable, nil);
    [drawable addPresentedHandler:^(id<MTLDrawable> shown) {
        atomic_fetch_add(&fpCallbacks, 1);
        fpEvent(@"callback", layer, (id<CAMetalDrawable>)shown,
                @{@"sequence":@(sequence), @"presentedTime":@(shown.presentedTime)});
        dispatch_async(dispatch_get_main_queue(), ^{ fpEvent(@"callback main observation", layer, nil, @{@"sequence":@(sequence)}); });
    }];
}
static void fpSubmit(id drawable, NSString *event) {
    if (!fpActive() || ![drawable respondsToSelector:@selector(layer)]) return;
    if (!objc_getAssociatedObject(drawable, @selector(fpSubmitted))) {
        atomic_fetch_add(&fpSubmissions, 1);
        objc_setAssociatedObject(drawable, @selector(fpSubmitted), @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    }
    fpEvent(event, [(id<CAMetalDrawable>)drawable layer], drawable, nil);
}
static IMP fpSetSize, fpSetScale, fpSetContents, fpFlush, fpCommit, fpOrder;
static void fpSize(CAMetalLayer *layer, SEL selector, CGSize size) {
    fpEvent(@"drawableSize before assignment", layer, nil, @{@"new":@[@(size.width),@(size.height)]});
    ((void (*)(id,SEL,CGSize))fpSetSize)(layer, selector, size);
    objc_setAssociatedObject(layer, @selector(fpSized), @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    fpEvent(@"drawableSize assigned", layer, nil, nil);
}
static void fpScale(CALayer *layer, SEL selector, CGFloat scale) {
    ((void (*)(id,SEL,CGFloat))fpSetScale)(layer, selector, scale);
    if ([layer isKindOfClass:CAMetalLayer.class]) fpEvent(@"contentsScale assigned", (CAMetalLayer *)layer, nil, nil);
}
static void fpContents(CALayer *layer, SEL selector, id contents) {
    if ([layer isKindOfClass:CAMetalLayer.class]) fpEvent(@"contents assigned", (CAMetalLayer *)layer, nil,
        @{@"contents":fpKey(contents), @"caller":@(callerName(__builtin_return_address(0)))});
    ((void (*)(id,SEL,id))fpSetContents)(layer, selector, contents);
}
static void fpFlushCall(id object, SEL selector) {
    atomic_fetch_add(&fpFlushes, 1); fpAll(@"CA flush begin", nil);
    ((void (*)(id,SEL))fpFlush)(object, selector); fpAll(@"CA flush end", nil);
}
static void fpCommitCall(id object, SEL selector) {
    atomic_fetch_add(&fpCommits, 1); fpAll(@"CA commit begin", nil);
    ((void (*)(id,SEL))fpCommit)(object, selector); fpAll(@"CA commit end", nil);
}
static void fpOrderCall(NSWindow *window, SEL selector, id sender) {
    fpAll(@"makeKeyAndOrderFront begin", nil);
    ((void (*)(id,SEL,id))fpOrder)(window, selector, sender);
    fpAll(@"makeKeyAndOrderFront end", nil);
}
static IMP fpReplace(Class cls, SEL selector, IMP replacement) {
    Method method = class_getInstanceMethod(cls, selector);
    IMP original = method_getImplementation(method);
    class_replaceMethod(cls, selector, replacement, method_getTypeEncoding(method));
    return original;
}
static void fpInit(void) {
    if (!(fpEnabled = getenv("G1_FIRST_PRESENT") != NULL)) return;
    fpStarted = CACurrentMediaTime(); fpRows = [NSMutableArray new]; fpViews = [NSMutableDictionary new];
    fpLayers = [NSMapTable strongToWeakObjectsMapTable];
    fpSetSize = fpReplace(CAMetalLayer.class, @selector(setDrawableSize:), (IMP)fpSize);
    fpSetScale = fpReplace(CALayer.class, @selector(setContentsScale:), (IMP)fpScale);
    fpSetContents = fpReplace(CALayer.class, @selector(setContents:), (IMP)fpContents);
    fpFlush = fpReplace(object_getClass(CATransaction.class), @selector(flush), (IMP)fpFlushCall);
    fpCommit = fpReplace(object_getClass(CATransaction.class), @selector(commit), (IMP)fpCommitCall);
    fpOrder = fpReplace(NSWindow.class, @selector(makeKeyAndOrderFront:), (IMP)fpOrderCall);
}
static NSArray *fpSnapshot(void) {
    pthread_mutex_lock(&fpLock); NSArray *rows = [fpRows copy] ?: @[]; pthread_mutex_unlock(&fpLock);
    return rows;
}
