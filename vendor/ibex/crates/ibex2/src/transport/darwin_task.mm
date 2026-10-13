// NSURLSession tasks with delegates, for the asynchronous transport
// (exact2 LLP 1120 D2): one session per lane, created on first use; each
// task has its own delegate, and every callback of the lane runs on the
// session's one serial delegate queue, never waiting on anything.
//
// The rules of darwin_http.mm hold here too: an ephemeral session with no
// cookies and no URL cache, and no redirect followed by the platform
// (willPerformHTTPRedirection completes with nil), because Rust re-checks
// the net.fetch grant on every hop.
//
// @ref LLP 0057#3-the-boundary — the platform executes; it does not decide

#import <Foundation/Foundation.h>

#include <cstdlib>
#include <cstring>

extern "C" {
// Rust's half (darwin_async.rs). `ctx` is one exchange's sink, called only
// from the lane's serial queue; it is freed exactly once.
int ibex2_async_head(void *ctx, int status, const char *headers, long long declared);
int ibex2_async_data(void *ctx, const unsigned char *bytes, size_t length);
void ibex2_async_end(void *ctx, const char *error);
void ibex2_async_metrics(void *ctx, const char *protocol, int reused, const char *local,
                         const char *remote, double queued_ms);
void ibex2_async_sink_free(void *ctx);
void ibex2_async_body_free(void *body);
}

namespace {
enum { kContinue = 0, kStop = 1, kSkip = 2 };

const char *utf8(NSString *value) { return value == nil ? nullptr : [value UTF8String]; }

NSString *endpoint(NSString *address, NSNumber *port) {
  if (address == nil) return nil;
  if (port == nil) return address;
  BOOL v6 = [address rangeOfString:@":"].location != NSNotFound;
  return [NSString stringWithFormat:v6 ? @"[%@]:%@" : @"%@:%@", address, port];
}
} // namespace

// One exchange's delegate: the task holds it until it completes, the lane
// until its metrics have come too (or a second after completion).
@interface Ibex2TaskDelegate : NSObject <NSURLSessionDataDelegate>
@property(nonatomic, assign) void *ctx;
@property(nonatomic, assign) BOOL stopped;
@property(nonatomic, assign) BOOL ended;
@property(nonatomic, assign) BOOL measured;
@property(nonatomic, strong) NSOperationQueue *queue;
@property(nonatomic, strong) NSMutableSet *keep;
// When the task was resumed: the platform's queueing is measured from here
// (taskInterval starts when the task was made).
@property(nonatomic, strong) NSDate *resumed;
// On the lane's queue: the exchange was activated. A task the session
// completed before that (its invalidation) keeps its end until then.
@property(nonatomic, assign) BOOL activated;
@property(nonatomic, assign) BOOL early;
@property(nonatomic, copy) NSString *earlyFailure;
@end

