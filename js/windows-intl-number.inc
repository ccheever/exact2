// Windows-only replacement for the pinned ICU NumberFormat placeholder.
// Included by the verified source patch inside hermes::platform_intl.
namespace {
using ExactNfHandle =
    std::unique_ptr<UNumberFormat, decltype(&unum_close)>;
using ExactNfSystem =
    std::unique_ptr<UNumberingSystem, decltype(&unumsys_close)>;
using ExactNfFields =
    std::unique_ptr<UFieldPositionIterator, decltype(&ufieldpositer_close)>;

vm::ExecutionStatus exactNfRefuse(
    vm::Runtime &runtime,
    const char16_t *option) {
  return runtime.raiseRangeError(
      vm::TwineChar16("Intl.NumberFormat: unsupported or invalid ") +
      vm::TwineChar16(option));
}

vm::ExecutionStatus exactNfIcuFailure(
    vm::Runtime &runtime,
    const char *operation,
    UErrorCode status) {
  return runtime.raiseError(
      vm::TwineChar16("Intl.NumberFormat ICU ") +
      vm::TwineChar16(operation) + vm::TwineChar16(": ") +
      vm::TwineChar16(u_errorName(status)));
}

bool exactNfString(
    vm::Runtime &runtime,
    const Options &options,
    const char16_t *name,
    const char16_t *fallback,
    std::u16string &out) {
  const auto it = options.find(name);
  if (it == options.end()) {
    out = fallback;
    return true;
  }
  if (!it->second.isString()) {
    (void)exactNfRefuse(runtime, name);
    return false;
  }
  out = it->second.getString();
  return true;
}

bool exactNfInteger(
    vm::Runtime &runtime,
    const Options &options,
    const char16_t *name,
    int32_t minimum,
    int32_t maximum,
    int32_t fallback,
    int32_t &out) {
  const auto it = options.find(name);
  if (it == options.end()) {
    out = fallback;
    return true;
  }
  if (!it->second.isNumber()) {
    (void)exactNfRefuse(runtime, name);
    return false;
  }
  const double value = it->second.getNumber();
  if (!std::isfinite(value) || value < minimum || value > maximum) {
    (void)exactNfRefuse(runtime, name);
    return false;
  }
  out = static_cast<int32_t>(std::floor(value));
  return true;
}

// A supported system is positional base ten. Algorithmic systems need a
// different ICU formatter and are refused, never silently replaced by latn.
bool exactNfSupportedSystem(const std::u16string &name) {
  if (!isUnicodeExtensionType(name))
    return false;
  const std::string ascii = impl_icu::toUTF8ASCII(name);
  UErrorCode status = U_ZERO_ERROR;
  ExactNfSystem system(unumsys_openByName(ascii.c_str(), &status), unumsys_close);
  return U_SUCCESS(status) && system &&
      !unumsys_isAlgorithmic(system.get()) &&
      unumsys_getRadix(system.get()) == 10 &&
      ascii == unumsys_getName(system.get());
}

struct ExactNfRendered {
  std::u16string text;
  std::vector<Part> parts;
};

class ExactNumberFormatICU final : public NumberFormat {
 public:
  vm::ExecutionStatus initialize(
      vm::Runtime &runtime,
      const std::vector<std::u16string> &locales,
      const Options &options) {
    const auto requested =
        impl_icu::LocaleBCP47Object::canonicalizeLocaleList(runtime, locales);
    if (requested == vm::ExecutionStatus::EXCEPTION)
      return vm::ExecutionStatus::EXCEPTION;

    std::u16string matcher, explicitSystem, style, display, currency;
    if (!exactNfString(runtime, options, u"localeMatcher", u"best fit", matcher) ||
        !exactNfString(runtime, options, u"numberingSystem", u"", explicitSystem) ||
        !exactNfString(runtime, options, u"style", u"decimal", style) ||
        !exactNfString(runtime, options, u"currencyDisplay", u"symbol", display) ||
        !exactNfString(runtime, options, u"currency", u"", currency))
      return vm::ExecutionStatus::EXCEPTION;
    if (matcher != u"lookup" && matcher != u"best fit")
      return exactNfRefuse(runtime, u"localeMatcher");
    if (style != u"decimal" && style != u"percent" && style != u"currency")
      return exactNfRefuse(runtime, u"style");
    if (display != u"symbol" && display != u"code")
      return exactNfRefuse(runtime, u"currencyDisplay");
    for (const auto &entry : {
             std::pair<const char16_t *, const char16_t *>{u"notation", u"standard"},
             {u"currencySign", u"standard"},
             {u"signDisplay", u"auto"},
             {u"roundingMode", u"halfExpand"},
             {u"roundingPriority", u"auto"},
             {u"trailingZeroDisplay", u"auto"}}) {
      std::u16string value;
      if (!exactNfString(runtime, options, entry.first, entry.second, value))
        return vm::ExecutionStatus::EXCEPTION;
      if (value != entry.second)
        return exactNfRefuse(runtime, entry.first);
    }
    for (const auto *name : {u"unit", u"unitDisplay", u"compactDisplay"})
      if (options.count(name))
        return exactNfRefuse(runtime, name);
    int32_t increment;
    if (!exactNfInteger(runtime, options, u"roundingIncrement", 1, 5000, 1, increment))
      return vm::ExecutionStatus::EXCEPTION;
    if (increment != 1)
      return exactNfRefuse(runtime, u"roundingIncrement");

    bool grouping = true;
    const auto groupingIt = options.find(u"useGrouping");
    if (groupingIt != options.end()) {
      if (!groupingIt->second.isBool())
        return exactNfRefuse(runtime, u"useGrouping");
      grouping = groupingIt->second.getBool();
    }
    if (options.count(u"currency")) {
      if (currency.size() != 3)
        return exactNfRefuse(runtime, u"currency");
      for (auto &ch : currency) {
        if (ch >= u'a' && ch <= u'z')
          ch -= u'a' - u'A';
        if (ch < u'A' || ch > u'Z')
          return exactNfRefuse(runtime, u"currency");
      }
    } else if (style == u"currency") {
      return runtime.raiseTypeError("Intl.NumberFormat currency style requires currency");
    }

    Options localeOptions;
    localeOptions.emplace(u"localeMatcher", Option(matcher));
    if (options.count(u"numberingSystem")) {
      if (!exactNfSupportedSystem(explicitSystem))
        return exactNfRefuse(runtime, u"numberingSystem");
      localeOptions.emplace(u"nu", Option(explicitSystem));
    }
    static constexpr std::u16string_view relevant[] = {u"nu"};
    // Retain nu first so an unsupported selected extension can be refused.
    // An explicit option retains the resolver's normal override precedence.
    auto resolved = impl_icu::resolveLocale(
        *requested, localeOptions, relevant,
        [](auto, auto, const auto &) { return true; });
    const auto nu = resolved.resolvedOpts.find(u"nu");
    if (nu != resolved.resolvedOpts.end() &&
        (!nu->second.isString() || !exactNfSupportedSystem(nu->second.getString())))
      return exactNfRefuse(runtime, u"numberingSystem");

    const auto base = impl_icu::toUTF8ASCII(resolved.localeBcp47Object.getLocaleNoExt());
    if (base.size() > static_cast<size_t>(std::numeric_limits<int32_t>::max()))
      return exactNfRefuse(runtime, u"locale");
    UErrorCode status = U_ZERO_ERROR;
    int32_t parsed = 0;
    int32_t size = uloc_forLanguageTag(base.c_str(), nullptr, 0, &parsed, &status);
    if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status))
      return exactNfIcuFailure(runtime, "locale length", status);
    if (size < 0 || size == std::numeric_limits<int32_t>::max() ||
        parsed != static_cast<int32_t>(base.size()))
      return exactNfRefuse(runtime, u"locale");
    std::string locale(static_cast<size_t>(size) + 1, '\0');
    status = U_ZERO_ERROR;
    const int32_t written = uloc_forLanguageTag(
        base.c_str(), &locale[0], size + 1, &parsed, &status);
    if (U_FAILURE(status))
      return exactNfIcuFailure(runtime, "locale", status);
    if (written != size || parsed != static_cast<int32_t>(base.size()))
      return exactNfRefuse(runtime, u"locale");
    locale.resize(size);
    if (nu != resolved.resolvedOpts.end())
      locale += "@numbers=" + impl_icu::toUTF8ASCII(nu->second.getString());
    status = U_ZERO_ERROR;
    ExactNfSystem system(unumsys_open(locale.c_str(), &status), unumsys_close);
    if (U_FAILURE(status) || !system)
      return exactNfIcuFailure(runtime, "numberingSystem", U_FAILURE(status) ? status : U_INTERNAL_PROGRAM_ERROR);
    if (unumsys_isAlgorithmic(system.get()) || unumsys_getRadix(system.get()) != 10)
      return exactNfRefuse(runtime, u"numberingSystem");
    const auto systemName = impl_icu::toUTF16ASCII(unumsys_getName(system.get()));
    if (nu != resolved.resolvedOpts.end() && nu->second.getString() != systemName)
      return exactNfRefuse(runtime, u"numberingSystem");

