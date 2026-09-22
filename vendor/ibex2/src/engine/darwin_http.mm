// NSURLSession behind the Transport trait: the platform half of LLP 0057 §3.
//
// What this buys, none of which Rust should reimplement: TLS with the system
// certificate store, system and per-network proxy configuration, VPN
// awareness, HTTP/2 and /3, and connection pooling.
//
// THE CRITICAL DETAIL: NSURLSession follows redirects by default, and it must
// not. Redirect policy is Rust's (LLP 0059.000 §3.5), and — far more
// importantly — Rust re-checks the net.fetch grant on every hop. If the
// platform followed redirects internally, a grant for a.example would silently
// deliver a response from b.example, reintroducing exactly the redirect
// laundering the semantics layer exists to prevent. The delegate below refuses
// every redirect by completing with nil, so the 3xx comes back to Rust to
// decide on.
//
// @ref LLP 0057#3-the-boundary — the platform executes; it does not decide

#import <Foundation/Foundation.h>

#include <cstdlib>
#include <cstring>
#include <string>

namespace {

char *dup_utf8(NSString *value) {
  if (value == nil) {
    return nullptr;
  }
  const char *raw = [value UTF8String];
  if (raw == nullptr) {
    return nullptr;
  }
  size_t len = std::strlen(raw);
  char *out = static_cast<char *>(std::malloc(len + 1));
  if (out != nullptr) {
    std::memcpy(out, raw, len + 1);
  }
  return out;
}

} // namespace

// Four serial delegate queues provide bounded native concurrency. Each session
// is leased until native completion; its connection pool survives.
@interface Ibex2SessionPool : NSObject
@property(nonatomic, strong) NSCondition *condition;
@property(nonatomic, strong) NSMutableArray<NSURLSession *> *idle;
@property(nonatomic, assign) NSUInteger count;
@end
@implementation Ibex2SessionPool
- (instancetype)init {
  if ((self = [super init])) { _condition = [[NSCondition alloc] init]; _idle = [NSMutableArray array]; }
  return self;
}
- (void)returnSession:(NSURLSession *)session {
  [self.condition lock];
  [self.idle addObject:session];
  [self.condition broadcast];
  [self.condition unlock];
}
- (void)dealloc { for (NSURLSession *session in _idle) [session finishTasksAndInvalidate]; }
@end

// A task owns its exchange until completion; Rust independently retains the
// exchange until its body and cancellation registration are both gone. Every
// state access uses condition, including callbacks on the shared delegate queue.
@interface Ibex2Exchange : NSObject
@property(nonatomic, strong) NSCondition *condition;
@property(nonatomic, strong) NSMutableData *bytes;
@property(nonatomic, strong) NSHTTPURLResponse *response;
@property(nonatomic, strong) NSString *failure;
@property(nonatomic, strong) NSURLSessionDataTask *task;
@property(nonatomic, assign) NSUInteger limit;
@property(nonatomic, assign) NSUInteger received;
@property(nonatomic, assign) BOOL finished;
@property(nonatomic, strong) Ibex2SessionPool *pool;
@property(nonatomic, strong) NSURLSession *session;
@property(nonatomic, strong) NSURLRequest *request;
@property(nonatomic, assign) BOOL nativeComplete;
@property(nonatomic, assign) int reused;
@end

@implementation Ibex2Exchange
- (instancetype)initWithLimit:(NSUInteger)limit {
  if ((self = [super init])) {
    _condition = [[NSCondition alloc] init];
    _bytes = [NSMutableData data];
    _limit = limit;
    _reused = -1;
  }
  return self;
}
// Caller holds condition. Recycle only after all old callbacks have finished.
// The exchange owns any buffered bytes independently of the pooled session.
- (void)recycle {
  if (self.nativeComplete && self.session != nil) {
    [self.pool returnSession:self.session];
    self.session = nil;
  }
}
// Caller holds condition. Error takes precedence over already buffered bytes.
- (void)finishWith:(NSString *)failure {
  if (!self.finished) {
    self.failure = failure;
    self.finished = YES;
    [self.condition broadcast];
  }
}
@end

@interface Ibex2NoRedirect : NSObject <NSURLSessionDataDelegate>
@property(nonatomic, strong) NSMutableDictionary<NSNumber *, Ibex2Exchange *> *exchanges;
@property(nonatomic, strong) NSLock *lock;
@end

