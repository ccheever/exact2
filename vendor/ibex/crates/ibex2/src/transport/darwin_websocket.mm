// NSURLSessionWebSocketTask behind SocketTransport: the platform half of a
// WebSocket (LLP 0057 §3; LLP 0059.000 §3.12). The platform
// owns TLS with the system trust store, proxies, pings and the closing
// handshake and send queue; Rust owns admission and the message vocabulary.
//
// Each socket has its own ephemeral session (no cookies, no cache) whose
// delegate refuses redirects: a handshake that redirects fails, as it does in
// a browser, and the grant was checked for this origin only. Nothing is ever
// @ref LLP 0057#3-the-boundary — the platform executes; it does not decide
// @ref LLP 0057.000#l4--websocket--completed-2026-10-04 — send does not require replacing the platform task

#import <Foundation/Foundation.h>
#import <Security/Security.h>

#include <cerrno>
#include <cstdlib>
#include <cstring>

namespace {
constexpr NSUInteger kMaxOutboundBytes = 16u << 20;
constexpr NSUInteger kMaxOutboundMessages = 256;

char *dup_utf8(NSString *value) {
  const char *raw = value == nil ? nullptr : [value UTF8String];
  if (raw == nullptr) return nullptr;
  size_t len = std::strlen(raw);
  char *out = static_cast<char *>(std::malloc(len + 1));
  if (out != nullptr) std::memcpy(out, raw, len + 1);
  return out;
}
} // namespace

// Every field is read and written under `condition`, including from the
// session's serial delegate queue.
@interface Ibex2SendGate : NSObject
@property(nonatomic, strong) NSCondition *condition;
@property(nonatomic, assign) BOOL admitted;
@property(nonatomic, assign) BOOL resume;
@property(nonatomic, assign) BOOL closeAttempted;
@property(nonatomic, assign) BOOL closeSubmitted;
@property(nonatomic, assign) BOOL sendSubmittedAfterClose;
@end

@implementation Ibex2SendGate
@end

@interface Ibex2Socket : NSObject <NSURLSessionWebSocketDelegate>
@property(nonatomic, strong) NSCondition *condition;
@property(nonatomic, strong) NSURLSession *session;
@property(nonatomic, strong) NSURLSessionWebSocketTask *task;
@property(nonatomic, assign) BOOL opened;
@property(nonatomic, assign) BOOL ended;
@property(nonatomic, assign) BOOL cancelled;
@property(nonatomic, assign) BOOL closing;
@property(nonatomic, strong) NSString *failure;
@property(nonatomic, strong) NSString *protocol;
@property(nonatomic, assign) NSUInteger pendingBytes;
@property(nonatomic, assign) NSUInteger pendingMessages;
@property(nonatomic, assign) NSInteger requestedCloseCode;
@property(nonatomic, strong) NSData *requestedCloseReason;
// A test-only pinned anchor supplied by the Rust unit-test transport. The
// production entry point always leaves this nil and uses normal system trust.
@property(nonatomic, strong) NSData *testCertificate;
// One received message, or the error that ended the receive.
@property(nonatomic, assign) BOOL waiting;
@property(nonatomic, assign) BOOL received;
@property(nonatomic, strong) NSURLSessionWebSocketMessage *message;
@property(nonatomic, strong) NSError *error;
// Installed only by the concurrent send/close unit test. Atomic publication
// lets the close entry point announce its attempt before taking `condition`.
@property(atomic, strong) Ibex2SendGate *testSendGate;
@end

