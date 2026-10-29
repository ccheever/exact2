//! Device grants: one line admits a device, and every host's spelling of it
//! is derived from this table.
//!
//! @ref LLP 1069.008 D1 (`device.<web permission name> <strings key>`) / D2
//! (one table maps a name to every host's spelling) / D6 (the served web's
//! `Permissions-Policy`)
//!
//! A device grant is an exact2 grant, like `surface.read`: [`io_grants`]
//! strips it before `ibex2` parses the rest. Its target is a strings key
//! (LLP 1060), never prose, so the line stays one line and the text is
//! translatable; the bake refuses a key the base table lacks.
//!
//! `auth.*` lines (LLP 1069.006) are stripped the same way and checked by
//! `crate::auth`; the bake derives their associated domains (`reach.auth`).
//!
//! [`io_grants`]: crate::io_grants

/// One device a grant can name, and what each host calls it.
#[derive(Debug, PartialEq, Eq)]
pub struct Device {
    /// The web Permissions Registry name where one exists; a name the web
    /// lacks is a declared deviation (`web` is then `None`).
    pub name: &'static str,
    /// iOS `Info.plist` usage keys, each given the purpose text.
    pub ios: &'static [&'static str],
    /// iOS `Info.plist` keys set to true.
    pub ios_flags: &'static [&'static str],
    /// Android's permissions (`<uses-permission>`): the runtime ones a module
    /// asks for in one request (LLP 1133.002 D6), and the normal ones granted
    /// at install (a foreground service's, LLP 1133.033 D16). One the platform retired at an API level
    /// is written `<name> maxSdkVersion=<level>`, which the manifest
    /// declares as `android:maxSdkVersion` (LLP 1133.019 D4).
    pub android: &'static [&'static str],
    /// Android permissions derived beside `android` only where the app uses
    /// a module name that writes under the line (`"writes": true` in its
    /// declaration; LLP 1133.018 D11: `WRITE_CALENDAR` for `saveEvent`).
    pub android_writes: &'static [&'static str],
    /// macOS `Info.plist` usage keys.
    pub macos: &'static [&'static str],
    /// The macOS hardened-runtime entitlement: without it a notarised build is
    /// silently refused, whatever `Info.plist` says.
    pub hardened: Option<&'static str>,
    /// The web `Permissions-Policy` feature, where the feature is policy-controlled.
    pub web: Option<&'static str>,
    /// Further `Permissions-Policy` features the row turns on beside `web`:
    /// motion's three sensors are three features (LLP 1133.013 D11).
    pub policy: &'static [&'static str],
    /// iOS signing entitlements the device needs beside its usage strings
    /// (LLP 1069.008.000 D2): HealthKit authorizes nothing without its own.
    pub ios_entitlements: &'static [&'static str],
    /// The value of each of `ios_entitlements` that is no boolean, by name:
    /// NFC's reader-session formats are an array (LLP 1133.035 D16 item 2).
    pub ios_entitlement_values: &'static [(&'static str, &'static [&'static str])],
    /// A usage key written with this row's text when no granted row writes it
    /// (LLP 1069.008.000 D1): App Store Connect wants both HealthKit keys
    /// beside the entitlement, and iOS shows only the prompt an app asks for.
    pub companion: Option<&'static str>,
    /// Whether the row applies on tvOS, which bakes the iOS plan (D5).
    pub tvos: bool,
    /// An app granting it keeps no answers across launches (D7): its data
    /// must not sit in `exact.kept.*` outside the platform's protection.
    pub keeps_no_answers: bool,
    /// The row this one asks for less of (a declared deviation, as
    /// `geolocation-approximate` is of `geolocation`, LLP 1133.002 D7): an app
    /// grants one or the other, never both (`grant-twice`).
    pub narrows: Option<&'static str>,
    /// A row the platforms will not give beside this one (D13: no region is
    /// watched under approximate location): refused together (`grant-conflict`).
    pub conflicts: Option<&'static str>,
    /// Whether the line names a purpose: false for a row no platform asks
    /// the person about, written `device.<name>` alone (LLP 1133.017 D14).
    pub purpose: bool,
    /// iOS `UIBackgroundModes` entries the row needs (LLP 1069.008 D4).
    pub ios_modes: &'static [&'static str],
    /// The `BGTaskScheduler` identifiers the row permits, each after the
    /// bundle id and a dot in `BGTaskSchedulerPermittedIdentifiers`.
    pub ios_tasks: &'static [&'static str],
    /// The row another must stand beside (`grant-needs`, LLP 1133.033 D3):
    /// a capture in the background is a capture first, and background
    /// location is asked of an app with foreground location (LLP 1133.021 D13).
    pub needs: Option<&'static str>,
    /// The Android reader's foreground services the row declares in the
    /// manifest, each `<kind>:<type>` (`capture:microphone`; LLP 1133.033
    /// D2): a kind's `<service>` is declared with every type its rows give
    /// it, so a later row adds a type to a kind (LLP 1133.032 D11 item 3).
    pub android_services: &'static [&'static str],
    /// Android hardware features the row declares, each written
    /// `<uses-feature android:name="android.hardware.<name>"
    /// android:required="false"/>` so a store still offers the app to a
    /// device without it (LLP 1133.037 D17: `usb.host`).
    pub android_features: &'static [&'static str],
}

