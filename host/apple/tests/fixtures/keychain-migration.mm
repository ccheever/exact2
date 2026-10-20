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
static NSString *mode;
static bool fail_add, fail_delete;
static int migrations, legacy_reads;
static NSData *bytes(NSString *value) { return [value dataUsingEncoding:NSUTF8StringEncoding]; }
static NSMutableDictionary *store(NSDictionary *query) {
  return [query[(__bridge id)kSecUseDataProtectionKeychain] boolValue] ? shared_items : login_items;
}
static NSString *account(NSDictionary *query) {
  NSData *ref = query[(__bridge id)kSecValuePersistentRef];
  return ref ? [[NSString alloc] initWithData:ref encoding:NSUTF8StringEncoding] : query[(__bridge id)kSecAttrAccount];
}
static OSStatus copy_item(CFDictionaryRef raw, CFTypeRef *result) {
  NSDictionary *query = (__bridge NSDictionary *)raw;
  if ([query[(__bridge id)kSecMatchLimit] isEqual:(__bridge id)kSecMatchLimitAll]) {
    ++migrations;
    assert(store(query) == login_items);
    assert([query[(__bridge id)kSecAttrService] isEqual:@"dev.exact.migration"]);
    assert(![query[(__bridge id)kSecReturnData] boolValue]);
    assert([query[(__bridge id)kSecReturnPersistentRef] boolValue]);
    if ([mode isEqual:@"locked-list"]) return errSecInteractionNotAllowed;
    if (!login_items.count) return errSecItemNotFound;
    NSMutableArray *items = [NSMutableArray new];
    // Put the broken item first so the test detects starvation of later items.
    for (NSString *name in [[login_items allKeys] sortedArrayUsingSelector:@selector(compare:)]) {
      NSMutableDictionary *item = [@{(__bridge id)kSecAttrAccount : name,
          (__bridge id)kSecValuePersistentRef : bytes(name)} mutableCopy];
      if ([name isEqual:@"broken"] && [mode isEqual:@"missing-account"]) [item removeObjectForKey:(__bridge id)kSecAttrAccount];
      [items addObject:item];
    }
    *result = (__bridge_retained CFTypeRef)items;
  } else {
    NSString *name = account(query);
    if (store(query) == login_items) {
      ++legacy_reads;
      if ([name isEqual:@"broken"]) {
        if ([mode isEqual:@"denied"]) return errSecAuthFailed;
        if ([mode isEqual:@"canceled"]) return errSecUserCanceled;
        if ([mode isEqual:@"locked"]) return errSecInteractionNotAllowed;
        if ([mode isEqual:@"decode"]) return errSecDecode;
        if ([mode isEqual:@"missing-data"]) return errSecSuccess;
      }
    }
    NSData *value = store(query)[name];
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
  NSString *name = account(query);
  if (shared_items[name]) return errSecDuplicateItem;
  shared_items[name] = query[(__bridge id)kSecValueData];
  return errSecSuccess;
}
static OSStatus update_item(CFDictionaryRef raw, CFDictionaryRef changes) {
  NSDictionary *query = (__bridge NSDictionary *)raw;
  NSString *name = account(query);
  if (!store(query)[name]) return errSecItemNotFound;
  store(query)[name] = ((__bridge NSDictionary *)changes)[(__bridge id)kSecValueData];
  return errSecSuccess;
}
static OSStatus delete_item(CFDictionaryRef raw) {
  NSDictionary *query = (__bridge NSDictionary *)raw;
  if (store(query) == login_items && fail_delete) return errSecInteractionNotAllowed;
  [store(query) removeObjectForKey:account(query)];
  return errSecSuccess;
}
int main(int argc, char **argv) {
  @autoreleasepool {
    assert(argc == 2);
    mode = [NSString stringWithUTF8String:argv[1]];
    const bool app = ![mode isEqual:@"ordinary"] && ![mode isEqual:@"extension"];
    const bool failure = app && ![mode isEqual:@"app"];
    login_items = [@{@"session" : bytes(@"old"), @"updated" : bytes(@"stale")} mutableCopy];
    if (failure) login_items[@"broken"] = bytes(@"retained");
    shared_items = [@{@"updated" : bytes(@"new")} mutableCopy];
    unsigned char *value = nullptr; size_t length = 0; char *error = nullptr;
    int prepared = ibex2_darwin_keychain_prepare("dev.exact.migration", &error);
    assert(prepared == (failure ? -1 : 0));
    assert((error != nullptr) == failure); free(error);
    assert(migrations == (app ? 1 : 0));
    int status = ibex2_darwin_keychain_get("dev.exact.migration", "session", &value, &length, &error);
    if ([mode isEqual:@"extension"]) {
      assert(status == 1 && migrations == 0 && login_items.count == 2);
      return 0;
    }
    if ([mode isEqual:@"locked-list"]) {
      assert(status == 1 && login_items.count == 3);
    } else {
      assert(status == 0 && error == nullptr && length == 3 && memcmp(value, "old", 3) == 0);
      free(value);
    }
    if ([mode isEqual:@"ordinary"]) {
      assert(migrations == 0 && login_items.count == 2);
      return 0;
    }
    if (failure) assert(login_items[@"broken"] && !shared_items[@"broken"]);
    if (![mode isEqual:@"locked-list"]) assert(login_items.count == (failure ? 1 : 0));
    assert([shared_items[@"updated"] isEqual:bytes(@"new")]);
    const int reads = legacy_reads;
    assert(ibex2_darwin_keychain_set("dev.exact.migration", "fresh", (const unsigned char *)"new", 3, &error) == 0);
    assert([shared_items[@"fresh"] isEqual:bytes(@"new")]);
    assert(ibex2_darwin_keychain_get("dev.exact.migration", "fresh", &value, &length, &error) == 0);
    assert(length == 3 && memcmp(value, "new", 3) == 0); free(value);
    assert(ibex2_darwin_keychain_forget("dev.exact.migration", "fresh", &error) == 0);
    assert(!shared_items[@"fresh"]);
    assert(prepare_service("dev.exact.migration") == errSecSuccess && migrations == 1 && legacy_reads == reads);
    if (failure) return 0;
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
