// Windows-only replacement of the pinned ICU date adapter and parts stub.
// Included by the receipt-bound patch inside hermes::platform_intl.
namespace {
using ExactDateHandle = std::unique_ptr<UDateFormat, decltype(&udat_close)>;
using ExactDatePattern =
    std::unique_ptr<UDateTimePatternGenerator, decltype(&udatpg_close)>;
using ExactDateFields =
    std::unique_ptr<UFieldPositionIterator, decltype(&ufieldpositer_close)>;

vm::ExecutionStatus exactDateRefuse(vm::Runtime &runtime, const char16_t *name) {
  return runtime.raiseRangeError(
      vm::TwineChar16("Intl.DateTimeFormat: unsupported or invalid ") +
      vm::TwineChar16(name));
}
vm::ExecutionStatus exactDateFailure(vm::Runtime &runtime, UErrorCode status) {
  return runtime.raiseError(
      vm::TwineChar16("Intl.DateTimeFormat ICU: ") +
      vm::TwineChar16(u_errorName(status)));
}

// A locale keyword is applied to the actual ICU identifier, not merely reported
// in resolvedOptions. Explicit options take precedence over Unicode extensions.
bool exactDateKeyword(std::string &locale, const char *key, const std::string &value) {
  std::vector<char> buffer(locale.size() + value.size() + 128);
  std::copy(locale.begin(), locale.end(), buffer.begin());
  UErrorCode status = U_ZERO_ERROR;
  const int32_t length = uloc_setKeywordValue(
      key, value.c_str(), buffer.data(), static_cast<int32_t>(buffer.size()), &status);
  if (U_FAILURE(status))
    return false;
  locale.assign(buffer.data(), static_cast<size_t>(length));
  return true;
}

// ICU76 exposes the default cycle, but not the preferred member of a requested
// 12/24-hour family. Read its own supplemental timeData ordering (the same
// language/region lookup used by DateTimePatternGenerator), not a parity guess
// from the default cycle. This notably preserves Japan's preferred K/h11.
vm::CallResult<std::u16string> exactDatePreferredCycle(
    vm::Runtime &runtime, Locale locale, bool twelve, UDateFormatHourCycle fallback) {
  if (twelve && fallback == UDAT_HOUR_CYCLE_11) return std::u16string(u"h11");
  if (twelve && fallback == UDAT_HOUR_CYCLE_12) return std::u16string(u"h12");
  if (!twelve && fallback == UDAT_HOUR_CYCLE_23) return std::u16string(u"h23");
  if (!twelve && fallback == UDAT_HOUR_CYCLE_24) return std::u16string(u"h24");
  UErrorCode status = U_ZERO_ERROR;
  locale.addLikelySubtags(status);
  if (U_FAILURE(status)) return exactDateFailure(runtime, status);
  using Resource = std::unique_ptr<UResourceBundle, decltype(&ures_close)>;
  Resource supplemental(ures_openDirect(nullptr, "supplementalData", &status), ures_close);
  Resource data(ures_getByKey(supplemental.get(), "timeData", nullptr, &status), ures_close);
  if (U_FAILURE(status) || !supplemental || !data) return exactDateFailure(runtime, status);
  const std::string language = locale.getLanguage();
  const std::string country = locale.getCountry();
  Resource entry(nullptr, ures_close);
  for (const auto &key : {language + "_" + country, country, std::string("001")}) {
    status = U_ZERO_ERROR;
    entry.reset(ures_getByKey(data.get(), key.c_str(), nullptr, &status));
    if (U_SUCCESS(status) && entry) break;
    if (status != U_MISSING_RESOURCE_ERROR) return exactDateFailure(runtime, status);
  }
  if (U_FAILURE(status) || !entry) return exactDateFailure(runtime, status);
  Resource allowed(ures_getByKey(entry.get(), "allowed", nullptr, &status), ures_close);
  if (U_FAILURE(status) || !allowed) return exactDateFailure(runtime, status);
  for (int32_t i = 0; i < ures_getSize(allowed.get()); ++i) {
    int32_t length = 0;
    const auto *value = ures_getType(allowed.get()) == URES_STRING
        ? ures_getString(allowed.get(), &length, &status)
        : ures_getStringByIndex(allowed.get(), i, &length, &status);
    if (U_FAILURE(status)) return exactDateFailure(runtime, status);
    if (!value || length == 0) continue;
    if (twelve && value[0] == u'K') return std::u16string(u"h11");
    if (twelve && value[0] == u'h') return std::u16string(u"h12");
    if (!twelve && value[0] == u'H') return std::u16string(u"h23");
    if (!twelve && value[0] == u'k') return std::u16string(u"h24");
  }
  return exactDateRefuse(runtime, u"hour12 locale data");
}

const char16_t *exactDateField(int32_t field) {
  switch (field) {
    case UDAT_ERA_FIELD: return u"era";
    case UDAT_YEAR_FIELD: case UDAT_EXTENDED_YEAR_FIELD: return u"year";
    case UDAT_MONTH_FIELD: case UDAT_STANDALONE_MONTH_FIELD: return u"month";
    case UDAT_DATE_FIELD: return u"day";
    case UDAT_DAY_OF_WEEK_FIELD: case UDAT_STANDALONE_DAY_FIELD: return u"weekday";
    case UDAT_HOUR_OF_DAY0_FIELD: case UDAT_HOUR_OF_DAY1_FIELD:
    case UDAT_HOUR0_FIELD: case UDAT_HOUR1_FIELD: return u"hour";
    case UDAT_MINUTE_FIELD: return u"minute";
    case UDAT_SECOND_FIELD: return u"second";
    case UDAT_FRACTIONAL_SECOND_FIELD: return u"fractionalSecond";
    case UDAT_AM_PM_FIELD: case UDAT_AM_PM_MIDNIGHT_NOON_FIELD:
    case UDAT_FLEXIBLE_DAY_PERIOD_FIELD: return u"dayPeriod";
    case UDAT_TIMEZONE_FIELD: case UDAT_TIMEZONE_RFC_FIELD:
    case UDAT_TIMEZONE_GENERIC_FIELD: case UDAT_TIMEZONE_SPECIAL_FIELD:
    case UDAT_TIMEZONE_LOCALIZED_GMT_OFFSET_FIELD:
    case UDAT_TIMEZONE_ISO_FIELD: case UDAT_TIMEZONE_ISO_LOCAL_FIELD:
      return u"timeZoneName";
    default: return u"literal";
  }
}

// ICU pattern symbols are meaningful only outside quoted literals. Reuse this
// scan for reported widths and hour-cycle changes; doubled quotes are literals.
template <typename F>
void exactDatePatternFields(const std::u16string &pattern, F visit) {
  bool quoted = false;
  for (size_t i = 0; i < pattern.size();) {
    const auto ch = pattern[i];
    if (ch == u'\'') {
      if (i + 1 < pattern.size() && pattern[i + 1] == u'\'') {
        i += 2;
        continue;
      }
      quoted = !quoted;
      ++i;
    } else if (!quoted && ((ch >= u'A' && ch <= u'Z') || (ch >= u'a' && ch <= u'z'))) {
      size_t end = i + 1;
      while (end < pattern.size() && pattern[end] == ch) ++end;
      visit(ch, end - i);
      i = end;
    } else {
      ++i;
    }
  }
}

class ExactDateTimeFormatICU final : public DateTimeFormat {
 public:
  vm::ExecutionStatus initialize(
      vm::Runtime &runtime,
      const std::vector<std::u16string> &locales,
      const Options &input) {
    auto requested = impl_icu::LocaleBCP47Object::canonicalizeLocaleList(runtime, locales);
    if (requested == vm::ExecutionStatus::EXCEPTION)
      return vm::ExecutionStatus::EXCEPTION;
    auto normalized = toDateTimeOptions(runtime, input, u"any", u"date");
    if (normalized == vm::ExecutionStatus::EXCEPTION)
      return vm::ExecutionStatus::EXCEPTION;
    const auto &options = *normalized;
    auto stringOption = [&](const char16_t *name,
                            llvh::ArrayRef<const char16_t *> values,
                            std::optional<std::u16string_view> fallback = {}) {
      return impl_icu::getStringOption(runtime, options, name, values, fallback);
    };
    auto matcher = stringOption(u"localeMatcher", {u"lookup", u"best fit"}, u"best fit");
    auto formatMatcher = stringOption(u"formatMatcher", {u"basic", u"best fit"}, u"best fit");
    if (matcher == vm::ExecutionStatus::EXCEPTION || formatMatcher == vm::ExecutionStatus::EXCEPTION)
      return vm::ExecutionStatus::EXCEPTION;
    if (**formatMatcher != u"best fit") return exactDateRefuse(runtime, u"formatMatcher");
    Options localeOptions;
    localeOptions.emplace(u"localeMatcher", Option(**matcher));
    for (const auto &entry : {std::pair<const char16_t *, const char16_t *>{u"calendar", u"ca"},
                             {u"numberingSystem", u"nu"}}) {
      auto value = stringOption(entry.first, {});
      if (value == vm::ExecutionStatus::EXCEPTION) return vm::ExecutionStatus::EXCEPTION;
      if (*value) {
        if (!isUnicodeExtensionType(**value)) return exactDateRefuse(runtime, entry.first);
        localeOptions.emplace(entry.second, Option(**value));
      }
    }
    auto cycle = stringOption(u"hourCycle", {u"h11", u"h12", u"h23", u"h24"});
    if (cycle == vm::ExecutionStatus::EXCEPTION) return vm::ExecutionStatus::EXCEPTION;
    auto hour12 = impl_icu::getBoolOption(options, u"hour12", {});
    if (hour12) localeOptions.emplace(u"hc", Option(std::u16string()));
    else if (*cycle) localeOptions.emplace(u"hc", Option(**cycle));
    static constexpr std::u16string_view relevant[] = {u"ca", u"nu", u"hc"};
    auto resolved = impl_icu::resolveLocale(*requested, localeOptions, relevant,
        [](auto, auto, const auto &) { return true; });
    const auto locale = resolved.localeBcp47Object.getCanonicalizedLocaleId();
    result_.emplace(u"locale", Option(locale));
    UErrorCode status = U_ZERO_ERROR;
    auto icuLocale = Locale::forLanguageTag(impl_icu::toUTF8ASCII(locale), status);
    if (U_FAILURE(status)) return exactDateFailure(runtime, status);
    std::string locale8 = icuLocale.getName();
    for (const auto &entry : {std::pair<const char16_t *, const char *>{u"ca", "calendar"},
                             {u"nu", "numbers"}}) {
      const auto found = resolved.resolvedOpts.find(entry.first);
      if (found != resolved.resolvedOpts.end() && !found->second.getString().empty()) {
        auto value = impl_icu::toUTF8ASCII(found->second.getString());
        if (entry.first == std::u16string_view(u"ca")) {
          if (value != "gregory") return exactDateRefuse(runtime, u"calendar");
          value = "gregorian";
        } else {
          UErrorCode nsStatus = U_ZERO_ERROR;
          std::unique_ptr<UNumberingSystem, decltype(&unumsys_close)> ns(
              unumsys_openByName(value.c_str(), &nsStatus), unumsys_close);
          if (U_FAILURE(nsStatus) || !ns || unumsys_isAlgorithmic(ns.get()) ||
              unumsys_getRadix(ns.get()) != 10 || value != unumsys_getName(ns.get()))
            return exactDateRefuse(runtime, u"numberingSystem");
        }
        if (!exactDateKeyword(locale8, entry.second, value))
          return runtime.raiseError("Intl.DateTimeFormat locale keyword failed");
      }
    }
    std::unique_ptr<UCalendar, decltype(&ucal_close)> calendar(
        ucal_open(nullptr, 0, locale8.c_str(), UCAL_DEFAULT, &status), ucal_close);
    if (U_FAILURE(status) || !calendar) return exactDateFailure(runtime, status);
    const char *calendarType = ucal_getType(calendar.get(), &status);
    if (U_FAILURE(status)) return exactDateFailure(runtime, status);
    if (std::string(calendarType) != "gregorian") return exactDateRefuse(runtime, u"calendar");
    result_.emplace(u"calendar", Option(std::u16string(u"gregory")));
    std::unique_ptr<UNumberingSystem, decltype(&unumsys_close)> system(
        unumsys_open(locale8.c_str(), &status), unumsys_close);
    if (U_FAILURE(status) || !system) return exactDateFailure(runtime, status);
    if (unumsys_isAlgorithmic(system.get()) || unumsys_getRadix(system.get()) != 10)
      return exactDateRefuse(runtime, u"numberingSystem");
    const std::string systemName = unumsys_getName(system.get());
    result_.emplace(u"numberingSystem", Option(std::u16string(systemName.begin(), systemName.end())));
    auto zoneOption = stringOption(u"timeZone", {});
    if (zoneOption == vm::ExecutionStatus::EXCEPTION) return vm::ExecutionStatus::EXCEPTION;
    std::u16string zone = *zoneOption ? **zoneOption : getDefaultTimeZone(runtime);
    if (!isValidTimeZoneName(zone)) return exactDateRefuse(runtime, u"timeZone");
    zone = canonicalizeTimeZoneName(zone);
    result_.emplace(u"timeZone", Option(zone));
    auto dateStyle = stringOption(u"dateStyle", {u"full", u"long", u"medium", u"short"});
    auto timeStyle = stringOption(u"timeStyle", {u"full", u"long", u"medium", u"short"});
    if (dateStyle == vm::ExecutionStatus::EXCEPTION || timeStyle == vm::ExecutionStatus::EXCEPTION)
      return vm::ExecutionStatus::EXCEPTION;
    const bool styles = bool(*dateStyle) || bool(*timeStyle);
    ExactDatePattern generator(udatpg_open(locale8.c_str(), &status), udatpg_close);
    if (U_FAILURE(status) || !generator) return exactDateFailure(runtime, status);
    auto defaultCycle = udatpg_getDefaultHourCycle(generator.get(), &status);
    if (U_FAILURE(status)) return exactDateFailure(runtime, status);
    std::u16string hourCycle = defaultCycle == UDAT_HOUR_CYCLE_11 ? u"h11" :
        defaultCycle == UDAT_HOUR_CYCLE_12 ? u"h12" :
        defaultCycle == UDAT_HOUR_CYCLE_23 ? u"h23" : u"h24";
    const auto hc = resolved.resolvedOpts.find(u"hc");
    if (hc != resolved.resolvedOpts.end() && !hc->second.getString().empty()) {
      hourCycle = hc->second.getString();
      if (hourCycle != u"h11" && hourCycle != u"h12" && hourCycle != u"h23" && hourCycle != u"h24")
        return exactDateRefuse(runtime, u"hourCycle");
    }
    if (hour12) {
      auto preferred = exactDatePreferredCycle(runtime, icuLocale, *hour12, defaultCycle);
      if (preferred == vm::ExecutionStatus::EXCEPTION) return vm::ExecutionStatus::EXCEPTION;
      hourCycle = std::move(*preferred);
    }
    const char16_t hourSymbol = hourCycle == u"h11" ? u'K' : hourCycle == u"h12" ? u'h' :
        hourCycle == u"h23" ? u'H' : u'k';
    std::u16string skeleton;
    struct Component { const char16_t *name; const char16_t *values[5]; const char16_t *patterns[5]; };
    const Component components[] = {
      {u"weekday", {u"narrow",u"short",u"long"}, {u"EEEEE",u"EEE",u"EEEE"}},
      {u"era", {u"narrow",u"short",u"long"}, {u"GGGGG",u"GGG",u"GGGG"}},
      {u"year", {u"numeric",u"2-digit"}, {u"y",u"yy"}},
      {u"month", {u"numeric",u"2-digit",u"narrow",u"short",u"long"}, {u"M",u"MM",u"MMMMM",u"MMM",u"MMMM"}},
      {u"day", {u"numeric",u"2-digit"}, {u"d",u"dd"}},
      {u"dayPeriod", {u"narrow",u"short",u"long"}, {u"BBBBB",u"B",u"BBBB"}},
      {u"hour", {u"numeric",u"2-digit"}, {u"h",u"hh"}},
      {u"minute", {u"numeric",u"2-digit"}, {u"m",u"mm"}},
      {u"second", {u"numeric",u"2-digit"}, {u"s",u"ss"}},
    };
    for (const auto &component : components) {
      auto value = stringOption(component.name, {});
      if (value == vm::ExecutionStatus::EXCEPTION) return vm::ExecutionStatus::EXCEPTION;
      if (!*value) continue;
      if (styles) return runtime.raiseTypeError("Intl.DateTimeFormat styles conflict with components");
      bool accepted = false;
      for (size_t i = 0; i < 5 && component.values[i]; ++i) {
        if (**value != component.values[i]) continue;
        auto part = std::u16string(component.patterns[i]);
        if (std::u16string_view(component.name) == u"hour") part.assign(part.size(), hourSymbol);
        skeleton += part;
        accepted = true;
        break;
      }
      if (!accepted) return exactDateRefuse(runtime, component.name);
    }
    auto fraction = impl_icu::getNumberOption(runtime, options, u"fractionalSecondDigits", 1, 3, {});
    if (fraction == vm::ExecutionStatus::EXCEPTION) return vm::ExecutionStatus::EXCEPTION;
    if (*fraction) {
      if (styles) return runtime.raiseTypeError("Intl.DateTimeFormat styles conflict with fractionalSecondDigits");
      skeleton.append(static_cast<size_t>(**fraction), u'S');
    }
    auto zoneName = stringOption(u"timeZoneName", {u"short",u"long",u"shortOffset",u"longOffset",u"shortGeneric",u"longGeneric"});
    if (zoneName == vm::ExecutionStatus::EXCEPTION) return vm::ExecutionStatus::EXCEPTION;
    if (*zoneName) {
      if (styles) return runtime.raiseTypeError("Intl.DateTimeFormat styles conflict with timeZoneName");
      skeleton += **zoneName == u"short" ? u"z" : **zoneName == u"long" ? u"zzzz" :
          **zoneName == u"shortOffset" ? u"O" : **zoneName == u"longOffset" ? u"OOOO" :
          **zoneName == u"shortGeneric" ? u"v" : u"vvvv";
    }
    if (styles) {
      auto style = [](const std::optional<std::u16string> &s) {
        return !s ? UDAT_NONE : *s == u"full" ? UDAT_FULL : *s == u"long" ? UDAT_LONG :
            *s == u"medium" ? UDAT_MEDIUM : UDAT_SHORT;
      };
      formatter_.reset(udat_open(style(*timeStyle), style(*dateStyle), locale8.c_str(),
          zone.data(), static_cast<int32_t>(zone.size()), nullptr, 0, &status));
      if (U_FAILURE(status) || !formatter_) return exactDateFailure(runtime, status);
      if (*dateStyle) result_.emplace(u"dateStyle", Option(**dateStyle));
      if (*timeStyle) result_.emplace(u"timeStyle", Option(**timeStyle));
      // Preserve the locale's complete style pattern unless the requested hour
      // convention requires a new best-fit pattern with the same field widths.
      if (hour12 || *cycle || hc != resolved.resolvedOpts.end()) {
        auto oldPattern = pattern(runtime);
        if (oldPattern == vm::ExecutionStatus::EXCEPTION) return vm::ExecutionStatus::EXCEPTION;
        exactDatePatternFields(*oldPattern, [&](char16_t ch, size_t count) {
          if (ch == u'a' || ch == u'b' || ch == u'B') return;
          if (ch == u'h' || ch == u'H' || ch == u'K' || ch == u'k') ch = hourSymbol;
          skeleton.append(count, ch);
        });
      }
    }
    if (!styles || !skeleton.empty()) {
      status = U_ZERO_ERROR;
      int32_t length = udatpg_getBestPatternWithOptions(generator.get(), skeleton.data(),
          static_cast<int32_t>(skeleton.size()), UDATPG_MATCH_HOUR_FIELD_LENGTH, nullptr, 0, &status);
      if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status)) return exactDateFailure(runtime, status);
      std::u16string best(static_cast<size_t>(length), u'\0');
      status = U_ZERO_ERROR;
      udatpg_getBestPatternWithOptions(generator.get(), skeleton.data(), static_cast<int32_t>(skeleton.size()),
          UDATPG_MATCH_HOUR_FIELD_LENGTH, best.data(), length, &status);
      if (U_FAILURE(status)) return exactDateFailure(runtime, status);
      // ICU best-fit may canonicalize K to h (or k to H). Preserve the
      // requested zero/one-based cycle without touching quoted literals.
      bool quoted = false;
      for (size_t i = 0; i < best.size(); ++i) {
        if (best[i] == u'\'') {
          if (i + 1 < best.size() && best[i + 1] == u'\'') { ++i; continue; }
          quoted = !quoted;
        } else if (!quoted && (best[i] == u'h' || best[i] == u'H' || best[i] == u'K' || best[i] == u'k')) {
          best[i] = hourSymbol;
        }
      }
      formatter_.reset(udat_open(UDAT_PATTERN, UDAT_PATTERN, locale8.c_str(), zone.data(),
          static_cast<int32_t>(zone.size()), best.data(), length, &status));
      if (U_FAILURE(status) || !formatter_) return exactDateFailure(runtime, status);
    }
    // Gregorian means proleptic Gregorian over the complete JS TimeClip range.
    // Clone the final calendar so timezone and formatter options survive. The
    // cutover is earlier than minimum TimeClip even after a timezone offset.
    status = U_ZERO_ERROR;
    std::unique_ptr<UCalendar, decltype(&ucal_close)> proleptic(
        ucal_clone(udat_getCalendar(formatter_.get()), &status), ucal_close);
    if (U_FAILURE(status) || !proleptic) return exactDateFailure(runtime, status);
    constexpr double earliestGregorian = -9007199254740992.0;
    ucal_setGregorianChange(proleptic.get(), earliestGregorian, &status);
    if (U_FAILURE(status)) return exactDateFailure(runtime, status);
    udat_setCalendar(formatter_.get(), proleptic.get());
    if (ucal_getGregorianChange(udat_getCalendar(formatter_.get()), &status) != earliestGregorian || U_FAILURE(status))
      return runtime.raiseError("Intl.DateTimeFormat Gregorian calendar attachment failed");
    auto actualPattern = pattern(runtime);
    if (actualPattern == vm::ExecutionStatus::EXCEPTION) return vm::ExecutionStatus::EXCEPTION;
    recordPattern(*actualPattern, styles);
    return vm::ExecutionStatus::RETURNED;
  }

  Options resolvedOptions() const { return result_; }

  struct Rendered { std::u16string text; std::vector<Part> parts; };
  vm::CallResult<Rendered> render(vm::Runtime &runtime, double value, bool parts) {
    if (!std::isfinite(value) || std::abs(value) > 8.64e15)
      return runtime.raiseRangeError("Intl.DateTimeFormat invalid time value");
    value = std::trunc(value);
    UErrorCode status = U_ZERO_ERROR;
    int32_t length = udat_format(formatter_.get(), value, nullptr, 0, nullptr, &status);
    if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status)) return exactDateFailure(runtime, status);
    Rendered out;
    out.text.resize(static_cast<size_t>(length));
    status = U_ZERO_ERROR;
    ExactDateFields fields(parts ? ufieldpositer_open(&status) : nullptr, ufieldpositer_close);
    if (U_FAILURE(status) || (parts && !fields)) return exactDateFailure(runtime, status);
    const int32_t written = parts ? udat_formatForFields(formatter_.get(), value, out.text.data(),
        length, fields.get(), &status) : udat_format(formatter_.get(), value, out.text.data(), length, nullptr, &status);
    if (U_FAILURE(status) || written != length) return exactDateFailure(runtime, status);
    if (parts) {
      std::vector<std::u16string> types(static_cast<size_t>(length), u"literal");
      std::vector<int32_t> widths(static_cast<size_t>(length), INT32_MAX);
      int32_t begin, end, field;
      while ((field = ufieldpositer_next(fields.get(), &begin, &end)) >= 0) {
        if (begin < 0 || end < begin || end > length)
          return runtime.raiseError("Intl.DateTimeFormat invalid ICU field range");
        const auto type = exactDateField(field);
        for (int32_t i = begin; i < end; ++i) {
          if (end - begin < widths[i]) { types[i] = type; widths[i] = end - begin; }
        }
      }
      for (size_t begin = 0; begin < types.size();) {
        size_t end = begin + 1;
        while (end < types.size() && types[end] == types[begin]) ++end;
        out.parts.push_back({{u"type", types[begin]}, {u"value", out.text.substr(begin, end - begin)}});
        begin = end;
      }
    }
    return out;
  }

 private:
  ExactDateHandle formatter_{nullptr, udat_close};
  Options result_;

  vm::CallResult<std::u16string> pattern(vm::Runtime &runtime) {
    UErrorCode status = U_ZERO_ERROR;
    int32_t length = udat_toPattern(formatter_.get(), false, nullptr, 0, &status);
    if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status)) return exactDateFailure(runtime, status);
    std::u16string out(static_cast<size_t>(length), u'\0');
    status = U_ZERO_ERROR;
    const auto written = udat_toPattern(formatter_.get(), false, out.data(), length, &status);
    if (U_FAILURE(status) || written != length) return exactDateFailure(runtime, status);
    return out;
  }
  void recordPattern(const std::u16string &pattern, bool styles) {
    exactDatePatternFields(pattern, [&](char16_t ch, size_t count) {
      const char16_t *name = nullptr;
      std::u16string value = count == 2 ? u"2-digit" : u"numeric";
      switch (ch) {
        case u'G': name = u"era"; value = count == 5 ? u"narrow" : count == 4 ? u"long" : u"short"; break;
        case u'y': name = u"year"; break;
        case u'M': case u'L': name = u"month";
          if (count >= 3) value = count == 5 ? u"narrow" : count == 4 ? u"long" : u"short"; break;
        case u'd': name = u"day"; break;
        case u'E': case u'c': case u'e': name = u"weekday";
          value = count == 5 ? u"narrow" : count == 4 ? u"long" : u"short"; break;
        case u'h': case u'H': case u'K': case u'k': {
          name = u"hour";
          result_.insert_or_assign(u"hourCycle", Option(std::u16string(ch == u'K' ? u"h11" :
              ch == u'h' ? u"h12" : ch == u'H' ? u"h23" : u"h24")));
          result_.insert_or_assign(u"hour12", Option(ch == u'K' || ch == u'h'));
          break;
        }
        case u'm': name = u"minute"; break;
        case u's': name = u"second"; break;
        case u'B': case u'b': name = u"dayPeriod";
          value = count == 5 ? u"narrow" : count == 4 ? u"long" : u"short"; break;
        case u'S': if (!styles) result_.insert_or_assign(u"fractionalSecondDigits", Option(double(count))); break;
        case u'z': case u'v': case u'O': name = u"timeZoneName";
          value = ch == u'z' ? (count >= 4 ? u"long" : u"short") :
              ch == u'v' ? (count >= 4 ? u"longGeneric" : u"shortGeneric") :
              (count >= 4 ? u"longOffset" : u"shortOffset"); break;
        default: break;
      }
      if (name && !styles) result_.insert_or_assign(name, Option(value));
    });
  }
};
} // namespace