/// What a row has unless it says otherwise.
const ROW: Device = Device {
    name: "",
    ios: &[],
    ios_flags: &[],
    android: &[],
    android_writes: &[],
    macos: &[],
    hardened: None,
    web: None,
    policy: &[],
    ios_entitlements: &[],
    ios_entitlement_values: &[],
    companion: None,
    tvos: true,
    keeps_no_answers: false,
    narrows: None,
    conflicts: None,
    purpose: true,
    ios_modes: &[],
    ios_tasks: &[],
    needs: None,
    android_services: &[],
    android_features: &[],
};

/// Every device grant. Adding a row is one entry here and no host code.
pub const DEVICES: &[Device] = &[
    Device {
        name: "microphone",
        ios: &["NSMicrophoneUsageDescription"],
        macos: &["NSMicrophoneUsageDescription"],
        hardened: Some("com.apple.security.device.audio-input"),
        web: Some("microphone"),
        android: &["android.permission.RECORD_AUDIO"],
        ..ROW
    },
    // @ref LLP 1133.033 D3 — a microphone capture keeps running while the app
    // is in the background. iOS keeps the process for the `audio` mode, and
    // the capture reads `ExactCapturesInBackground`, since other lines derive
    // `audio` too; Android runs it in the reader's `capture` service, whose
    // notification's text is the purpose. A Mac and the web need nothing,
    // and tvOS's recording cell is later.
    Device {
        name: "microphone-background",
        ios_flags: &["ExactCapturesInBackground"],
        ios_modes: &["audio"],
        android: &[
            "android.permission.FOREGROUND_SERVICE",
            "android.permission.FOREGROUND_SERVICE_MICROPHONE",
        ],
        android_services: &["capture:microphone"],
        needs: Some("microphone"),
        tvos: false,
        ..ROW
    },
    // @ref LLP 1133.033 D16 — `@exact/calls`. A call keeps the process for
    // its active audio session on iOS (`audio`), and CallKit refuses a
    // transaction as `unentitled` without `voip` (measured on an iOS 27
    // simulator, 2026-10-27). On Android it runs in the reader's `call` service,
    // which needs `MANAGE_OWN_CALLS`; the purpose names the call
    // notification's channel. Every Android permission here is normal but
    // `USE_FULL_SCREEN_INTENT`, a special access the person can turn off.
    // No call service on a Mac, the web or tvOS.
    Device {
        name: "calls",
        ios_modes: &["audio", "voip"],
        android: &[
            "android.permission.MANAGE_OWN_CALLS",
            "android.permission.FOREGROUND_SERVICE",
            "android.permission.FOREGROUND_SERVICE_PHONE_CALL",
            "android.permission.FOREGROUND_SERVICE_MICROPHONE",
            "android.permission.USE_FULL_SCREEN_INTENT",
        ],
        android_services: &["call:phoneCall", "call:microphone"],
        needs: Some("microphone"),
        tvos: false,
        ..ROW
    },
    // The camera, `@exact/camera`'s (LLP 1133.008 D6). Its Android features
    // are declared not required where the permission is (app/build.gradle.kts).
    Device {
        name: "camera",
        ios: &["NSCameraUsageDescription"],
        macos: &["NSCameraUsageDescription"],
        hardened: Some("com.apple.security.device.camera"),
        web: Some("camera"),
        android: &["android.permission.CAMERA"],
        ..ROW
    },
    Device {
        name: "geolocation",
        ios: &["NSLocationWhenInUseUsageDescription"],
        macos: &[
            "NSLocationUsageDescription",
            "NSLocationWhenInUseUsageDescription",
        ],
        hardened: Some("com.apple.security.personal-information.location"),
        web: Some("geolocation"),
        android: &[
            "android.permission.ACCESS_FINE_LOCATION",
            "android.permission.ACCESS_COARSE_LOCATION",
        ],
        ..ROW
    },
    // Approximate location (LLP 1133.002 D7): the same keys, and on iOS the
    // prompt opens with precise off (macOS has no such key).
    Device {
        name: "geolocation-approximate",
        ios: &["NSLocationWhenInUseUsageDescription"],
        ios_flags: &["NSLocationDefaultAccuracyReduced"],
        // Coarse alone: a ceiling on Android, where fine can then never be asked (D7).
        android: &["android.permission.ACCESS_COARSE_LOCATION"],
        macos: &[
            "NSLocationUsageDescription",
            "NSLocationWhenInUseUsageDescription",
        ],
        hardened: Some("com.apple.security.personal-information.location"),
        web: Some("geolocation"),
        narrows: Some("geolocation"),
        ..ROW
    },
    // Background location, as geofencing needs it (LLP 1133.021 D13): iOS
    // asks with its own key beside the When In Use one; a Mac's location
    // authorization is one grant; Android adds the background permission and
    // the boot broadcast its geofences are re-armed by. Neither platform
    // watches a region under approximate location. No tvOS cell.
    Device {
        name: "geolocation-background",
        ios: &["NSLocationAlwaysAndWhenInUseUsageDescription"],
        macos: &[
            "NSLocationUsageDescription",
            "NSLocationWhenInUseUsageDescription",
        ],
        hardened: Some("com.apple.security.personal-information.location"),
        android: &[
            "android.permission.ACCESS_BACKGROUND_LOCATION",
            "android.permission.RECEIVE_BOOT_COMPLETED",
        ],
        tvos: false,
        needs: Some("geolocation"),
        conflicts: Some("geolocation-approximate"),
        ..ROW
    },
    // iOS's prompt text is fixed and the web's is not policy-controlled; the
    // grant still gates the request.
    Device {
        name: "notifications",
        ios: &[],
        macos: &[],
        hardened: None,
        web: None,
        ..ROW
    },
    // Deviation: the web has no name; its speech API is gated by `microphone`.
    Device {
        name: "speech-recognition",
        ios: &["NSSpeechRecognitionUsageDescription"],
        macos: &["NSSpeechRecognitionUsageDescription"],
        hardened: None,
        web: None,
        ..ROW
    },
    // Deviation, same reason: saving to the library. PhotoKit under the
    // hardened runtime needs the library entitlement for a write too. Apple
    // lists the add key for iOS alone and the read key for macOS too, so
    // macOS writes both (LLP 1133.019 D4).
    Device {
        name: "photos-add",
        ios: &["NSPhotoLibraryAddUsageDescription"],
        macos: &[
            "NSPhotoLibraryAddUsageDescription",
            "NSPhotoLibraryUsageDescription",
        ],
        hardened: Some("com.apple.security.personal-information.photos-library"),
        web: None,
        ..ROW
    },
    // Reading the library, `@exact/photos` (LLP 1133.019 D4): a deviation,
    // the web has no library. iOS never shows its automatic limited-access
    // alert, which a tool's or a wake's read would otherwise raise (D6). An
    // app reading it keeps no answers (D6, ruling 6). Android's read is a
    // permission per API range: `READ_EXTERNAL_STORAGE` to 32,
    // `READ_MEDIA_IMAGES` from 33, `READ_MEDIA_VISUAL_USER_SELECTED` (a
    // partial grant) from 34; and `ACCESS_MEDIA_LOCATION`, without which
    // another app's photo reads with its location redacted (D4). A level
    // ignores a permission it does not define, so only a range's end is
    // written; the implementation asks those its level has. Adding needs no
    // permission from API 29, the host's oldest.
    Device {
        name: "photos",
        ios: &["NSPhotoLibraryUsageDescription"],
        ios_flags: &["PHPhotoLibraryPreventAutomaticLimitedAccessAlert"],
        android: &[
            "android.permission.READ_EXTERNAL_STORAGE maxSdkVersion=32",
            "android.permission.READ_MEDIA_IMAGES",
            "android.permission.READ_MEDIA_VISUAL_USER_SELECTED",
            "android.permission.ACCESS_MEDIA_LOCATION",
        ],
        macos: &["NSPhotoLibraryUsageDescription"],
        hardened: Some("com.apple.security.personal-information.photos-library"),
        web: None,
        keeps_no_answers: true,
        ..ROW
    },
    // HealthKit (LLP 1069.008.000): Apple only, a deviation (the web has no
    // Health permission). Read and write are HealthKit's own share/update
    // prompts, one key each; each carries the other's key as its companion,
    // and the HealthKit entitlement on iOS. macOS writes the keys and signs
    // with no entitlement, which is restricted there (D5). Not on tvOS.
    Device {
        name: "health-read",
        ios: &["NSHealthShareUsageDescription"],
        macos: &["NSHealthShareUsageDescription"],
        ios_entitlements: &["com.apple.developer.healthkit"],
        companion: Some("NSHealthUpdateUsageDescription"),
        tvos: false,
        keeps_no_answers: true,
        ..ROW
    },
    Device {
        name: "health-write",
        ios: &["NSHealthUpdateUsageDescription"],
        macos: &["NSHealthUpdateUsageDescription"],
        ios_entitlements: &["com.apple.developer.healthkit"],
        companion: Some("NSHealthShareUsageDescription"),
        tvos: false,
        keeps_no_answers: true,
        ..ROW
    },
    // The person's address book, `@exact/contacts` (LLP 1133.018 D11): a
    // deviation, the web has no store. Choosing contacts needs no line (D4).
    // An app reading it keeps no answers (D8).
    Device {
        name: "contacts",
        ios: &["NSContactsUsageDescription"],
        macos: &["NSContactsUsageDescription"],
        hardened: Some("com.apple.security.personal-information.addressbook"),
        android: &["android.permission.READ_CONTACTS"],
        tvos: false,
        keeps_no_answers: true,
        ..ROW
    },
    // The person's calendars, `@exact/calendar` (D11): full access, and
    // `WRITE_CALENDAR` on Android only where the app writes (`android_writes`).
    // Neither calendar line narrows the other: Apple's path between them is
    // an upgrade, so an app may write both.
    Device {
        name: "calendar",
        ios: &["NSCalendarsFullAccessUsageDescription"],
        macos: &["NSCalendarsFullAccessUsageDescription"],
        hardened: Some("com.apple.security.personal-information.calendars"),
        android: &["android.permission.READ_CALENDAR"],
        android_writes: &["android.permission.WRITE_CALENDAR"],
        tvos: false,
        keeps_no_answers: true,
        ..ROW
    },
    // Apple's write-only calendar access (iOS 17, macOS 14). Android has no
    // narrower grant: an insert names a calendar row, which only a read finds,
    // so the line derives both permissions there (D11).
    Device {
        name: "calendar-write-only",
        ios: &["NSCalendarsWriteOnlyAccessUsageDescription"],
        macos: &["NSCalendarsWriteOnlyAccessUsageDescription"],
        hardened: Some("com.apple.security.personal-information.calendars"),
        android: &[
            "android.permission.READ_CALENDAR",
            "android.permission.WRITE_CALENDAR",
        ],
        tvos: false,
        keeps_no_answers: true,
        ..ROW
    },
    // @ref LLP 1133.036 D7 — local network privacy (TN3179): macOS 15 and
    // iOS 14 ask before the app first browses or reaches a LAN address, and
    // show this text. The `net.dns-sd` and `local-network` lines it requires
    // derive `NSBonjourServices`, `NSAllowsLocalNetworking` and Android's
    // multicast permission (`bake/src/reach.rs`). Deviation: the web's Local
    // Network Access prompt is the browser's own, and Android asks nothing
    // until the host targets API 37, so the row writes Apple keys only.
    Device {
        name: "local-network",
        ios: &["NSLocalNetworkUsageDescription"],
        macos: &["NSLocalNetworkUsageDescription"],
        ..ROW
    },
    // @ref LLP 1133.010 D12 — local authentication. iOS shows the purpose the
    // first time Face ID is used, and refuses Face ID without the key; Touch ID
    // and macOS ask nothing. A declared deviation: the web has no name (its
    // cell is `later`, over passkeys). Not on tvOS, which has no biometry.
    Device {
        name: "local-auth",
        ios: &["NSFaceIDUsageDescription"],
        android: &["android.permission.USE_BIOMETRIC"],
        tvos: false,
        ..ROW
    },
    // @ref LLP 1125 D6 — Web MIDI's permission and policy feature. Apple asks
    // for nothing, so no usage key; tvOS has no CoreMIDI (`unavailable`).
    Device {
        name: "midi",
        web: Some("midi"),
        tvos: false,
        ..ROW
    },
    // @ref LLP 1133.013 D11 — `@exact/motion`. A declared deviation: the
    // Registry has three names and one line grants all three, the
    // magnetometer's whatever the app reads. iOS and Android ask nothing for
    // these sensors (`NSMotionUsageDescription` is the activity and
    // pedometer APIs'), and tvOS has none of Core Motion's streams.
    Device {
        name: "motion",
        web: Some("accelerometer"),
        policy: &["gyroscope", "magnetometer"],
        tvos: false,
        ..ROW
    },
    // Background work (LLP 1133.017 D14): nothing asks the person, so no
    // line names a purpose. The WICG specifications name the first two
    // (the Permissions Registry lists none of the three). The host keeps
    // one refresh identifier and two processing ones, by constraints (D8);
    // a `sync` waiting asks for both kinds. tvOS's refresh is later (D17):
    // its build writes none.
    Device {
        name: "background-sync",
        purpose: false,
        ios_modes: &["fetch", "processing"],
        ios_tasks: &["refresh", "processing.network", "processing.network-power"],
        ..ROW
    },
    Device {
        name: "periodic-background-sync",
        purpose: false,
        ios_modes: &["fetch"],
        ios_tasks: &["refresh"],
        ..ROW
    },
    Device {
        name: "background-fetch",
        purpose: false,
        ..ROW
    },
    // A login item and global shortcuts (LLP 1133.025 D10): nothing asks the
    // person, so the lines name no purpose; each is the app's statement of
    // what it may do, checked at bake. The installed Windows build's
    // `desktop:StartupTask` derives from `login-item`. Desktop only.
    Device {
        name: "login-item",
        purpose: false,
        tvos: false,
        ..ROW
    },
    Device {
        name: "global-shortcuts",
        purpose: false,
        tvos: false,
        ..ROW
    },
    // App Attest and Play Integrity (LLP 1133.028 D6): nothing is shown or
    // asked. The entitlement's value, `development` or `production`, is the
    // build's to choose (`host/apple/build.mjs`); a Mac takes it only with a
    // profile (D17), so no macOS key.
    Device {
        name: "attestation",
        purpose: false,
        ios_entitlements: &["com.apple.developer.devicecheck.appattest-environment"],
        ..ROW
    },
    // The stores' age signal (D10): the platform's sheet says what it asks,
    // and neither platform has a usage string. Not on tvOS.
    Device {
        name: "age-range",
        purpose: false,
        ios_entitlements: &["com.apple.developer.declared-age-range"],
        tvos: false,
        ..ROW
    },
    // App Tracking Transparency (D13): iOS and tvOS alone; Apple lists the
    // key for no Mac.
    Device {
        name: "tracking",
        ios: &["NSUserTrackingUsageDescription"],
        ..ROW
    },
    // Alarms, `@exact/alarms`'s (LLP 1133.030 D7), a declared deviation: no
    // web name. AlarmKit schedules nothing without its key. On Android the
    // package owns the app's one exact-alarm permission, needs notifications
    // for the ring's Stop, and runs receivers and a ringing service at a
    // boot and at the alarm's time; macOS, tvOS and the web have no alarm.
    Device {
        name: "alarms",
        ios: &["NSAlarmKitUsageDescription"],
        android: &[
            "android.permission.SCHEDULE_EXACT_ALARM",
            "android.permission.POST_NOTIFICATIONS",
            "android.permission.RECEIVE_BOOT_COMPLETED",
            "android.permission.WAKE_LOCK",
            "android.permission.FOREGROUND_SERVICE",
            "android.permission.FOREGROUND_SERVICE_SYSTEM_EXEMPTED",
        ],
        tvos: false,
        ..ROW
    },
    // NFC, `@exact/nfc`'s (LLP 1133.035 D15): iOS asks nothing, yet ends an
    // app that reads a tag without the usage key, and reads none without the
    // reader-session entitlement, whose value lists the formats (`TAG`, the
    // one Apple documents). Android's permission is a normal one, granted at
    // install. Web NFC has no policy feature; Core NFC is on neither the Mac
    // nor the TV.
    Device {
        name: "nfc",
        ios: &["NFCReaderUsageDescription"],
        android: &["android.permission.NFC"],
        ios_entitlements: &["com.apple.developer.nfc.readersession.formats"],
        ios_entitlement_values: &[("com.apple.developer.nfc.readersession.formats", &["TAG"])],
        tvos: false,
        ..ROW
    },
    // @ref LLP 1133.037 D17 — Web Serial, WebHID and WebUSB. No platform
    // shows a purpose for any of them, so no line names one. The web's
    // policy features are the APIs' names. Android declares USB host mode,
    // not required, for serial and USB; HID is absent there (D11). macOS
    // derives nothing while the app is not sandboxed. No device API exists
    // for apps on iOS or tvOS (D21).
    Device {
        name: "serial",
        web: Some("serial"),
        purpose: false,
        tvos: false,
        android_features: &["usb.host"],
        ..ROW
    },
    Device {
        name: "hid",
        web: Some("hid"),
        purpose: false,
        tvos: false,
        ..ROW
    },
    Device {
        name: "usb",
        web: Some("usb"),
        purpose: false,
        tvos: false,
        android_features: &["usb.host"],
        ..ROW
    },
];

