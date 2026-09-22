// A caller-owned runtime: no ibex2 Hermes owner, loader, or pump.
#include <hermes/hermes.h>
#include "../include/ibex2_jsi.h"
#include <cstdlib>
#include <cstring>

using namespace facebook;
namespace {
struct Bytes : jsi::Buffer {
  std::vector<uint8_t> bytes;
  Bytes(const uint8_t *p, size_t n) : bytes(p, p + n) {}
  size_t size() const override { return bytes.size(); }
  const uint8_t *data() const override { return bytes.data(); }
};
struct Consumer {
  std::unique_ptr<jsi::Runtime> runtime;
  std::unique_ptr<ibex2::jsi_adapter::Adapter> adapter;
};
char *copy(const std::string &s) {
  auto *p = static_cast<char *>(std::malloc(s.size() + 1));
  std::memcpy(p, s.c_str(), s.size() + 1);
  return p;
}
}
extern "C" {
void *storage_consumer_create(const void *queue, const void *grants,
                              const uint8_t *factory, size_t len,
                              const uint8_t *harden, size_t harden_len, char **error) {
  try {
    auto c = std::make_unique<Consumer>();
    auto config = ::hermes::vm::RuntimeConfig::Builder()
        .withEnableEval(false).withMicrotaskQueue(true).build();
    c->runtime = facebook::hermes::makeHermesRuntimeNoThrow(config);
    if (!c->runtime) return nullptr;
    auto &rt = *c->runtime;
    auto names = rt.global().getPropertyAsObject(rt, "Object")
        .getPropertyAsFunction(rt, "getOwnPropertyNames");
    auto stringify = rt.global().getPropertyAsObject(rt, "JSON")
        .getPropertyAsFunction(rt, "stringify");
    auto before = stringify.call(rt, names.call(rt, rt.global())).getString(rt).utf8(rt);
    auto f = rt.evaluateJavaScript(std::make_shared<Bytes>(factory, len), "sqlite.hbc")
        .getObject(rt).getFunction(rt);
    c->adapter = std::make_unique<ibex2::jsi_adapter::Adapter>(rt, queue);
    if (harden_len) rt.evaluateJavaScript(std::make_shared<Bytes>(harden, harden_len), "harden.hbc");
    auto storage = c->adapter->storage(grants, f);
    auto after = stringify.call(rt, names.call(rt, rt.global())).getString(rt).utf8(rt);
    if (before != after) throw jsi::JSError(rt, "installer mutated caller globals");
    // The caller deliberately passes its capability to the test application.
    rt.global().setProperty(rt, "storage", storage);
    return c.release();
  } catch (const std::exception &e) { *error = copy(e.what()); return nullptr; }
}
int storage_consumer_eval(void *h, const uint8_t *bytes, size_t len, char **out) {
  auto &rt = *static_cast<Consumer *>(h)->runtime;
  try {
    auto v = rt.evaluateJavaScript(std::make_shared<Bytes>(bytes, len), "test.hbc");
    *out = copy(v.toString(rt).utf8(rt));
    return 0;
  } catch (const std::exception &e) { *out = copy(e.what()); return 1; }
}
int storage_consumer_step(void *h, bool deliver, char **out) {
  auto *c = static_cast<Consumer *>(h);
  try {
    if (deliver) return c->adapter->deliver_one() ? 1 : 0;
    c->runtime->drainMicrotasks();
    return 0;
  } catch (const std::exception &e) { *out = copy(e.what()); return -1; }
}
void storage_consumer_detach(void *h) { static_cast<Consumer *>(h)->adapter.reset(); }
void storage_consumer_destroy(void *h) { delete static_cast<Consumer *>(h); }
void storage_consumer_free(char *s) { std::free(s); }
}