@implementation Ibex2TaskDelegate
// On the lane's queue: the sink goes once it has had its end and metrics.
- (void)release_ {
  if (self.ctx != nullptr) {
    ibex2_async_sink_free(self.ctx);
    self.ctx = nullptr;
  }
  [self.keep removeObject:self];
}
- (void)URLSession:(NSURLSession *)session
              dataTask:(NSURLSessionDataTask *)task
    didReceiveResponse:(NSURLResponse *)response
     completionHandler:(void (^)(NSURLSessionResponseDisposition))completionHandler {
  if (self.ctx == nullptr || self.stopped || !self.activated ||
      ![response isKindOfClass:[NSHTTPURLResponse class]]) {
    self.stopped = YES;
    completionHandler(NSURLSessionResponseCancel);
    return;
  }
  NSHTTPURLResponse *http = (NSHTTPURLResponse *)response;
  NSMutableString *headers = [NSMutableString string];
  [http.allHeaderFields enumerateKeysAndObjectsUsingBlock:^(id key, id value, BOOL *) {
    [headers appendFormat:@"%@: %@\n", key, value];
  }];
  int flow = ibex2_async_head(self.ctx, (int)http.statusCode, utf8(headers),
                              (long long)response.expectedContentLength);
  if (flow != kContinue) {
    self.stopped = YES;
    completionHandler(NSURLSessionResponseCancel);
  } else {
    completionHandler(NSURLSessionResponseAllow);
  }
}
- (void)URLSession:(NSURLSession *)session
          dataTask:(NSURLSessionDataTask *)task
    didReceiveData:(NSData *)data {
  if (self.ctx == nullptr || self.stopped || !self.activated) return;
  __block BOOL stop = NO;
  [data enumerateByteRangesUsingBlock:^(const void *bytes, NSRange range, BOOL *done) {
    if (ibex2_async_data(self.ctx, (const unsigned char *)bytes, range.length) != kContinue) {
      stop = YES;
      *done = YES;
    }
  }];
  if (stop) {
    self.stopped = YES;
    [task cancel];
  }
}
- (void)URLSession:(NSURLSession *)session
                    task:(NSURLSessionTask *)task
    didCompleteWithError:(NSError *)error {
  NSString *failure = error == nil ? nil
      : [NSString stringWithFormat:@"TypeError: Failed to fetch — %@", error.localizedDescription];
  if (!self.activated) {
    // Completed by the session before activation: told at activation.
    self.early = YES;
    self.earlyFailure = failure ?: @"TypeError: Failed to fetch — cancelled";
    return;
  }
  [self endWith:failure];
}
// On the lane's queue, once activated: the end, then the sink's release
// once metrics have come or had their second.
- (void)endWith:(NSString *)failure {
  if (self.ended) return;
  self.ended = YES;
  if (self.ctx != nullptr) ibex2_async_end(self.ctx, utf8(failure));
  if (self.measured) {
    [self release_];
    return;
  }
  // Metrics may follow completion; they get a second.
  __weak Ibex2TaskDelegate *weak = self;
  NSOperationQueue *queue = self.queue;
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC),
                 dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
    [queue addOperationWithBlock:^{ [weak release_]; }];
  });
}
- (void)URLSession:(NSURLSession *)session
                          task:(NSURLSessionTask *)task
    didFinishCollectingMetrics:(NSURLSessionTaskMetrics *)metrics {
  if (self.measured) return;
  self.measured = YES;
  // Before activation nothing was sent: no metrics to tell.
  if (!self.activated) return;
  if (self.ctx != nullptr) {
    NSURLSessionTaskTransactionMetrics *first = metrics.transactionMetrics.firstObject;
    NSURLSessionTaskTransactionMetrics *last = metrics.transactionMetrics.lastObject;
    double queued = -1;
    if (first.fetchStartDate != nil && self.resumed != nil) {
      queued = MAX(0.0, [first.fetchStartDate timeIntervalSinceDate:self.resumed] * 1000.0);
    }
    NSString *local = endpoint(last.localAddress, last.localPort);
    NSString *remote = endpoint(last.remoteAddress, last.remotePort);
    ibex2_async_metrics(self.ctx, utf8(last.networkProtocolName),
                        last == nil ? -1 : (last.reusedConnection ? 1 : 0), utf8(local),
                        utf8(remote), queued);
  }
  if (self.ended) [self release_];
}
- (void)URLSession:(NSURLSession *)session
                          task:(NSURLSessionTask *)task
    willPerformHTTPRedirection:(NSHTTPURLResponse *)response
                    newRequest:(NSURLRequest *)request
             completionHandler:(void (^)(NSURLRequest *))completionHandler {
  completionHandler(nil);
}
@end

// The session's own delegate: it outlives retirement until invalidation.
@interface Ibex2LaneDelegate : NSObject <NSURLSessionDelegate>
@end
@implementation Ibex2LaneDelegate
- (void)URLSession:(NSURLSession *)session didBecomeInvalidWithError:(NSError *)error {
}
@end