/// One `device.<name> <key>` line.
#[derive(Debug, PartialEq, Eq)]
pub struct DeviceGrant<'a> {
    /// The table's row.
    pub device: &'static Device,
    /// The strings key whose text is the purpose; empty for a row that
    /// names none.
    pub purpose: &'a str,
}

/// Whether a grant line is a device grant (the runner's own family).
pub fn is_device_line(line: &str) -> bool {
    line.trim_start().starts_with("device.")
}

/// Every device grant in `spec`, or every malformed one, each reported.
pub fn device_grants(spec: &str) -> Result<Vec<DeviceGrant<'_>>, Vec<String>> {
    let (mut grants, mut errors) = (Vec::<DeviceGrant>::new(), Vec::new());
    for line in spec.lines().map(str::trim).filter(|l| is_device_line(l)) {
        let mut words = line.split_whitespace();
        let name = words
            .next()
            .unwrap_or_default()
            .trim_start_matches("device.");
        let Some(device) = DEVICES.iter().find(|d| d.name == name) else {
            let known: Vec<_> = DEVICES.iter().map(|d| d.name).collect();
            errors.push(format!(
                "`device.{name}` is not a device grant; the table has {}",
                known.join(", ")
            ));
            continue;
        };
        if line.contains(['"', '\'']) {
            errors.push(format!(
                "`{line}`: the purpose is a strings key (LLP 1060), not a literal: `device.{name} purpose.{name}`, with `\"purpose.{name}\": \"…\"` in strings/en.json"
            ));
            continue;
        }
        if !device.purpose {
            match words.next() {
                Some(word) => errors.push(format!(
                    "`{line}`: nothing asks the person for `device.{name}`, so its line names no purpose: write `device.{name}` (not `{word}`)"
                )),
                None if grants.iter().any(|g| g.device.name == name) => {}
                None => grants.push(DeviceGrant {
                    device,
                    purpose: "",
                }),
            }
            continue;
        }
        let (Some(purpose), None) = (words.next(), words.next()) else {
            errors.push(format!(
                "`{line}`: a device grant names one strings key, as in `device.{name} purpose.{name}`, whose text in strings/en.json (`\"purpose.{name}\": \"…\"`) is the purpose the person is shown"
            ));
            continue;
        };
        match grants.iter().find(|g| g.device.name == name) {
            Some(have) if have.purpose != purpose => errors.push(format!(
                "`device.{name}` names two purposes, `{}` and `{purpose}`",
                have.purpose
            )),
            Some(_) => {}
            None => grants.push(DeviceGrant { device, purpose }),
        }
    }
    for g in &grants {
        if let Some(needed) = g.device.needs {
            if !grants.iter().any(|h| h.device.name == needed) {
                errors.push(format!(
                    "`device.{}` needs `device.{needed} <purposeKey>` beside it (grant-needs)",
                    g.device.name
                ));
            }
        }
        if let Some(other) = g.device.conflicts {
            if grants.iter().any(|h| h.device.name == other) {
                errors.push(format!(
                    "`device.{}` and `device.{other}` both: no platform gives the first beside the second (grant-conflict)",
                    g.device.name
                ));
            }
        }
        if let Some(wider) = g.device.narrows {
            if grants.iter().any(|h| h.device.name == wider) {
                errors.push(format!(
                    "`device.{}` and `device.{wider}` both: write the one the app needs (grant-twice)",
                    g.device.name
                ));
            }
        }
    }
    if errors.is_empty() {
        Ok(grants)
    } else {
        Err(errors)
    }
}