@implementation Ibex2Socket
- (void)URLSession:(NSURLSession *)session
          webSocketTask:(NSURLSessionWebSocketTask *)task
    didOpenWithProtocol:(NSString *)protocol {
  [self.condition lock];
  self.protocol = protocol ?: @"";
  self.opened = YES;
  [self.condition broadcast];
  [self.condition unlock];
}
- (void)URLSession:(NSURLSession *)session
       webSocketTask:(NSURLSessionWebSocketTask *)task
    didCloseWithCode:(NSURLSessionWebSocketCloseCode)code
              reason:(NSData *)reason {
  [self.condition lock];
  self.ended = YES;
  [self.condition broadcast];
  [self.condition unlock];
}
- (void)URLSession:(NSURLSession *)session
                    task:(NSURLSessionTask *)task
    didCompleteWithError:(NSError *)error {
  [self.condition lock];
  self.ended = YES;
  if (!self.opened && self.failure == nil) {
    NSInteger status = [task.response isKindOfClass:[NSHTTPURLResponse class]]
                           ? ((NSHTTPURLResponse *)task.response).statusCode
                           : 0;
    self.failure = status > 0 ? [NSString stringWithFormat:@"HTTP %ld", (long)status]
                              : (error.localizedDescription ?: @"the connection failed");
  }
  [self.condition broadcast];
  [self.condition unlock];
}
- (void)URLSession:(NSURLSession *)session
                          task:(NSURLSessionTask *)task
    willPerformHTTPRedirection:(NSHTTPURLResponse *)response
                    newRequest:(NSURLRequest *)request
             completionHandler:(void (^)(NSURLRequest *))completionHandler {
  completionHandler(nil);
}
- (void)URLSession:(NSURLSession *)session
    didReceiveChallenge:(NSURLAuthenticationChallenge *)challenge
      completionHandler:(void (^)(NSURLSessionAuthChallengeDisposition,
                                  NSURLCredential *))completionHandler {
  if (self.testCertificate == nil ||
      ![challenge.protectionSpace.authenticationMethod
          isEqualToString:NSURLAuthenticationMethodServerTrust] ||
      challenge.protectionSpace.serverTrust == nil) {
    completionHandler(NSURLSessionAuthChallengePerformDefaultHandling, nil);
    return;
  }
  SecCertificateRef anchor = SecCertificateCreateWithData(
      kCFAllocatorDefault, (__bridge CFDataRef)self.testCertificate);
  if (anchor == nullptr) {
    completionHandler(NSURLSessionAuthChallengeCancelAuthenticationChallenge, nil);
    return;
  }
  NSArray *anchors = @[ (__bridge id)anchor ];
  SecTrustRef trust = challenge.protectionSpace.serverTrust;
  OSStatus setStatus = SecTrustSetAnchorCertificates(
      trust, (__bridge CFArrayRef)anchors);
  if (setStatus == errSecSuccess)
    setStatus = SecTrustSetAnchorCertificatesOnly(trust, true);
  CFErrorRef trustError = nullptr;
  BOOL trusted = setStatus == errSecSuccess &&
                 SecTrustEvaluateWithError(trust, &trustError);
  if (trustError != nullptr) CFRelease(trustError);
  CFRelease(anchor);
  if (trusted) {
    completionHandler(NSURLSessionAuthChallengeUseCredential,
                      [NSURLCredential credentialForTrust:trust]);
  } else {
    completionHandler(NSURLSessionAuthChallengeCancelAuthenticationChallenge, nil);
  }
}
@end

namespace {
void await_test_send(Ibex2SendGate *gate) {
  if (gate == nil) return;
  [gate.condition lock];
  gate.admitted = YES;
  [gate.condition broadcast];
  while (!gate.resume) [gate.condition wait];
  [gate.condition unlock];
}

void note_test_close_attempt(Ibex2SendGate *gate) {
  if (gate == nil) return;
  [gate.condition lock];
  gate.closeAttempted = YES;
  [gate.condition broadcast];
  [gate.condition unlock];
}

void note_test_close_submission(Ibex2SendGate *gate) {
  if (gate == nil) return;
  [gate.condition lock];
  gate.closeSubmitted = YES;
  [gate.condition broadcast];
  [gate.condition unlock];
}

void note_test_send_submission(Ibex2SendGate *gate) {
  if (gate == nil) return;
  [gate.condition lock];
  if (gate.closeSubmitted) gate.sendSubmittedAfterClose = YES;
  [gate.condition unlock];
}
} // namespace

