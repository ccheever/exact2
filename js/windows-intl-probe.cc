#include <hermes/hermes.h>
#include <jsi/jsi.h>
#include <unicode/udata.h>
#include <fstream>
#include <iostream>
#include <iterator>
int main(int argc, char **argv) {
  if (argc != 2) return 2;
  UErrorCode status = U_ZERO_ERROR;
  udata_setFileAccess(UDATA_NO_FILES, &status);
  if (U_FAILURE(status)) return 3;
  std::ifstream file(argv[1], std::ios::binary);
  std::string bytes((std::istreambuf_iterator<char>(file)), {});
  if (bytes.empty()) return 4;
  try {
    auto config = ::hermes::vm::RuntimeConfig::Builder().withEnableEval(false).build();
    auto rt = facebook::hermes::makeHermesRuntime(config);
    auto value = rt->evaluateJavaScript(std::make_shared<facebook::jsi::StringBuffer>(bytes), "intl-probe.hbc");
    std::cout << value.asString(*rt).utf8(*rt) << std::endl;
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << std::endl;
    return 1;
  }
}
