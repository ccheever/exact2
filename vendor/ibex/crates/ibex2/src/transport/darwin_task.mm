// NSURLSession tasks with delegates, for the asynchronous transport
// (LLP 0068.000 §5): one session per lane, created on first use; each
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

// A replayable immutable upload owner. Stream metadata can outlive a task,
// but physical completion detaches the Rust allocation. A read copies at most
// 64 KiB while holding the same lock as detach; no raw pointer escapes.
@interface Ibex2Upload : NSObject
@property(nonatomic, assign) const unsigned char *bytes;
@property(nonatomic, assign) NSUInteger length;
@property(nonatomic, assign) void *ctx;
- (void)detach;
@end
@implementation Ibex2Upload
- (void)detach {
  void *released;
  @synchronized(self) { released = self.ctx; self.ctx = nullptr; self.bytes = nullptr; }
  if (released != nullptr) ibex2_async_body_free(released);
}
- (void)dealloc { [self detach]; }
@end

@interface Ibex2UploadStream : NSInputStream {
  Ibex2Upload *_upload;
  NSUInteger _offset;
  NSStreamStatus _status;
  __weak id<NSStreamDelegate> _observer;
  NSRunLoop *_loop;
  NSString *_mode;
  BOOL _notificationPending;
  BOOL _openedNotified;
}
- (instancetype)initWithUpload:(Ibex2Upload *)upload;
- (void)notify;
@end
@implementation Ibex2UploadStream
- (instancetype)initWithUpload:(Ibex2Upload *)upload {
  if ((self = [super init])) { _upload = upload; _status = NSStreamStatusNotOpen; }
  return self;
}
- (void)notify {
  NSRunLoop *loop;
  NSString *mode;
  @synchronized(self) {
    if (_notificationPending || _loop == nil || _status == NSStreamStatusClosed) return;
    _notificationPending = YES; loop = _loop; mode = _mode;
  }
  // One outstanding notification and one scheduled native run-loop context;
  // URLSession schedules each private input stream on its network run loop.
  [loop performInModes:@[mode] block:^{
    id<NSStreamDelegate> observer;
    NSStreamEvent event;
    @synchronized(self) {
      self->_notificationPending = NO;
      observer = self->_observer;
      if (self->_status == NSStreamStatusClosed || self->_loop == nil) return;
      if (!self->_openedNotified && self->_status != NSStreamStatusNotOpen) {
        self->_openedNotified = YES; event = NSStreamEventOpenCompleted;
      } else if (self->_status == NSStreamStatusAtEnd) {
        event = NSStreamEventEndEncountered;
      } else if (self->_status == NSStreamStatusOpen) {
        event = NSStreamEventHasBytesAvailable;
      } else { return; }
    }
    [observer stream:self handleEvent:event];
    if (event == NSStreamEventOpenCompleted) [self notify];
  }];
}
- (void)open { @synchronized(self) { if (_status == NSStreamStatusNotOpen) _status = NSStreamStatusOpen; } [self notify]; }
- (void)close { @synchronized(self) { _status = NSStreamStatusClosed; _loop = nil; _mode = nil; _observer = nil; } }
- (NSInteger)read:(uint8_t *)buffer maxLength:(NSUInteger)length {
  NSUInteger count = 0;
  @synchronized(self) {
    if (_status == NSStreamStatusClosed || _status == NSStreamStatusNotOpen) return -1;
    @synchronized(_upload) {
      if (_upload.bytes != nullptr && _offset < _upload.length) {
        count = MIN(MIN(length, _upload.length - _offset), (NSUInteger)(64 << 10));
        memcpy(buffer, _upload.bytes + _offset, count); _offset += count;
      }
      if (_upload.bytes == nullptr || _offset == _upload.length) _status = NSStreamStatusAtEnd;
    }
  }
  [self notify];
  return (NSInteger)count;
}
- (BOOL)getBuffer:(uint8_t **)buffer length:(NSUInteger *)length { return NO; }
- (BOOL)hasBytesAvailable {
  @synchronized(self) { @synchronized(_upload) {
    return _status == NSStreamStatusOpen && _upload.bytes != nullptr && _offset < _upload.length;
  } }
}
- (NSStreamStatus)streamStatus { @synchronized(self) { return _status; } }
- (NSError *)streamError { return nil; }
- (id<NSStreamDelegate>)delegate { @synchronized(self) { return _observer; } }
- (void)setDelegate:(id<NSStreamDelegate>)observer { @synchronized(self) { _observer = observer; } }
- (void)scheduleInRunLoop:(NSRunLoop *)loop forMode:(NSRunLoopMode)mode {
  @synchronized(self) { _loop = loop; _mode = mode; } [self notify];
}
- (void)removeFromRunLoop:(NSRunLoop *)loop forMode:(NSRunLoopMode)mode {
  @synchronized(self) { if (_loop == loop && [_mode isEqual:mode]) { _loop = nil; _mode = nil; } }
}
- (id)propertyForKey:(NSStreamPropertyKey)key {
  @synchronized(self) { return [key isEqual:NSStreamFileCurrentOffsetKey] ? @(_offset) : nil; }
}
- (BOOL)setProperty:(id)value forKey:(NSStreamPropertyKey)key { return NO; }
@end