    int32_t currencyDigits = 2;
    if (style == u"currency") {
      status = U_ZERO_ERROR;
      currencyDigits = ucurr_getDefaultFractionDigits(currency.c_str(), &status);
      if (U_FAILURE(status))
        return exactNfIcuFailure(runtime, "currency digits", status);
      if (currencyDigits < 0 || currencyDigits > 20)
        return exactNfRefuse(runtime, u"currency digits");
    }
    int32_t integerDigits, minDigits, maxDigits;
    if (!exactNfInteger(runtime, options, u"minimumIntegerDigits", 1, 21, 1, integerDigits))
      return vm::ExecutionStatus::EXCEPTION;
    const bool significant = options.count(u"minimumSignificantDigits") ||
        options.count(u"maximumSignificantDigits");
    if (significant) {
      // Fraction options have already been read by the VM, but are not active
      // under the supported roundingPriority:auto significant-digit policy.
      if (!exactNfInteger(runtime, options, u"minimumSignificantDigits", 1, 21, 1, minDigits) ||
          !exactNfInteger(runtime, options, u"maximumSignificantDigits", minDigits, 21, 21, maxDigits))
        return vm::ExecutionStatus::EXCEPTION;
    } else {
      const int32_t defaultMin = style == u"currency" ? currencyDigits : 0;
      const int32_t defaultMax = style == u"currency" ? currencyDigits : style == u"percent" ? 0 : 3;
      int32_t givenMin = -1, givenMax = -1;
      if (!exactNfInteger(runtime, options, u"minimumFractionDigits", 0, 20, -1, givenMin) ||
          !exactNfInteger(runtime, options, u"maximumFractionDigits", 0, 20, -1, givenMax))
        return vm::ExecutionStatus::EXCEPTION;
      minDigits = givenMin >= 0 ? givenMin : givenMax >= 0 ? std::min(defaultMin, givenMax) : defaultMin;
      maxDigits = givenMax >= 0 ? givenMax : std::max(defaultMax, minDigits);
      if (minDigits > maxDigits)
        return exactNfRefuse(runtime, u"minimumFractionDigits/maximumFractionDigits");
    }