/// Whether `spec` grants the device `name`: what a first-party capability
/// reads before it acts, refusing by name without it (D3).
pub fn granted(spec: &str, name: &str) -> bool {
    device_grants(spec).is_ok_and(|all| all.iter().any(|g| g.device.name == name))
}

/// Whether `spec` grants a device whose app keeps no answers
/// (LLP 1069.008.000 D7).
pub fn keeps_no_answers(spec: &str) -> bool {
    device_grants(spec).is_ok_and(|all| all.iter().any(|g| g.device.keeps_no_answers))
}

/// The served web's `Permissions-Policy` (D6): `feature=(self)` for each
/// granted row with a web name, `feature=()` for every other one the table
/// has. Built from the table's own names, so no app text reaches the header.
pub fn permissions_policy(spec: &str) -> String {
    let granted = device_grants(spec).unwrap_or_default();
    // A feature two rows share (`geolocation`) is named once: on when either is granted.
    let mut features: Vec<&str> = Vec::new();
    let of = |d: &'static Device| d.web.into_iter().chain(d.policy.iter().copied());
    for feature in DEVICES.iter().flat_map(of) {
        if !features.contains(&feature) {
            features.push(feature);
        }
    }
    features
        .into_iter()
        .map(|feature| {
            let on = granted.iter().any(|g| of(g.device).any(|f| f == feature));
            format!("{feature}=({})", if on { "self" } else { "" })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LLP 1133.002 D7: the approximate line is a row of its own, granting
    /// the same web feature; an app writes it or the precise one, not both.
    #[test]
    fn approximate_location_is_its_own_line_and_never_beside_the_precise_one() {
        let one = "device.geolocation-approximate locationPurpose";
        assert!(granted(one, "geolocation-approximate") && !granted(one, "geolocation"));
        let policy = permissions_policy(one);
        assert_eq!(policy.matches("geolocation=").count(), 1, "{policy}");
        assert!(policy.contains("geolocation=(self)"), "{policy}");
        let both = format!("{one}\ndevice.geolocation locationPurpose");
        let errors = device_grants(&both).unwrap_err();
        assert!(errors[0].contains("(grant-twice)"), "{errors:?}");
    }

    /// LLP 1133.021 D13: background location adds to precise location, and
    /// is refused without it and beside the approximate line.
    #[test]
    fn background_location_needs_precise_location() {
        let ok = "device.geolocation where\ndevice.geolocation-background arrival";
        assert!(granted(ok, "geolocation-background") && granted(ok, "geolocation"));
        let alone = device_grants("device.geolocation-background arrival").unwrap_err();
        assert!(alone[0].contains("(grant-needs)"), "{alone:?}");
        let approximate =
            "device.geolocation-approximate where\ndevice.geolocation-background arrival";
        let errors = device_grants(approximate).unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("(grant-needs)")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("(grant-conflict)")),
            "{errors:?}"
        );
    }

    #[test]
    fn a_device_line_names_a_row_and_one_key() {
        let spec = "net.fetch https://x/\ndevice.microphone purpose.mic\ndevice.speech-recognition purpose.speech";
        let all = device_grants(spec).unwrap();
        assert_eq!(
            all.iter()
                .map(|g| (g.device.name, g.purpose))
                .collect::<Vec<_>>(),
            [
                ("microphone", "purpose.mic"),
                ("speech-recognition", "purpose.speech")
            ]
        );
        assert!(granted(spec, "microphone") && !granted(spec, "camera"));
        let errors = device_grants(
            "device.lidar purpose.x\ndevice.camera\ndevice.camera \"Takes photos.\"\ndevice.microphone a b\ndevice.geolocation a\ndevice.geolocation b",
        )
        .unwrap_err();
        assert_eq!(errors.len(), 5, "{errors:?}");
        assert!(errors[0].contains("not a device grant"));
        assert!(errors[2].contains("not a literal"));
        assert!(errors[4].contains("two purposes"));
        // A row nothing asks about names no purpose (LLP 1133.017 D14).
        let quiet =
            device_grants("device.background-sync\ndevice.periodic-background-sync").unwrap();
        assert!(quiet.iter().all(|g| g.purpose.is_empty()));
        let errors = device_grants("device.background-fetch purpose.x").unwrap_err();
        assert!(errors[0].contains("names no purpose"), "{errors:?}");
        // LLP 1133.028 D6, D10, D13: attestation and the age range ask
        // nothing; tracking names the alert's purpose.
        let rows =
            device_grants("device.attestation\ndevice.age-range\ndevice.tracking purpose.ads")
                .unwrap();
        assert_eq!(
            rows.iter()
                .map(|g| (g.device.name, g.purpose))
                .collect::<Vec<_>>(),
            [
                ("attestation", ""),
                ("age-range", ""),
                ("tracking", "purpose.ads")
            ]
        );
        assert!(!rows[1].device.tvos && rows[0].device.tvos && rows[2].device.tvos);
    }

    #[test]
    fn health_is_two_rows_that_keep_no_answers() {
        // LLP 1069.008.000 D1, D7.
        let spec = "device.health-read purpose.read\ndevice.health-write purpose.write";
        let all = device_grants(spec).unwrap();
        assert_eq!(
            all.iter().map(|g| g.device.name).collect::<Vec<_>>(),
            ["health-read", "health-write"]
        );
        assert!(all.iter().all(|g| !g.device.tvos && g.device.web.is_none()));
        assert!(keeps_no_answers("device.health-read purpose.read"));
        assert!(!keeps_no_answers("device.microphone purpose.mic"));
        assert!(
            device_grants("device.health purpose.read purpose.write").is_err(),
            "the fork's one row with two keys"
        );
    }

    #[test]
    fn local_network_writes_the_apple_key_and_no_web_policy() {
        let all = device_grants("device.local-network purpose.lan").unwrap();
        let row = all[0].device;
        assert_eq!(row.ios, ["NSLocalNetworkUsageDescription"]);
        assert_eq!(row.macos, ["NSLocalNetworkUsageDescription"]);
        assert!(row.web.is_none() && row.android.is_empty() && row.hardened.is_none() && row.tvos);
        assert!(!permissions_policy("device.local-network purpose.lan").contains("local"));
    }

    #[test]
    fn the_policy_allows_what_is_granted_and_denies_the_rest() {
        assert_eq!(
            permissions_policy("device.microphone p\ndevice.speech-recognition q"),
            "microphone=(self), camera=(), geolocation=(), midi=(), accelerometer=(), gyroscope=(), magnetometer=(), serial=(), hid=(), usb=()"
        );
        assert_eq!(
            permissions_policy(""),
            "microphone=(), camera=(), geolocation=(), midi=(), accelerometer=(), gyroscope=(), magnetometer=(), serial=(), hid=(), usb=()"
        );
    }

    /// LLP 1133.033 D3: the background line stands beside the capture's own.
    #[test]
    fn a_capture_in_the_background_needs_the_microphone_line() {
        let errors = device_grants("device.microphone-background purpose.bg").unwrap_err();
        assert!(
            errors[0].contains("needs `device.microphone <purposeKey>` beside it (grant-needs)"),
            "{errors:?}"
        );
        let both =
            device_grants("device.microphone purpose.mic\ndevice.microphone-background purpose.bg")
                .unwrap();
        let row = both[1].device;
        assert_eq!(row.ios_modes, ["audio"]);
        assert_eq!(row.ios_flags, ["ExactCapturesInBackground"]);
        assert_eq!(row.android_services, ["capture:microphone"]);
        assert!(row.web.is_none() && !row.tvos && row.macos.is_empty());
    }

    /// LLP 1133.013 D11: one line turns on the three sensors' features.
    #[test]
    fn motion_is_one_line_for_three_policy_features() {
        let policy = permissions_policy("device.motion purpose.motion");
        for feature in ["accelerometer", "gyroscope", "magnetometer"] {
            assert!(policy.contains(&format!("{feature}=(self)")), "{policy}");
        }
        assert!(policy.contains("microphone=()"), "{policy}");
    }
}