@implementation Ibex2NoRedirect
- (instancetype)init {
  if ((self = [super init])) {
    _exchanges = [NSMutableDictionary dictionary];
    _lock = [[NSLock alloc] init];
  }
  return self;
}
- (void)track:(Ibex2Exchange *)exchange for:(NSUInteger)identifier {
  [self.lock lock];
  self.exchanges[@(identifier)] = exchange;
  [self.lock unlock];
}
- (Ibex2Exchange *)exchangeFor:(NSUInteger)identifier {
  [self.lock lock];
  Ibex2Exchange *exchange = self.exchanges[@(identifier)];
  [self.lock unlock];
  return exchange;
}
- (void)URLSession:(NSURLSession *)session
              dataTask:(NSURLSessionDataTask *)dataTask
    didReceiveResponse:(NSURLResponse *)response
     completionHandler:(void (^)(NSURLSessionResponseDisposition))completionHandler {
  Ibex2Exchange *exchange = [self exchangeFor:dataTask.taskIdentifier];
  [exchange.condition lock];
  if (exchange == nil || exchange.finished) {
    [exchange.condition unlock];
    completionHandler(NSURLSessionResponseCancel);
    return;
  }
  BOOL isHTTP = [response isKindOfClass:[NSHTTPURLResponse class]];
  NSInteger status = isHTTP ? ((NSHTTPURLResponse *)response).statusCode : 0;
  // HEAD and null-body statuses describe a representation's length, not bytes
  // to receive. Enforce declared length only when the response has a body.
  BOOL hasBody = ![exchange.request.HTTPMethod isEqualToString:@"HEAD"] &&
      status >= 200 && status != 204 && status != 205 && status != 304;
  if (!isHTTP) {
    [exchange finishWith:@"TypeError: Failed to fetch — no response"];
  } else if (hasBody && response.expectedContentLength >= 0 &&
             (unsigned long long)response.expectedContentLength > exchange.limit) {
    [exchange finishWith:[NSString stringWithFormat:
        @"TypeError: Failed to fetch — response exceeded the %lu-byte limit",
        (unsigned long)exchange.limit]];
  } else {
    exchange.response = (NSHTTPURLResponse *)response;
    [exchange.condition broadcast];
  }
  BOOL failed = exchange.finished;
  [exchange.condition unlock];
  // Prefetch through the bounded handoff: small completed bodies return their
  // lease without forcing a consumer to read them first.
  completionHandler(failed ? NSURLSessionResponseCancel : NSURLSessionResponseAllow);
}
- (void)URLSession:(NSURLSession *)session
          dataTask:(NSURLSessionDataTask *)dataTask didReceiveData:(NSData *)data {
  Ibex2Exchange *exchange = [self exchangeFor:dataTask.taskIdentifier];
  [exchange.condition lock];
  if (exchange == nil || exchange.finished) {
    [exchange.condition unlock];
    return;
  }
  const NSUInteger capacity = 64 * 1024;
  if (data.length > exchange.limit - exchange.received) {
    [exchange finishWith:[NSString stringWithFormat:
        @"TypeError: Failed to fetch — response exceeded the %lu-byte limit",
        (unsigned long)exchange.limit]];
    [dataTask cancel];
  } else {
    exchange.received += data.length;
    NSUInteger offset = 0;
    while (offset < data.length && !exchange.finished) {
      while (exchange.bytes.length == capacity && !exchange.finished)
        [exchange.condition wait];
      if (exchange.finished) break;
      NSUInteger amount = MIN(data.length - offset, capacity - exchange.bytes.length);
      [exchange.bytes appendBytes:(const unsigned char *)data.bytes + offset length:amount];
      offset += amount;
      [exchange.condition broadcast];
    }
  }
  [exchange.condition unlock];
}
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task
    didCompleteWithError:(NSError *)error {
  Ibex2Exchange *exchange = [self exchangeFor:task.taskIdentifier];
  [exchange.condition lock];
  NSString *failure = error == nil ? nil : [NSString stringWithFormat:
      @"TypeError: Failed to fetch — %@", error.localizedDescription];
  if (failure == nil && exchange.response == nil)
    failure = @"TypeError: Failed to fetch — no response";
  exchange.task = nil;
  [self.lock lock];
  [self.exchanges removeObjectForKey:@(task.taskIdentifier)];
  [self.lock unlock];
  exchange.nativeComplete = YES;
  [exchange recycle];
  // Publish EOF after the lease is returned, so sequential requests can reuse
  // the completed connection without racing the final delegate bookkeeping.
  [exchange finishWith:failure];
  [exchange.condition unlock];
}
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task
    didFinishCollectingMetrics:(NSURLSessionTaskMetrics *)metrics {
  Ibex2Exchange *exchange = [self exchangeFor:task.taskIdentifier];
  NSURLSessionTaskTransactionMetrics *last = metrics.transactionMetrics.lastObject;
  [exchange.condition lock];
  if (last != nil) exchange.reused = last.reusedConnection ? 1 : 0;
  [exchange.condition unlock];
}
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task
    willPerformHTTPRedirection:(NSHTTPURLResponse *)response
                    newRequest:(NSURLRequest *)request
             completionHandler:(void (^)(NSURLRequest *))completionHandler {
  completionHandler(nil);
}
@end