    const UNumberFormatStyle icuStyle = style == u"currency"
        ? (display == u"code" ? UNUM_CURRENCY_ISO : UNUM_CURRENCY)
        : style == u"percent" ? UNUM_PERCENT : UNUM_DECIMAL;
    status = U_ZERO_ERROR;
    formatter_.reset(unum_open(icuStyle, nullptr, 0, locale.c_str(), nullptr, &status));
    if (U_FAILURE(status) || !formatter_)
      return exactNfIcuFailure(runtime, "open", U_FAILURE(status) ? status : U_INTERNAL_PROGRAM_ERROR);
    if (style == u"currency") {
      status = U_ZERO_ERROR;
      unum_setTextAttribute(formatter_.get(), UNUM_CURRENCY_CODE, currency.c_str(), 3, &status);
      if (U_FAILURE(status))
        return exactNfIcuFailure(runtime, "currency", status);
    }
    unum_setAttribute(formatter_.get(), UNUM_GROUPING_USED, grouping ? 1 : 0);
    // Omitted grouping uses CLDR's locale default; explicit true requests it
    // whenever the integer has a grouping position. String modes are refused.
    unum_setAttribute(formatter_.get(), UNUM_MINIMUM_GROUPING_DIGITS,
        groupingIt == options.end() ? UNUM_MINIMUM_GROUPING_DIGITS_AUTO : 1);
    unum_setAttribute(formatter_.get(), UNUM_MAX_INTEGER_DIGITS, 999);
    unum_setAttribute(formatter_.get(), UNUM_MIN_INTEGER_DIGITS, integerDigits);
    unum_setAttribute(formatter_.get(), UNUM_ROUNDING_MODE, UNUM_ROUND_HALFUP);
    unum_setDoubleAttribute(formatter_.get(), UNUM_ROUNDING_INCREMENT, 0.0);
    unum_setAttribute(formatter_.get(), UNUM_SIGNIFICANT_DIGITS_USED, significant ? 1 : 0);
    const auto minAttribute = significant ? UNUM_MIN_SIGNIFICANT_DIGITS : UNUM_MIN_FRACTION_DIGITS;
    const auto maxAttribute = significant ? UNUM_MAX_SIGNIFICANT_DIGITS : UNUM_MAX_FRACTION_DIGITS;
    unum_setAttribute(formatter_.get(), maxAttribute, maxDigits);
    unum_setAttribute(formatter_.get(), minAttribute, minDigits);
    if (unum_getAttribute(formatter_.get(), UNUM_ROUNDING_MODE) != UNUM_ROUND_HALFUP ||
        unum_getAttribute(formatter_.get(), UNUM_MIN_INTEGER_DIGITS) != integerDigits ||
        unum_getAttribute(formatter_.get(), minAttribute) != minDigits ||
        unum_getAttribute(formatter_.get(), maxAttribute) != maxDigits ||
        unum_getAttribute(formatter_.get(), UNUM_GROUPING_USED) != (grouping ? 1 : 0))
      return exactNfIcuFailure(runtime, "effective attributes", U_INTERNAL_PROGRAM_ERROR);

