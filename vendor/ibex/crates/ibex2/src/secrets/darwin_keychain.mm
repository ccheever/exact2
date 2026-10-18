// The Keychain behind the SecretStore trait: the platform half of LLP 0069 §3.
//
// Generic-password items: kSecAttrService is the app's identity, kSecAttrAccount
// the secret's name, the value the data. `set` updates an existing item and
// adds one when there is none; `forget` treats "not found" as done.
//
// iOS: the data-protection keychain, AfterFirstUnlockThisDeviceOnly — readable
// after the first unlock (a background fetch can use it), never restored to
// another device from a backup. macOS normally uses the login keychain,
// whose ACL trusts the creating app's designated requirement. A provisioned
// widget cohort uses its explicit restricted data-protection access group
// instead (LLP 1126.001 WP16). At provisioned app startup it migrates the
// service's login items, deleting each only after a successful copy. The
// extension never reads or migrates the login keychain.
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
  NSBundle *bundle = [NSBundle mainBundle];
  NSString *owner = [bundle objectForInfoDictionaryKey:@"ExactAppIdentifier"];
  NSString *group = [bundle objectForInfoDictionaryKey:@"ExactKeychainAccessGroup"];
  if (group.length && [owner isEqualToString:query[(__bridge id)kSecAttrService]]) {
    query[(__bridge id)kSecAttrAccessGroup] = group;
#if TARGET_OS_OSX
    // A provisioned widget cohort shares the restricted data-protection
    // Keychain. The login Keychain's ACL belongs to the creating app alone.
    query[(__bridge id)kSecUseDataProtectionKeychain] = @YES;
#endif
  }
  return query;
}

#if TARGET_OS_OSX
OSStatus migrate_login_items(NSString *service, NSString *group) {
  NSMutableDictionary *query = [@{(__bridge id)kSecClass : (__bridge id)kSecClassGenericPassword,
      (__bridge id)kSecAttrService : service, (__bridge id)kSecUseDataProtectionKeychain : @NO,
      (__bridge id)kSecReturnAttributes : @YES, (__bridge id)kSecReturnData : @YES,
      (__bridge id)kSecMatchLimit : (__bridge id)kSecMatchLimitAll} mutableCopy];
  CFTypeRef result = nullptr;
  OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &result);
  if (status == errSecItemNotFound) return errSecSuccess;
  if (status != errSecSuccess) return status;
  NSArray *items = (__bridge_transfer NSArray *)result;
  for (NSDictionary *item in items) {
    NSString *account = item[(__bridge id)kSecAttrAccount];
    NSData *value = item[(__bridge id)kSecValueData];
    if (account == nil || value == nil) return errSecDecode;
    NSDictionary *destination = @{(__bridge id)kSecClass : (__bridge id)kSecClassGenericPassword,
        (__bridge id)kSecAttrService : service, (__bridge id)kSecAttrAccount : account,
        (__bridge id)kSecAttrAccessGroup : group, (__bridge id)kSecUseDataProtectionKeychain : @YES,
        (__bridge id)kSecAttrAccessible : (__bridge id)kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
        (__bridge id)kSecValueData : value};
    status = SecItemAdd((__bridge CFDictionaryRef)destination, nullptr);
    // A newer shared item wins over an old login item, including after a
    // crash between the prior copy and delete. Never overwrite it.
    if (status != errSecSuccess && status != errSecDuplicateItem) return status;
    NSDictionary *old = @{(__bridge id)kSecClass : (__bridge id)kSecClassGenericPassword,
        (__bridge id)kSecAttrService : service, (__bridge id)kSecAttrAccount : account,
        (__bridge id)kSecUseDataProtectionKeychain : @NO};
    status = SecItemDelete((__bridge CFDictionaryRef)old);
    if (status != errSecSuccess && status != errSecItemNotFound) return status;
  }
  return errSecSuccess;
}

OSStatus prepare_service(const char *service) {
  NSBundle *bundle = NSBundle.mainBundle;
  NSString *owner = [bundle objectForInfoDictionaryKey:@"ExactAppIdentifier"];
  NSString *group = [bundle objectForInfoDictionaryKey:@"ExactKeychainAccessGroup"];
  if (!group.length || ![owner isEqualToString:[NSString stringWithUTF8String:service]] ||
      ![bundle.bundlePath.pathExtension isEqualToString:@"app"]) return errSecSuccess;
  static NSMutableSet<NSString *> *completed;
  static dispatch_once_t once;
  dispatch_once(&once, ^{ completed = [NSMutableSet new]; });
  @synchronized(completed) {
    NSString *key = [NSString stringWithFormat:@"%@/%@", group, owner];
    if ([completed containsObject:key]) return errSecSuccess;
    OSStatus status = migrate_login_items(owner, group);
    if (status == errSecSuccess) [completed addObject:key];
    return status;
  }
}
#else
OSStatus prepare_service(const char *) { return errSecSuccess; }
#endif

} // namespace

extern "C" {

int ibex2_darwin_keychain_prepare(const char *service, char **out_error) {
  *out_error = nullptr;
  @autoreleasepool {
    OSStatus status = prepare_service(service);
    if (status == errSecSuccess) return 0;
    *out_error = status_message(status);
    return -1;
  }
}

int ibex2_darwin_keychain_get(const char *service, const char *name, unsigned char **out_value,
                              size_t *out_len, char **out_error) {
  @autoreleasepool {
    *out_value = nullptr;
    *out_len = 0;
    *out_error = nullptr;
    OSStatus prepared = prepare_service(service);
    if (prepared != errSecSuccess) {
      *out_error = status_message(prepared);
      return -1;
    }
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
    OSStatus prepared = prepare_service(service);
    if (prepared != errSecSuccess) {
      *out_error = status_message(prepared);
      return -1;
    }
    NSData *data = [NSData dataWithBytes:value length:len];
    NSMutableDictionary *query = query_for(service, name);
    NSDictionary *update = @{(__bridge id)kSecValueData : data};
    OSStatus status = SecItemUpdate((__bridge CFDictionaryRef)query, (__bridge CFDictionaryRef)update);
    if (status == errSecItemNotFound) {
      NSMutableDictionary *item = query_for(service, name);
      item[(__bridge id)kSecValueData] = data;
#if TARGET_OS_OSX
      if (item[(__bridge id)kSecUseDataProtectionKeychain])
#endif
      item[(__bridge id)kSecAttrAccessible] =
          (__bridge id)kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly;
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
    OSStatus prepared = prepare_service(service);
    if (prepared != errSecSuccess) {
      *out_error = status_message(prepared);
      return -1;
    }
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
