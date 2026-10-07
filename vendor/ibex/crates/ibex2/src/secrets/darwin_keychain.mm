// The Keychain behind the SecretStore trait: the platform half of LLP 0069 §3.
//
// Generic-password items: kSecAttrService is the app's identity, kSecAttrAccount
// the secret's name, the value the data. `set` updates an existing item and
// adds one when there is none; `forget` treats "not found" as done.
//
// iOS: the data-protection keychain, AfterFirstUnlockThisDeviceOnly — readable
// after the first unlock (a background fetch can use it), never restored to
// another device from a backup. macOS: the login keychain, on purpose —
// kSecUseDataProtectionKeychain would need an application-identifier
// entitlement (a bundle and a provisioning profile), and the login keychain
// serves a bare signed binary. Its ACL trusts the creating app by its code
// signature's designated requirement, so a consumer that rebuilds must sign
// with a stable identity or be asked again after every build (exact2 LLP 1018
// D7).
//
// @ref LLP 0069#3-the-backends — the Keychain, and why the login keychain on macOS

#import <Foundation/Foundation.h>
#import <Security/Security.h>
#include <TargetConditionals.h>

#include <cstdlib>
#include <cstring>

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

char *status_message(OSStatus status) {
  CFStringRef message = SecCopyErrorMessageString(status, nullptr);
  NSString *text = message != nullptr ? (__bridge_transfer NSString *)message : nil;
  if (text == nil) {
    text = [NSString stringWithFormat:@"OSStatus %d", static_cast<int>(status)];
  }
  return dup_utf8(text);
}

NSMutableDictionary *query_for(const char *service, const char *name) {
  NSMutableDictionary *query = [NSMutableDictionary dictionary];
  query[(__bridge id)kSecClass] = (__bridge id)kSecClassGenericPassword;
  query[(__bridge id)kSecAttrService] = [NSString stringWithUTF8String:service];
  query[(__bridge id)kSecAttrAccount] = [NSString stringWithUTF8String:name];
  return query;
}

} // namespace

extern "C" {

int ibex2_darwin_keychain_get(const char *service, const char *name, unsigned char **out_value,
                              size_t *out_len, char **out_error) {
  @autoreleasepool {
    *out_value = nullptr;
    *out_len = 0;
    *out_error = nullptr;
    NSMutableDictionary *query = query_for(service, name);
    query[(__bridge id)kSecReturnData] = @YES;
    query[(__bridge id)kSecMatchLimit] = (__bridge id)kSecMatchLimitOne;
    CFTypeRef result = nullptr;
    OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &result);
    if (status == errSecItemNotFound) {
      return 1;
    }
    if (status != errSecSuccess) {
      *out_error = status_message(status);
      return -1;
    }
    NSData *data = (__bridge_transfer NSData *)result;
    unsigned char *bytes = static_cast<unsigned char *>(std::malloc(data.length + 1));
    if (bytes == nullptr) {
      *out_error = dup_utf8(@"out of memory");
      return -1;
    }
    std::memcpy(bytes, data.bytes, data.length);
    *out_value = bytes;
    *out_len = data.length;
    return 0;
  }
}

int ibex2_darwin_keychain_set(const char *service, const char *name, const unsigned char *value,
                              size_t len, char **out_error) {
  @autoreleasepool {
    *out_error = nullptr;
    NSData *data = [NSData dataWithBytes:value length:len];
    NSMutableDictionary *query = query_for(service, name);
    NSDictionary *update = @{(__bridge id)kSecValueData : data};
    OSStatus status = SecItemUpdate((__bridge CFDictionaryRef)query, (__bridge CFDictionaryRef)update);
    if (status == errSecItemNotFound) {
      NSMutableDictionary *item = query_for(service, name);
      item[(__bridge id)kSecValueData] = data;
#if TARGET_OS_IPHONE
      item[(__bridge id)kSecAttrAccessible] =
          (__bridge id)kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly;
#endif
      status = SecItemAdd((__bridge CFDictionaryRef)item, nullptr);
    }
    if (status != errSecSuccess) {
      *out_error = status_message(status);
      return -1;
    }
    return 0;
  }
}

int ibex2_darwin_keychain_forget(const char *service, const char *name, char **out_error) {
  @autoreleasepool {
    *out_error = nullptr;
    OSStatus status = SecItemDelete((__bridge CFDictionaryRef)query_for(service, name));
    if (status != errSecSuccess && status != errSecItemNotFound) {
      *out_error = status_message(status);
      return -1;
    }
    return 0;
  }
}

char *ibex2_darwin_bundle_identifier(void) {
  @autoreleasepool {
    return dup_utf8([[NSBundle mainBundle] bundleIdentifier]);
  }
}

} // extern "C"
