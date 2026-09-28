// NSURLSessionWebSocketTask behind SocketTransport: the platform half of a
// listening WebSocket (LLP 0057 §3; Exact's LLP 1016.000 D3). The platform
// owns TLS with the system trust store, proxies, pings and the closing
// handshake; Rust owns the grant check and the message vocabulary.
//
// Each socket has its own ephemeral session (no cookies, no cache) whose
// delegate refuses redirects: a handshake that redirects fails, as it does in
// a browser, and the grant was checked for this origin only. Nothing is ever
// sent: the task is only asked to receive.
//
// @ref LLP 0057#3-the-boundary — the platform executes; it does not decide

#import <Foundation/Foundation.h>

#include <cerrno>
#include <cstdlib>
#include <cstring>

namespace {
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
@interface Ibex2Socket : NSObject <NSURLSessionWebSocketDelegate>
@property(nonatomic, strong) NSCondition *condition;
@property(nonatomic, strong) NSURLSession *session;
@property(nonatomic, strong) NSURLSessionWebSocketTask *task;
@property(nonatomic, assign) BOOL opened;
@property(nonatomic, assign) BOOL ended;
@property(nonatomic, assign) BOOL cancelled;
@property(nonatomic, strong) NSString *failure;
// One received message, or the error that ended the receive.
@property(nonatomic, assign) BOOL waiting;
@property(nonatomic, assign) BOOL received;
@property(nonatomic, strong) NSURLSessionWebSocketMessage *message;
@property(nonatomic, strong) NSError *error;
@end

@implementation Ibex2Socket
- (void)URLSession:(NSURLSession *)session
          webSocketTask:(NSURLSessionWebSocketTask *)task
    didOpenWithProtocol:(NSString *)protocol {
  [self.condition lock];
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
@end

extern "C" {

// Start opening `url`; the handle is retained for Rust until release.
void *ibex2_darwin_ws_start(const char *url, size_t max_message) {
  @autoreleasepool {
    NSURL *target = [NSURL URLWithString:[NSString stringWithUTF8String:url]];
    if (target == nil) return nullptr;
    Ibex2Socket *socket = [[Ibex2Socket alloc] init];
    socket.condition = [[NSCondition alloc] init];
    NSURLSessionConfiguration *config = [NSURLSessionConfiguration ephemeralSessionConfiguration];
    config.HTTPCookieStorage = nil;
    config.URLCache = nil;
    config.HTTPShouldSetCookies = NO;
    NSOperationQueue *queue = [[NSOperationQueue alloc] init];
    queue.maxConcurrentOperationCount = 1;
    socket.session = [NSURLSession sessionWithConfiguration:config delegate:socket delegateQueue:queue];
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:target];
    request.timeoutInterval = 15;
    socket.task = [socket.session webSocketTaskWithRequest:request];
    // One over the ceiling, so a message exactly at it still arrives and
    // anything larger is refused by the platform before it is buffered.
    socket.task.maximumMessageSize = (NSInteger)(max_message < (size_t)NSIntegerMax ? max_message + 1 : max_message);
    [socket.task resume];
    return (__bridge_retained void *)socket;
  }
}

// Block until the handshake completes. Nonzero on failure, with a message.
int ibex2_darwin_ws_wait_open(void *handle, char **out_error) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  [socket.condition lock];
  while (!socket.opened && !socket.ended && !socket.cancelled) [socket.condition wait];
  BOOL opened = socket.opened && !socket.cancelled;
  NSString *failure = socket.cancelled ? @"aborted" : socket.failure;
  [socket.condition unlock];
  if (opened) return 0;
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
  while (!socket.received && !socket.cancelled) [socket.condition wait];
  socket.waiting = NO;
  if (socket.cancelled) {
    [socket.condition unlock];
    return 1;
  }
  NSURLSessionWebSocketMessage *message = socket.message;
  NSError *error = socket.error;
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
    return 0;
  }
  // The platform refuses a message over maximumMessageSize with EMSGSIZE.
  if ([error.domain isEqualToString:NSPOSIXErrorDomain] && error.code == EMSGSIZE) {
    *out_kind = 3;
    return 0;
  }
  NSInteger code = socket.task.closeCode;
  *out_kind = 2;
  *out_code = code == NSURLSessionWebSocketCloseCodeInvalid ? 1006 : (int)code;
  NSData *reason = socket.task.closeReason;
  if (reason.length > 0) {
    NSString *text = [[NSString alloc] initWithData:reason encoding:NSUTF8StringEncoding];
    *out_data = dup_utf8(text ?: @"");
  }
  return 0;
}

// Close the socket (idempotent); a blocked open or read returns aborted.
void ibex2_darwin_ws_cancel(void *handle) {
  Ibex2Socket *socket = (__bridge Ibex2Socket *)handle;
  [socket.condition lock];
  BOOL already = socket.cancelled;
  socket.cancelled = YES;
  [socket.condition broadcast];
  [socket.condition unlock];
  if (!already) [socket.task cancelWithCloseCode:NSURLSessionWebSocketCloseCodeNormalClosure reason:nil];
}

// Close and release Rust's reference; the session ends with it.
void ibex2_darwin_ws_release(void *handle) {
  Ibex2Socket *socket = (__bridge_transfer Ibex2Socket *)handle;
  [socket.condition lock];
  BOOL already = socket.cancelled;
  socket.cancelled = YES;
  [socket.condition broadcast];
  [socket.condition unlock];
  if (!already) [socket.task cancelWithCloseCode:NSURLSessionWebSocketCloseCodeNormalClosure reason:nil];
  [socket.session invalidateAndCancel];
}

void ibex2_darwin_ws_free(void *value) { std::free(value); }

} // extern "C"