// A started exchange until Rust activates it: its task, or the refusal its
// sink is told on the lane's queue.
@interface Ibex2Start : NSObject
@property(nonatomic, strong) NSURLSessionDataTask *task;
@property(nonatomic, strong) Ibex2TaskDelegate *delegate;
@property(nonatomic, copy) NSString *refusal;
// Guarded by @synchronized(self): a cancel before activation waits for it,
// so a suspended task, which a cancel would complete at once, calls back
// only once activated, and is never resumed.
@property(nonatomic, assign) BOOL activated;
@property(nonatomic, assign) BOOL cancelled;
@end
@implementation Ibex2Start
@end

@interface Ibex2Lane : NSObject
@property(nonatomic, strong) NSURLSession *session;
@property(nonatomic, strong) NSOperationQueue *queue;
// Delegates waiting for their metrics after completion.
@property(nonatomic, strong) NSMutableSet *keep;
@end
@implementation Ibex2Lane
@end

extern "C" {

void *ibex2_darwin_lane_create(int max_per_host) {
  @autoreleasepool {
    NSURLSessionConfiguration *config = [NSURLSessionConfiguration ephemeralSessionConfiguration];
    // As darwin_http.mm's new_session, for its reasons: no cookies, no cache.
    config.HTTPCookieAcceptPolicy = NSHTTPCookieAcceptPolicyNever;
    config.HTTPCookieStorage = nil;
    config.HTTPShouldSetCookies = NO;
    config.URLCache = nil;
    config.requestCachePolicy = NSURLRequestReloadIgnoringLocalCacheData;
    if (max_per_host > 0) config.HTTPMaximumConnectionsPerHost = max_per_host;
    Ibex2Lane *lane = [[Ibex2Lane alloc] init];
    lane.queue = [[NSOperationQueue alloc] init];
    lane.queue.maxConcurrentOperationCount = 1;
    lane.queue.name = [NSString stringWithFormat:@"ibex2.lane.%p", lane];
    lane.keep = [NSMutableSet set];
    lane.session = [NSURLSession sessionWithConfiguration:config
                                                 delegate:[[Ibex2LaneDelegate alloc] init]
                                            delegateQueue:lane.queue];
    return (__bridge_retained void *)lane;
  }
}

/// What the lane's session keeps, read back so a test can check it: a
/// cookie jar or policy, a URL cache, and its per-host connection limit.
void ibex2_darwin_lane_state(void *handle, int *out_cookies, int *out_cache, int *out_per_host) {
  @autoreleasepool {
    NSURLSessionConfiguration *config = ((__bridge Ibex2Lane *)handle).session.configuration;
    *out_cookies = (config.HTTPCookieStorage != nil || config.HTTPShouldSetCookies ||
                    config.HTTPCookieAcceptPolicy != NSHTTPCookieAcceptPolicyNever) ? 1 : 0;
    *out_cache = config.URLCache != nil ? 1 : 0;
    *out_per_host = (int)config.HTTPMaximumConnectionsPerHost;
  }
}

/// Retirement: every task is cancelled, and each still gets its completion.
void ibex2_darwin_lane_destroy(void *handle) {
  if (handle == nullptr) return;
  @autoreleasepool {
    Ibex2Lane *lane = (__bridge_transfer Ibex2Lane *)handle;
    [lane.session invalidateAndCancel];
  }
}

/// Prepare a task, or (`refusal` set, or an invalid URL) a refusal. Nothing
/// reaches the sink until `ibex2_darwin_task_activate`, which resumes the
/// task or delivers the refusal on the lane's queue. `body` is lent: its
/// deallocator frees `body_ctx`.
void *ibex2_darwin_task_start(void *handle, const char *method, const char *url,
                              const char *header_block, const unsigned char *body,
                              size_t body_length, void *body_ctx, double timeout_seconds,
                              const char *refusal, void *ctx) {
  @autoreleasepool {
    Ibex2Lane *lane = (__bridge Ibex2Lane *)handle;
    NSData *payload = nil;
    if (body_ctx != nullptr) {
      payload = [[NSData alloc] initWithBytesNoCopy:(void *)body
                                             length:body_length
                                        deallocator:^(void *, NSUInteger) {
                                          ibex2_async_body_free(body_ctx);
                                        }];
    }
    Ibex2TaskDelegate *delegate = [[Ibex2TaskDelegate alloc] init];
    delegate.ctx = ctx;
    delegate.queue = lane.queue;
    delegate.keep = lane.keep;
    Ibex2Start *start = [[Ibex2Start alloc] init];
    start.delegate = delegate;
    // Kept by the lane until its metrics have come (`release_`): queued now,
    // ahead of anything the task can call back, on the same serial queue.
    [lane.queue addOperationWithBlock:^{ [delegate.keep addObject:delegate]; }];
    NSURL *nsurl = refusal != nullptr ? nil : [NSURL URLWithString:[NSString stringWithUTF8String:url]];
    if (nsurl == nil) {
      start.refusal = refusal != nullptr ? [NSString stringWithUTF8String:refusal]
                                         : @"TypeError: Failed to fetch — invalid URL";
      return (__bridge_retained void *)start;
    }
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:nsurl];
    request.HTTPMethod = [NSString stringWithUTF8String:method];
    if (header_block != nullptr) {
      NSString *block = [NSString stringWithUTF8String:header_block];
      for (NSString *line in [block componentsSeparatedByString:@"\n"]) {
        NSRange colon = [line rangeOfString:@": "];
        if (colon.location == NSNotFound) continue;
        [request setValue:[line substringFromIndex:colon.location + colon.length]
            forHTTPHeaderField:[line substringToIndex:colon.location]];
      }
    }
    if (payload != nil) request.HTTPBody = payload;
    if (timeout_seconds > 0) request.timeoutInterval = timeout_seconds;
    start.task = [lane.session dataTaskWithRequest:request];
    start.task.delegate = delegate;
    return (__bridge_retained void *)start;
  }
}