extern "C" {

void *start_socket(const char *url, const char *protocols, size_t max_message,
                   NSData *test_certificate) {
  @autoreleasepool {
    NSURL *target = [NSURL URLWithString:[NSString stringWithUTF8String:url]];
    if (target == nil) return nullptr;
    Ibex2Socket *socket = [[Ibex2Socket alloc] init];
    socket.condition = [[NSCondition alloc] init];
    socket.testCertificate = test_certificate;
    NSURLSessionConfiguration *config = [NSURLSessionConfiguration ephemeralSessionConfiguration];
    config.HTTPCookieStorage = nil;
    config.URLCache = nil;
    config.HTTPShouldSetCookies = NO;
    NSOperationQueue *queue = [[NSOperationQueue alloc] init];
    queue.maxConcurrentOperationCount = 1;
    socket.session = [NSURLSession sessionWithConfiguration:config delegate:socket delegateQueue:queue];
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:target];
    if (protocols != nullptr && protocols[0] != '\0') {
      [request setValue:[NSString stringWithUTF8String:protocols]
          forHTTPHeaderField:@"Sec-WebSocket-Protocol"];
    }
    request.timeoutInterval = 15;
    socket.task = [socket.session webSocketTaskWithRequest:request];
    // One over the ceiling, so a message exactly at it still arrives and
    // anything larger is refused by the platform before it is buffered.
    socket.task.maximumMessageSize = (NSInteger)(max_message < (size_t)NSIntegerMax ? max_message + 1 : max_message);
    [socket.task resume];
    return (__bridge_retained void *)socket;
  }
}

// Start opening `url`; the handle is retained for Rust until release.
void *ibex2_darwin_ws_start(const char *url, const char *protocols,
                            size_t max_message) {
  return start_socket(url, protocols, max_message, nil);
}

// Unit-test seam for a loopback TLS server with a pinned self-signed leaf.
// No production Rust path calls this entry point.
void *ibex2_darwin_ws_start_with_test_certificate(
    const char *url, const char *protocols, size_t max_message,
    const unsigned char *certificate, size_t certificate_len) {
  @autoreleasepool {
    NSData *data = [NSData dataWithBytes:certificate length:certificate_len];
    return start_socket(url, protocols, max_message, data);
  }
}

// Block until the handshake completes. Nonzero on failure, with a message.
int ibex2_darwin_ws_wait_open(void *handle, char **out_error,
                              char **out_protocol) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  [socket.condition lock];
  while (!socket.opened && !socket.ended && !socket.cancelled) [socket.condition wait];
  BOOL opened = socket.opened && !socket.cancelled;
  NSString *failure = socket.cancelled ? @"aborted" : socket.failure;
  [socket.condition unlock];
  if (opened) {
    *out_protocol = dup_utf8(socket.protocol ?: @"");
    return 0;
  }
  *out_error = dup_utf8(failure ?: @"the connection failed");
  return 1;
}

// The next message. Kind: 0 text (`out_data`), 1 binary (`out_len` bytes),
// 2 closed (`out_code`, reason in `out_data`), 3 too large. Nonzero: aborted.
int ibex2_darwin_ws_next(void *handle, int *out_kind, char **out_data, size_t *out_len,
                         int *out_code) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  [socket.condition lock];
  if (socket.cancelled) {
    [socket.condition unlock];
    return 1;
  }
  socket.waiting = YES;
  socket.received = NO;
  socket.message = nil;
  socket.error = nil;
  [socket.condition unlock];
  __weak Ibex2Socket *weak = socket;
  [socket.task receiveMessageWithCompletionHandler:^(NSURLSessionWebSocketMessage *message, NSError *error) {
    Ibex2Socket *strong = weak;
    if (strong == nil) return;
    [strong.condition lock];
    strong.message = message;
    strong.error = error;
    strong.received = YES;
    [strong.condition broadcast];
    [strong.condition unlock];
  }];
  [socket.condition lock];
  while (!socket.received && !socket.cancelled && !socket.ended)
    [socket.condition wait];
  socket.waiting = NO;
  if (socket.cancelled) {
    [socket.condition unlock];
    return 1;
  }
  NSURLSessionWebSocketMessage *message = socket.message;
  NSError *error = socket.error;
  NSInteger requestedCloseCode = socket.requestedCloseCode;
  NSData *requestedCloseReason = socket.requestedCloseReason;
  [socket.condition unlock];
  if (message != nil && message.type == NSURLSessionWebSocketMessageTypeString) {
    *out_kind = 0;
    NSData *bytes = [message.string dataUsingEncoding:NSUTF8StringEncoding];
    *out_len = bytes.length;
    *out_data = static_cast<char *>(std::malloc(bytes.length + 1));
    if (*out_data == nullptr) return 1;
    std::memcpy(*out_data, bytes.bytes, bytes.length);
    (*out_data)[bytes.length] = 0;
    return 0;
  }
  if (message != nil) {
    *out_kind = 1;
    *out_len = message.data.length;
    if (*out_len > 0) {
      *out_data = static_cast<char *>(std::malloc(*out_len));
      if (*out_data == nullptr) return 1;
      std::memcpy(*out_data, message.data.bytes, *out_len);
    }
    return 0;
  }
  // The platform refuses a message over maximumMessageSize with EMSGSIZE.
  if ([error.domain isEqualToString:NSPOSIXErrorDomain] && error.code == EMSGSIZE) {
    *out_kind = 3;
    return 0;
  }
  NSInteger code = socket.task.closeCode;
  if (code == NSURLSessionWebSocketCloseCodeInvalid &&
      requestedCloseCode != 0)
    code = requestedCloseCode;
  *out_kind = 2;
  *out_code = code == NSURLSessionWebSocketCloseCodeInvalid ? 1006 : (int)code;
  NSData *reason = socket.task.closeReason ?: requestedCloseReason;
  if (reason.length > 0) {
    NSString *text = [[NSString alloc] initWithData:reason encoding:NSUTF8StringEncoding];
    *out_data = dup_utf8(text ?: @"");
  }
  return 0;
}