@class Ibex2TaskDelegate;
// A started exchange until Rust activates it: its task, or the refusal its
// sink is told on the lane's queue.
@interface Ibex2Start : NSObject
@property(nonatomic, strong) NSURLSessionDataTask *task;
@property(nonatomic, strong) Ibex2TaskDelegate *delegate;
@property(nonatomic, copy) NSString *refusal;
// Guarded by @synchronized(self). Exchange.cancel also schedules activation
// to retire a prepared task without resuming it.
@property(nonatomic, assign) BOOL activated;
@property(nonatomic, assign) BOOL cancelled;
@end
@implementation Ibex2Start
@end

// One exchange's delegate, retained until physical completion. Optional
// metrics after completion are discarded.
@interface Ibex2TaskDelegate : NSObject <NSURLSessionDataDelegate>
@property(nonatomic, weak) Ibex2Start *owner;
@property(nonatomic, strong) Ibex2Upload *upload;
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
  // Foundation has already parsed its header dictionary. Bound our own
  // serialized copy before appending, separately from the consumer's cap.
  NSMutableString *headers = [NSMutableString string];
  __block NSUInteger bytes = 0, count = 0;
  __block BOOL oversized = NO;
  [http.allHeaderFields enumerateKeysAndObjectsUsingBlock:^(id key, id value, BOOL *stop) {
    NSString *name = [key description], *text = [value description];
    NSUInteger addition = [name lengthOfBytesUsingEncoding:NSUTF8StringEncoding] +
                          [text lengthOfBytesUsingEncoding:NSUTF8StringEncoding] + 3;
    if (++count > 4096 || addition > (1 << 20) || bytes > (1 << 20) - addition) {
      oversized = YES; *stop = YES; return;
    }
    bytes += addition;
    [headers appendFormat:@"%@: %@\n", name, text];
  }];
  if (oversized) {
    self.stopped = YES;
    self.earlyFailure = @"HTTP headers exceed native transport limit";
    completionHandler(NSURLSessionResponseCancel);
    return;
  }
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
    // Session retirement cancels suspended accepted tasks too. There need
    // not be a future activation; physical end is delivered on this queue.
    self.activated = YES;
    self.measured = YES;
    [self endWith:failure ?: @"TypeError: Failed to fetch — cancelled"];
    return;
  }
  [self endWith:self.earlyFailure ?: failure];
}
// On the lane's queue: report physical end and release the sink.
- (void)endWith:(NSString *)failure {
  if (self.ended) return;
  self.ended = YES;
  [self.upload detach];
  self.upload = nil;
  // A retained cancellation handle must not retain the completed upload.
  Ibex2Start *owner = self.owner;
  __attribute__((objc_precise_lifetime)) NSURLSessionDataTask *completed;
  @synchronized(owner) { completed = owner.task; owner.task = nil; }
  (void)completed;
  completed = nil;
  if (self.ctx != nullptr) ibex2_async_end(self.ctx, utf8(failure));
  // Metrics are optional. NSURLSession normally supplies them before end;
  // release the sink at physical end instead of retaining payload-bearing
  // consumer closures for a grace-period timer. Late metrics are discarded.
  [self release_];
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
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task
    needNewBodyStream:(void (^)(NSInputStream *))completionHandler {
  completionHandler(self.ended || self.upload == nil ? nil
                    : [[Ibex2UploadStream alloc] initWithUpload:self.upload]);
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

@interface Ibex2Lane : NSObject
@property(nonatomic, strong) NSURLSession *session;
@property(nonatomic, strong) NSOperationQueue *queue;
// Active or prepared delegates, released at physical completion.
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
    [lane.queue addOperationWithBlock:^{
      // Invalid/refused prepared exchanges have no NSURLSessionTask, hence
      // no native completion. Retire them (and suspended tasks) explicitly.
      for (Ibex2TaskDelegate *delegate in [lane.keep allObjects]) {
        if (!delegate.activated) {
          delegate.activated = YES;
          delegate.measured = YES;
          [delegate endWith:@"TypeError: Failed to fetch — session retired"];
        }
      }
    }];
  }
}

