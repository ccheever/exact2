#include <hermes/hermes.h>
#include <jsi/jsi.h>
#include <fstream>
#include <iostream>
int main(int argc, char**argv) {
 auto rt=facebook::hermes::makeHermesRuntimeNoThrow(::hermes::vm::RuntimeConfig::Builder().withEnableEval(false).withMicrotaskQueue(true).build());
 std::ifstream f(argv[1], std::ios::binary); std::string code((std::istreambuf_iterator<char>(f)),{});
 rt->evaluateJavaScript(std::make_shared<facebook::jsi::StringBuffer>(code),"repro");
 rt->drainMicrotasks();
 std::cout << rt->global().getProperty(*rt,"result").toString(*rt).utf8(*rt) << std::endl;
}