DateTimeFormat::DateTimeFormat() = default;
DateTimeFormat::~DateTimeFormat() = default;
vm::CallResult<std::vector<std::u16string>> DateTimeFormat::supportedLocalesOf(
    vm::Runtime &runtime, const std::vector<std::u16string> &locales, const Options &options) noexcept {
  return impl_icu::supportedLocales(runtime, locales, options);
}
vm::CallResult<std::unique_ptr<DateTimeFormat>> DateTimeFormat::create(
    vm::Runtime &runtime, const std::vector<std::u16string> &locales, const Options &options) noexcept {
  return createInstance<ExactDateTimeFormatICU, DateTimeFormat>(runtime, locales, options);
}
Options DateTimeFormat::resolvedOptions() noexcept {
  return static_cast<ExactDateTimeFormatICU *>(this)->resolvedOptions();
}
vm::CallResult<std::u16string> DateTimeFormat::format(vm::Runtime &runtime, double value) noexcept {
  auto result = static_cast<ExactDateTimeFormatICU *>(this)->render(runtime, value, false);
  if (result == vm::ExecutionStatus::EXCEPTION) return vm::ExecutionStatus::EXCEPTION;
  return std::move(result->text);
}
vm::CallResult<std::vector<Part>> DateTimeFormat::formatToParts(vm::Runtime &runtime, double value) noexcept {
  auto result = static_cast<ExactDateTimeFormatICU *>(this)->render(runtime, value, true);
  if (result == vm::ExecutionStatus::EXCEPTION) return vm::ExecutionStatus::EXCEPTION;
  return std::move(result->parts);
}