int send_message(Ibex2Socket *socket, NSURLSessionWebSocketMessage *message,
                 size_t length) {
  [socket.condition lock];
  socket.pendingBytes = length > NSUIntegerMax - socket.pendingBytes
      ? NSUIntegerMax : socket.pendingBytes + length;
  BOOL ended = socket.ended || socket.cancelled || socket.closing;
  BOOL full = !ended &&
      (socket.pendingBytes > kMaxOutboundBytes ||
       socket.pendingMessages >= kMaxOutboundMessages);
  if (full) {
    socket.closing = YES;
    socket.requestedCloseCode = 1009;
  }
  if (!ended && !full) socket.pendingMessages += 1;
  // WHATWG keeps bytes handed to a closing/closed socket in bufferedAmount.
  if (ended) {
    [socket.condition unlock];
    return 0;
  }
  if (full) {
    [socket.condition unlock];
    [socket.task cancelWithCloseCode:(NSURLSessionWebSocketCloseCode)1009
                              reason:nil];
    return 0;
  }
  Ibex2SendGate *testGate = socket.testSendGate;
  await_test_send(testGate);
  note_test_send_submission(testGate);
  __weak Ibex2Socket *weak = socket;
  // @ref LLP 0059.000#312-websocket--delegating-capability-bearing-author-required — admission and NSURLSession submission are atomic with close
  [socket.task sendMessage:message completionHandler:^(NSError *error) {
    Ibex2Socket *strong = weak;
    if (strong == nil) return;
    [strong.condition lock];
    if (error == nil) {
      strong.pendingBytes = strong.pendingBytes >= length
                                ? strong.pendingBytes - length
                                : 0;
      strong.pendingMessages = strong.pendingMessages > 0
                                   ? strong.pendingMessages - 1
                                   : 0;
    } else {
      strong.error = error;
      strong.ended = YES;
    }
    [strong.condition broadcast];
    [strong.condition unlock];
  }];
  [socket.condition unlock];
  return 0;
}

int ibex2_darwin_ws_send_text(void *handle, const unsigned char *data,
                              size_t len) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  NSData *bytes = [NSData dataWithBytes:data length:len];
  NSString *text = [[NSString alloc] initWithData:bytes
                                         encoding:NSUTF8StringEncoding];
  if (text == nil) return 1;
  return send_message(socket,
      [[NSURLSessionWebSocketMessage alloc] initWithString:text], len);
}

int ibex2_darwin_ws_send_binary(void *handle, const unsigned char *data,
                                size_t len) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  NSData *bytes = [NSData dataWithBytes:data length:len];
  return send_message(socket,
      [[NSURLSessionWebSocketMessage alloc] initWithData:bytes], len);
}