/// Prepare a task, or (`refusal` set, or an invalid URL) a refusal. Nothing
/// reaches the sink until `ibex2_darwin_task_activate`, which resumes the
/// task or delivers the refusal on the lane's queue. The upload owner holds
/// `body_ctx` until physical completion, serializing detach against reads.
void *ibex2_darwin_task_start(void *handle, const char *method, const char *url,
                              const char *header_block, const unsigned char *body,
                              size_t body_length, void *body_ctx, double timeout_seconds,
                              const char *refusal, void *ctx) {
  @autoreleasepool {
    Ibex2Lane *lane = (__bridge Ibex2Lane *)handle;
    Ibex2Upload *upload = nil;
    if (body_ctx != nullptr) {
      upload = [[Ibex2Upload alloc] init];
      upload.bytes = body; upload.length = body_length; upload.ctx = body_ctx;
    }
    Ibex2TaskDelegate *delegate = [[Ibex2TaskDelegate alloc] init];
    delegate.ctx = ctx;
    delegate.upload = upload;
    delegate.queue = lane.queue;
    delegate.keep = lane.keep;
    Ibex2Start *start = [[Ibex2Start alloc] init];
    start.delegate = delegate;
    delegate.owner = start;
    // Kept by the lane until physical completion (`release_`): queued now,
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
    if (timeout_seconds > 0) request.timeoutInterval = timeout_seconds;
    if (upload != nil) [request setValue:[NSString stringWithFormat:@"%zu", body_length]
                              forHTTPHeaderField:@"Content-Length"];
    start.task = upload != nil ? [lane.session uploadTaskWithStreamedRequest:request]
                               : [lane.session dataTaskWithRequest:request];
    start.task.delegate = delegate;
    return (__bridge_retained void *)start;
  }
}

/// Let the exchange deliver: resume its task, or tell its sink its refusal.
void ibex2_darwin_task_activate(void *handle) {
  @autoreleasepool {
    Ibex2Start *start = (__bridge Ibex2Start *)handle;
    Ibex2TaskDelegate *delegate = start.delegate;
    NSString *refusal = start.refusal;
    @synchronized(start) {
      if (start.activated) return;
      start.activated = YES;

    }
    // Only the queue resumes a task. Do not capture the task in a completed
    // NSOperation's block: Foundation may retain that operation after it ran,
    // which would retain the immutable upload past physical completion.
    [delegate.queue addOperationWithBlock:^{
      NSURLSessionDataTask *running;
      BOOL stopped;
      @synchronized(start) { running = start.task; stopped = start.cancelled; }
      delegate.activated = YES;
      delegate.resumed = [NSDate date];
      if (running == nil) {
        delegate.measured = YES;
        [delegate endWith:stopped ? @"TypeError: Failed to fetch — cancelled" : refusal];
      } else if (stopped) {
        [running cancel];
      } else {
        [running resume];
      }
    }];
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
    NSURLSessionDataTask *task;
    @synchronized(start) { task = start.task; }
    [task cancel];
  }
}

void ibex2_darwin_task_release(void *handle) {
  @autoreleasepool {
    Ibex2Start *start = (__bridge_transfer Ibex2Start *)handle;
    (void)start;
  }
}

} // extern "C"