/// Let the exchange deliver: resume its task, or tell its sink its refusal.
void ibex2_darwin_task_activate(void *handle) {
  @autoreleasepool {
    Ibex2Start *start = (__bridge Ibex2Start *)handle;
    Ibex2TaskDelegate *delegate = start.delegate;
    NSURLSessionDataTask *task = start.task;
    NSString *refusal = start.refusal;
    BOOL cancelled;
    @synchronized(start) {
      if (start.activated) return;
      start.activated = YES;
      cancelled = start.cancelled;
    }
    // Ahead of anything a resume makes the task call back, on the same
    // serial queue: from here the sink may hear, and an end the session
    // made before (its invalidation) is told now.
    [delegate.queue addOperationWithBlock:^{
      delegate.activated = YES;
      if (task == nil) {
        delegate.measured = YES;
        [delegate endWith:refusal];
      } else if (delegate.early) {
        delegate.measured = YES;
        [delegate endWith:delegate.earlyFailure];
      }
    }];
    if (task == nil) {
      return;
    } else if (cancelled) {
      // Never resumed: it completes as cancelled, sending nothing.
      [task cancel];
    } else {
      delegate.resumed = [NSDate date];
      [task resume];
    }
  }
}

void ibex2_darwin_task_cancel(void *handle) {
  @autoreleasepool {
    Ibex2Start *start = (__bridge Ibex2Start *)handle;
    @synchronized(start) {
      if (!start.activated) {
        start.cancelled = YES;
        return;
      }
    }
    [start.task cancel];
  }
}

void ibex2_darwin_task_release(void *handle) {
  @autoreleasepool {
    Ibex2Start *start = (__bridge_transfer Ibex2Start *)handle;
    (void)start;
  }
}

} // extern "C"