void ibex2_darwin_ws_close(void *handle, int code,
                           const unsigned char *reason, size_t len) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  Ibex2SendGate *testGate = socket.testSendGate;
  note_test_close_attempt(testGate);
  NSData *data = len == 0 ? nil : [NSData dataWithBytes:reason length:len];
  // NSURLSession has no spelling for an empty RFC 6455 close payload. Its
  // `Invalid` enum produces a transport cancellation, not a clean handshake,
  // so the no-argument WHATWG close uses the platform's normal closure.
  NSURLSessionWebSocketCloseCode platformCode =
      code == 0 ? NSURLSessionWebSocketCloseCodeNormalClosure
                : (NSURLSessionWebSocketCloseCode)code;
  [socket.condition lock];
  BOOL already = socket.closing || socket.ended || socket.cancelled;
  socket.closing = YES;
  socket.requestedCloseCode = platformCode;
  socket.requestedCloseReason = data;
  if (already) {
    [socket.condition unlock];
    return;
  }
  note_test_close_submission(testGate);
  [socket.task cancelWithCloseCode:platformCode
                            reason:data];
  [socket.condition unlock];
}

void ibex2_darwin_ws_test_pause_next_send(void *handle) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  Ibex2SendGate *gate = [[Ibex2SendGate alloc] init];
  gate.condition = [[NSCondition alloc] init];
  socket.testSendGate = gate;
}

void ibex2_darwin_ws_test_wait_send_admitted(void *handle) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  Ibex2SendGate *gate = socket.testSendGate;
  [gate.condition lock];
  while (!gate.admitted) [gate.condition wait];
  [gate.condition unlock];
}

void ibex2_darwin_ws_test_wait_close_attempted(void *handle) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  Ibex2SendGate *gate = socket.testSendGate;
  [gate.condition lock];
  while (!gate.closeAttempted) [gate.condition wait];
  [gate.condition unlock];
}

int ibex2_darwin_ws_test_wait_close_submitted(void *handle,
                                               double timeout_seconds) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  Ibex2SendGate *gate = socket.testSendGate;
  NSDate *limit = [NSDate dateWithTimeIntervalSinceNow:timeout_seconds];
  [gate.condition lock];
  while (!gate.closeSubmitted && [gate.condition waitUntilDate:limit]) {}
  BOOL submitted = gate.closeSubmitted;
  [gate.condition unlock];
  return submitted ? 1 : 0;
}

void ibex2_darwin_ws_test_resume_send(void *handle) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  Ibex2SendGate *gate = socket.testSendGate;
  [gate.condition lock];
  gate.resume = YES;
  [gate.condition broadcast];
  [gate.condition unlock];
}

int ibex2_darwin_ws_test_send_submitted_after_close(void *handle) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  Ibex2SendGate *gate = socket.testSendGate;
  [gate.condition lock];
  BOOL raced = gate.sendSubmittedAfterClose;
  [gate.condition unlock];
  return raced ? 1 : 0;
}

size_t ibex2_darwin_ws_buffered_amount(void *handle) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  [socket.condition lock];
  NSUInteger amount = socket.pendingBytes;
  [socket.condition unlock];
  return (size_t)amount;
}

// Close the socket (idempotent); a blocked open or read returns aborted.
void ibex2_darwin_ws_cancel(void *handle) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  [socket.condition lock];
  BOOL already = socket.cancelled;
  socket.cancelled = YES;
  socket.closing = YES;
  [socket.condition broadcast];
  [socket.condition unlock];
  if (!already) [socket.task cancel];
}

// Close and release Rust's reference; the session ends with it.
void ibex2_darwin_ws_release(void *handle) {
  Ibex2Socket *socket = (__bridge_transfer Ibex2Socket *)handle;
  [socket.condition lock];
  BOOL already = socket.cancelled;
  socket.cancelled = YES;
  socket.closing = YES;
  [socket.condition broadcast];
  [socket.condition unlock];
  if (!already) [socket.task cancelWithCloseCode:NSURLSessionWebSocketCloseCodeNormalClosure reason:nil];
  [socket.session invalidateAndCancel];
}

void ibex2_darwin_ws_free(void *value) { std::free(value); }

} // extern "C"