extern "C" {

/// Each runtime keeps up to four native sessions. A session is reused after
/// the previous native task finishes, preserving its platform connection pool.
/// Serial delegate queues preserve per-task callback order while the four
/// leases bound native callback concurrency and application handoff buffers.
/// @ref LLP 0057#3-the-boundary — pooling is the platform's job
static NSURLSession *new_session(void) {
  @autoreleasepool {
    NSURLSessionConfiguration *config =
        [NSURLSessionConfiguration ephemeralSessionConfiguration];

    // Ephemeral means "not on disk", NOT "no state". Apple still gives the
    // session a private in-memory cookie jar and URL cache, and sharing one
    // session across a runtime is exactly what makes them live long enough to
    // matter — with a session per request they were destroyed each time.
    //
    // Cookies are ambient authority the grant check cannot see: `net.fetch` is
    // granted per *origin*, while cookies are RFC 6265 *domain*-scoped, so a
    // module granted evil.example.com could set a cookie for example.com that
    // the platform then attaches to another module's request to
    // app.example.com. v1 has no credentials mode (LLP 0059.000 §3.5), so the
    // correct number of cookies is zero. Ibex 1 already did this
    // (src/engine/native_fetch_macos.mm).
    config.HTTPCookieAcceptPolicy = NSHTTPCookieAcceptPolicyNever;
    config.HTTPCookieStorage = nil;
    config.HTTPShouldSetCookies = NO;

    // The response cache goes for the same reason and one more: v1 has no
    // cache mode either, so a cached response is a result Rust's semantics
    // layer never decided to serve. It also keeps "did the connection get
    // reused" answerable — a URL cache answers a repeat request without any
    // connection at all, which would make the transport look fast for the
    // wrong reason.
    config.URLCache = nil;
    config.requestCachePolicy = NSURLRequestReloadIgnoringLocalCacheData;

    // One serial delegate per session; leases never overlap on a session.
    Ibex2NoRedirect *delegate = [[Ibex2NoRedirect alloc] init];
    NSURLSession *session = [NSURLSession sessionWithConfiguration:config
                                                         delegate:delegate
                                                    delegateQueue:nil];
    return session;
  }
}

void *ibex2_darwin_session_create(void) {
  @autoreleasepool { return (__bridge_retained void *)[[Ibex2SessionPool alloc] init]; }
}

/// Report whether a session kept a cookie jar or a response cache.
///
/// Exists so `the_session_keeps_no_cookies_and_no_cache` can assert the
/// property instead of trusting the comment above it — the previous version of
/// that comment claimed "no cookie jar, no disk cache" while the session had
/// both in memory.
void ibex2_darwin_session_has_state(void *handle, int *out_cookies,
                                    int *out_cache) {
  *out_cookies = 0;
  *out_cache = 0;
  if (handle == nullptr) {
    return;
  }
  @autoreleasepool {
    NSURLSession *session = new_session();
    NSURLSessionConfiguration *config = session.configuration;
    [session finishTasksAndInvalidate];
    *out_cookies = (config.HTTPCookieStorage != nil ||
                    config.HTTPShouldSetCookies ||
                    config.HTTPCookieAcceptPolicy !=
                        NSHTTPCookieAcceptPolicyNever)
                       ? 1
                       : 0;
    *out_cache = (config.URLCache != nil) ? 1 : 0;
  }
}

/// Drop the transport's pool ownership. Outstanding exchanges retain the pool
/// independently, so dropping the transport cannot invalidate a live body.
void ibex2_darwin_session_destroy(void *handle) {
  if (handle == nullptr) {
    return;
  }
  @autoreleasepool {
    Ibex2SessionPool *pool = (__bridge_transfer Ibex2SessionPool *)handle;
    (void)pool;
  }
}

// Start without waiting, so Rust can register cancellation before waiting for
// headers. Inputs are copied by Foundation before this function returns.
void *ibex2_darwin_http_start(void *session_handle, const char *method,
    const char *url, const char *header_block, const unsigned char *body,
    size_t body_len, size_t max_body, char **out_error) {
  @autoreleasepool {
    *out_error = nullptr;
    NSURL *nsurl = [NSURL URLWithString:[NSString stringWithUTF8String:url]];
    Ibex2SessionPool *pool = (__bridge Ibex2SessionPool *)session_handle;
    if (nsurl == nil || pool == nil) {
      *out_error = dup_utf8(@"TypeError: Failed to fetch — invalid URL or session");
      return nullptr;
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
    if (body_len > 0) request.HTTPBody = [NSData dataWithBytes:body length:body_len];
    Ibex2Exchange *exchange = [[Ibex2Exchange alloc] initWithLimit:max_body];
    exchange.pool = pool;
    exchange.request = request;
    // Rust installs cancellation before resume, including an already aborted signal.
    return (__bridge_retained void *)exchange;
  }
}
int ibex2_darwin_http_headers(void *handle, int *out_status,
    char **out_headers, char **out_error) {
  @autoreleasepool {
    Ibex2Exchange *exchange = (__bridge Ibex2Exchange *)handle;
    *out_headers = nullptr;
    *out_error = nullptr;
    NSURLSession *session = nil;
    while (session == nil) {
      [exchange.condition lock];
      BOOL aborted = exchange.finished;
      [exchange.condition unlock];
      if (aborted) break;
      Ibex2SessionPool *pool = exchange.pool;
      [pool.condition lock];
      if (pool.idle.count != 0) {
        session = pool.idle.lastObject;
        [pool.idle removeLastObject];
      } else if (pool.count < 4) {
        pool.count++;
        [pool.condition unlock];
        session = new_session();
        continue;
      } else {
        // Timed wait also closes the check/wait race with cancellation.
        [pool.condition waitUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
      }
      [pool.condition unlock];
    }
    [exchange.condition lock];
    if (!exchange.finished) {
      exchange.session = session;
      exchange.task = [session dataTaskWithRequest:exchange.request];
      [(Ibex2NoRedirect *)session.delegate track:exchange for:exchange.task.taskIdentifier];
      [exchange.task resume];
    } else {
      if (session != nil) [exchange.pool returnSession:session];
      exchange.nativeComplete = YES;
    }

    while (exchange.response == nil && !exchange.finished) [exchange.condition wait];
    if (exchange.failure != nil) {
      *out_error = dup_utf8(exchange.failure);
      [exchange.condition unlock];
      return 1;
    }
    *out_status = (int)exchange.response.statusCode;
    NSMutableString *headers = [NSMutableString string];
    [exchange.response.allHeaderFields enumerateKeysAndObjectsUsingBlock:
        ^(id key, id value, BOOL *) { [headers appendFormat:@"%@: %@\n", key, value]; }];
    *out_headers = dup_utf8(headers);
    [exchange.condition unlock];
    return 0;
  }
}
int ibex2_darwin_http_read(void *handle, unsigned char *output, size_t capacity,
    size_t *out_length, char **out_error, int *out_reused) {
  @autoreleasepool {
    Ibex2Exchange *exchange = (__bridge Ibex2Exchange *)handle;
    *out_length = 0;
    *out_error = nullptr;
    [exchange.condition lock];
    while (exchange.bytes.length == 0 && !exchange.finished) [exchange.condition wait];
    *out_reused = exchange.reused;
    if (exchange.failure != nil) {
      *out_error = dup_utf8(exchange.failure);
      [exchange.condition unlock];
      return 1;
    }
    size_t count = MIN(capacity, exchange.bytes.length);
    if (count != 0) {
      std::memcpy(output, exchange.bytes.bytes, count);
      [exchange.bytes replaceBytesInRange:NSMakeRange(0, count) withBytes:nullptr length:0];
    }
    *out_length = count;
    [exchange.condition broadcast];
    [exchange.condition unlock];
    return 0;
  }
}
void ibex2_darwin_http_cancel(void *handle) {
  @autoreleasepool {
    Ibex2Exchange *exchange = (__bridge Ibex2Exchange *)handle;
    [exchange.condition lock];
    if (!exchange.finished) {
      [exchange finishWith:@"AbortError: The operation was aborted"];
      [exchange.bytes setLength:0];
      [exchange.task cancel];
    }
    [exchange.condition unlock];
    [exchange.pool.condition lock];
    [exchange.pool.condition broadcast];
    [exchange.pool.condition unlock];
  }
}
void ibex2_darwin_http_release(void *handle) {
  @autoreleasepool {
    Ibex2Exchange *exchange = (__bridge_transfer Ibex2Exchange *)handle;
    (void)exchange;
  }
}
void ibex2_darwin_free(void *value) { std::free(value); }
} // extern "C"