    resolved_.emplace(u"locale", Option(resolved.localeBcp47Object.getCanonicalizedLocaleId()));
    resolved_.emplace(u"numberingSystem", Option(systemName));
    resolved_.emplace(u"style", Option(style));
    resolved_.emplace(u"minimumIntegerDigits", Option(static_cast<double>(integerDigits)));
    resolved_.emplace(significant ? u"minimumSignificantDigits" : u"minimumFractionDigits", Option(static_cast<double>(minDigits)));
    resolved_.emplace(significant ? u"maximumSignificantDigits" : u"maximumFractionDigits", Option(static_cast<double>(maxDigits)));
    resolved_.emplace(u"useGrouping", Option(grouping));
    resolved_.emplace(u"notation", Option(std::u16string(u"standard")));
    resolved_.emplace(u"signDisplay", Option(std::u16string(u"auto")));
    resolved_.emplace(u"roundingMode", Option(std::u16string(u"halfExpand")));
    resolved_.emplace(u"roundingIncrement", Option(1.0));
    resolved_.emplace(u"roundingPriority", Option(std::u16string(u"auto")));
    resolved_.emplace(u"trailingZeroDisplay", Option(std::u16string(u"auto")));
    if (style == u"currency") {
      resolved_.emplace(u"currency", Option(currency));
      resolved_.emplace(u"currencyDisplay", Option(display));
      resolved_.emplace(u"currencySign", Option(std::u16string(u"standard")));
    }
    return vm::ExecutionStatus::RETURNED;
  }

  Options options() const { return resolved_; }

  vm::CallResult<ExactNfRendered> render(vm::Runtime &runtime, double number, bool parts) {
    UErrorCode status = U_ZERO_ERROR;
    const int32_t size = unum_formatDouble(formatter_.get(), number, nullptr, 0, nullptr, &status);
    if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status))
      return exactNfIcuFailure(runtime, "format length", status);
    if (size <= 0 || size == std::numeric_limits<int32_t>::max())
      return exactNfIcuFailure(runtime, "format length", U_INTERNAL_PROGRAM_ERROR);
    ExactNfRendered result;
    result.text.resize(static_cast<size_t>(size) + 1);
    status = U_ZERO_ERROR;
    ExactNfFields fields(parts ? ufieldpositer_open(&status) : nullptr, ufieldpositer_close);
    if (U_FAILURE(status) || (parts && !fields))
      return exactNfIcuFailure(runtime, "fields open", U_FAILURE(status) ? status : U_INTERNAL_PROGRAM_ERROR);
    const int32_t written = parts
        ? unum_formatDoubleForFields(formatter_.get(), number, &result.text[0], size + 1, fields.get(), &status)
        : unum_formatDouble(formatter_.get(), number, &result.text[0], size + 1, nullptr, &status);
    if (U_FAILURE(status))
      return exactNfIcuFailure(runtime, "format", status);
    if (written != size)
      return exactNfIcuFailure(runtime, "format length changed", U_INTERNAL_PROGRAM_ERROR);
    result.text.resize(size);
    if (!parts)
      return std::move(result);

    std::vector<std::u16string> types(size, u"literal");
    std::vector<int32_t> widths(size, std::numeric_limits<int32_t>::max());
    int32_t begin, end;
    bool numeric = false;
    for (int32_t field; (field = ufieldpositer_next(fields.get(), &begin, &end)) >= 0;) {
      if (begin < 0 || end <= begin || end > size)
        return exactNfIcuFailure(runtime, "field bounds", U_INTERNAL_PROGRAM_ERROR);
      const char16_t *type = nullptr;
      switch (field) {
        case UNUM_INTEGER_FIELD:
          type = std::isnan(number) ? u"nan" : std::isinf(number) ? u"infinity" : u"integer";
          numeric = true;
          break;
        case UNUM_FRACTION_FIELD: type = u"fraction"; break;
        case UNUM_DECIMAL_SEPARATOR_FIELD: type = u"decimal"; break;
        case UNUM_GROUPING_SEPARATOR_FIELD: type = u"group"; break;
        case UNUM_CURRENCY_FIELD: type = u"currency"; break;
        case UNUM_PERCENT_FIELD: type = u"percentSign"; break;
        case UNUM_SIGN_FIELD: type = std::signbit(number) ? u"minusSign" : u"plusSign"; break;
        default:
          return exactNfIcuFailure(runtime, "unexpected standard-notation field", U_UNSUPPORTED_ERROR);
      }
      for (int32_t i = begin; i < end; ++i) {
        const int32_t width = end - begin;
        if (width < widths[i]) {
          widths[i] = width;
          types[i] = type;
        } else if (width == widths[i] && types[i] != type) {
          return exactNfIcuFailure(runtime, "ambiguous equal field spans", U_INTERNAL_PROGRAM_ERROR);
        }
      }
    }
    if (!numeric)
      return exactNfIcuFailure(runtime, "missing numeric field", U_INTERNAL_PROGRAM_ERROR);
    for (int32_t start = 0; start < size;) {
      int32_t stop = start + 1;
      while (stop < size && types[stop] == types[start])
        ++stop;
      result.parts.push_back(Part{{u"type", types[start]}, {u"value", result.text.substr(start, stop - start)}});
      start = stop;
    }
    return std::move(result);
  }

 private:
  ExactNfHandle formatter_{nullptr, unum_close};
  Options resolved_;
};
} // namespace

