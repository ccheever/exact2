// LLP 1126.001 WP16: exercise the native migration without a provisioned Keychain.
#import <Foundation/Foundation.h>
#import <Security/Security.h>
#include <cassert>

static OSStatus copy_item(CFDictionaryRef, CFTypeRef *);
static OSStatus add_item(CFDictionaryRef, CFTypeRef *);
static OSStatus update_item(CFDictionaryRef, CFDictionaryRef);
static OSStatus delete_item(CFDictionaryRef);
#define SecItemCopyMatching copy_item
#define SecItemAdd add_item
#define SecItemUpdate update_item
#define SecItemDelete delete_item
#include "../../../../vendor/ibex/crates/ibex2/src/secrets/darwin_keychain.mm"
#undef SecItemCopyMatching
#undef SecItemAdd
#undef SecItemUpdate
#undef SecItemDelete

static NSMutableDictionary<NSString *, NSData *> *login_items, *shared_items;
static bool fail_add, fail_delete;
static int migrations;
static NSData *bytes(NSString *value) { return [value dataUsingEncoding:NSUTF8StringEncoding]; }
static NSMutableDictionary *store(NSDictionary *query) {
  return [query[(__bridge id)kSecUseDataProtectionKeychain] boolValue] ? shared_items : login_items;
}
static OSStatus copy_item(CFDictionaryRef raw, CFTypeRef *result) {
  NSDictionary *query = (__bridge NSDictionary *)raw;
  assert([query[(__bridge id)kSecAttrService] isEqual:@"dev.exact.migration"]);
  if ([query[(__bridge id)kSecMatchLimit] isEqual:(__bridge id)kSecMatchLimitAll]) {
    ++migrations;
    assert(store(query) == login_items);
    if (!login_items.count) return errSecItemNotFound;
    NSMutableArray *items = [NSMutableArray new];
    for (NSString *name in login_items) [items addObject:@{(__bridge id)kSecAttrAccount : name,
        (__bridge id)kSecValueData : login_items[name]}];
    *result = (__bridge_retained CFTypeRef)items;
  } else {
    NSData *value = store(query)[query[(__bridge id)kSecAttrAccount]];
    if (!value) return errSecItemNotFound;
    *result = (__bridge_retained CFTypeRef)value;
  }
  return errSecSuccess;
}
static OSStatus add_item(CFDictionaryRef raw, CFTypeRef *) {
  NSDictionary *query = (__bridge NSDictionary *)raw;
  assert(store(query) == shared_items);
  assert([query[(__bridge id)kSecAttrAccessGroup] isEqual:@"TEAM.dev.exact.migration"]);
  assert([query[(__bridge id)kSecAttrAccessible] isEqual:(__bridge id)kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly]);
  if (fail_add) return errSecAuthFailed;
  NSString *name = query[(__bridge id)kSecAttrAccount];
  if (shared_items[name]) return errSecDuplicateItem;
  shared_items[name] = query[(__bridge id)kSecValueData];
  return errSecSuccess;
}
static OSStatus update_item(CFDictionaryRef, CFDictionaryRef) { assert(false); return errSecUnimplemented; }
static OSStatus delete_item(CFDictionaryRef raw) {
  NSDictionary *query = (__bridge NSDictionary *)raw;
  assert(store(query) == login_items);
  if (fail_delete) return errSecInteractionNotAllowed;
  [login_items removeObjectForKey:query[(__bridge id)kSecAttrAccount]];
  return errSecSuccess;
}
int main(int argc, char **argv) {
  @autoreleasepool {
    assert(argc == 2);
    login_items = [@{@"session" : bytes(@"old"), @"updated" : bytes(@"stale")} mutableCopy];
    shared_items = [@{@"updated" : bytes(@"new")} mutableCopy];
    unsigned char *value = nullptr; size_t length = 0; char *error = nullptr;
    assert(ibex2_darwin_keychain_prepare("dev.exact.migration", &error) == 0 && error == nullptr);
    assert(migrations == (strcmp(argv[1], "app") == 0 ? 1 : 0));
    int status = ibex2_darwin_keychain_get("dev.exact.migration", "session", &value, &length, &error);
    if (strcmp(argv[1], "extension") == 0) {
      assert(status == 1 && migrations == 0 && login_items.count == 2);
      return 0;
    }
    assert(status == 0 && error == nullptr && length == 3 && memcmp(value, "old", 3) == 0);
    free(value);
    if (strcmp(argv[1], "ordinary") == 0) {
      assert(migrations == 0 && login_items.count == 2);
      return 0;
    }
    assert(migrations == 1 && login_items.count == 0);
    assert([shared_items[@"updated"] isEqual:bytes(@"new")]);
    assert(prepare_service("dev.exact.migration") == errSecSuccess && migrations == 1);
    login_items[@"retry"] = bytes(@"retained"); fail_add = true;
    assert(migrate_login_items(@"dev.exact.migration", @"TEAM.dev.exact.migration") == errSecAuthFailed);
    assert(login_items[@"retry"] && !shared_items[@"retry"]);
    fail_add = false; fail_delete = true;
    assert(migrate_login_items(@"dev.exact.migration", @"TEAM.dev.exact.migration") == errSecInteractionNotAllowed);
    assert(login_items[@"retry"] && shared_items[@"retry"]);
    shared_items[@"retry"] = bytes(@"newer"); fail_delete = false;
    assert(migrate_login_items(@"dev.exact.migration", @"TEAM.dev.exact.migration") == errSecSuccess);
    assert(!login_items.count && [shared_items[@"retry"] isEqual:bytes(@"newer")]);
    assert(migrate_login_items(@"dev.exact.migration", @"TEAM.dev.exact.migration") == errSecSuccess);
  }
}
