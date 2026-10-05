// Windows-only ICU locale casing; included inside hermes::platform_intl.
namespace {
vm::CallResult<std::u16string> exactLocaleCase(
    vm::Runtime &runtime,
    const std::vector<std::u16string> &locales,
    const std::u16string &text,
    bool upper) {
  auto requested =
      impl_icu::LocaleBCP47Object::canonicalizeLocaleList(runtime, locales);
  if (requested == vm::ExecutionStatus::EXCEPTION)
    return vm::ExecutionStatus::EXCEPTION;
  const auto locale = requested->empty()
      ? std::string(uloc_getDefault())
      : impl_icu::toUTF8ASCII(requested->front().getCanonicalizedLocaleId());
  if (text.size() > static_cast<size_t>(INT32_MAX))
    return runtime.raiseRangeError("Intl locale casing input is too long");
  auto operation = upper ? u_strToUpper : u_strToLower;
  UErrorCode status = U_ZERO_ERROR;
  const int32_t length = operation(
      nullptr, 0, text.data(), static_cast<int32_t>(text.size()),
      locale.c_str(), &status);
  if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status))
    return runtime.raiseError("Intl locale casing ICU measurement failed");
  std::u16string result(static_cast<size_t>(length), u'\0');
  status = U_ZERO_ERROR;
  const int32_t written = operation(
      result.data(), length, text.data(), static_cast<int32_t>(text.size()),
      locale.c_str(), &status);
  if (U_FAILURE(status) || written != length)
    return runtime.raiseError("Intl locale casing ICU conversion failed");
  return result;
}
} // namespace

vm::CallResult<std::u16string> toLocaleLowerCase(
    vm::Runtime &runtime,
    const std::vector<std::u16string> &locales,
    const std::u16string &text) {
  return exactLocaleCase(runtime, locales, text, false);
}
vm::CallResult<std::u16string> toLocaleUpperCase(
    vm::Runtime &runtime,
    const std::vector<std::u16string> &locales,
    const std::u16string &text) {
  return exactLocaleCase(runtime, locales, text, true);
}