NumberFormat::NumberFormat() = default;
NumberFormat::~NumberFormat() = default;

vm::CallResult<std::vector<std::u16string>> NumberFormat::supportedLocalesOf(
    vm::Runtime &runtime,
    const std::vector<std::u16string> &locales,
    const Options &options) noexcept {
  return impl_icu::supportedLocales(runtime, locales, options);
}

vm::CallResult<std::unique_ptr<NumberFormat>> NumberFormat::create(
    vm::Runtime &runtime,
    const std::vector<std::u16string> &locales,
    const Options &options) noexcept {
  auto result = std::make_unique<ExactNumberFormatICU>();
  if (result->initialize(runtime, locales, options) == vm::ExecutionStatus::EXCEPTION)
    return vm::ExecutionStatus::EXCEPTION;
  return std::unique_ptr<NumberFormat>(std::move(result));
}

Options NumberFormat::resolvedOptions() noexcept {
  return static_cast<ExactNumberFormatICU *>(this)->options();
}

vm::CallResult<std::u16string> NumberFormat::format(
    vm::Runtime &runtime, double number) noexcept {
  auto result = static_cast<ExactNumberFormatICU *>(this)->render(runtime, number, false);
  if (result == vm::ExecutionStatus::EXCEPTION)
    return vm::ExecutionStatus::EXCEPTION;
  return std::move(result->text);
}

vm::CallResult<std::vector<Part>> NumberFormat::formatToParts(
    vm::Runtime &runtime, double number) noexcept {
  auto result = static_cast<ExactNumberFormatICU *>(this)->render(runtime, number, true);
  if (result == vm::ExecutionStatus::EXCEPTION)
    return vm::ExecutionStatus::EXCEPTION;
  return std::move(result->parts);
}
