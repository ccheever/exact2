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
    /// macOS `Info.plist` usage keys.
    pub macos: &'static [&'static str],
    /// The macOS hardened-runtime entitlement: without it a notarised build is
    /// silently refused, whatever `Info.plist` says.
    pub hardened: Option<&'static str>,
    /// The web `Permissions-Policy` feature, where the feature is policy-controlled.
    pub web: Option<&'static str>,
}

/// Every device grant. Adding a row is one entry here and no host code.
pub const DEVICES: &[Device] = &[
    Device {
        name: "microphone",
        ios: &["NSMicrophoneUsageDescription"],
        macos: &["NSMicrophoneUsageDescription"],
        hardened: Some("com.apple.security.device.audio-input"),
        web: Some("microphone"),
    },
    // Camera stays deferred (`rules/DEFERRED.md`); the row exists so a native
    // module can declare it.
    Device {
        name: "camera",
        ios: &["NSCameraUsageDescription"],
        macos: &["NSCameraUsageDescription"],
        hardened: Some("com.apple.security.device.camera"),
        web: Some("camera"),
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
    },
    // iOS's prompt text is fixed and the web's is not policy-controlled; the
    // grant still gates the request.
    Device {
        name: "notifications",
        ios: &[],
        macos: &[],
        hardened: None,
        web: None,
    },
    // Deviation: the web has no name; its speech API is gated by `microphone`.
    Device {
        name: "speech-recognition",
        ios: &["NSSpeechRecognitionUsageDescription"],
        macos: &["NSSpeechRecognitionUsageDescription"],
        hardened: None,
        web: None,
    },
    // Deviation, same reason: saving to the library. PhotoKit under the
    // hardened runtime needs the library entitlement for a write too.
    Device {
        name: "photos-add",
        ios: &["NSPhotoLibraryAddUsageDescription"],
        macos: &["NSPhotoLibraryAddUsageDescription"],
        hardened: Some("com.apple.security.personal-information.photos-library"),
        web: None,
    },
];

/// One `device.<name> <key>` line.
#[derive(Debug, PartialEq, Eq)]
pub struct DeviceGrant<'a> {
    /// The table's row.
    pub device: &'static Device,
    /// The strings key whose text is the purpose.
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
                "`{line}`: the purpose is a strings key (LLP 1060), not a literal"
            ));
            continue;
        }
        let (Some(purpose), None) = (words.next(), words.next()) else {
            errors.push(format!(
                "`{line}`: a device grant names one strings key, as in `device.{name} purpose.{name}`, whose text is the purpose"
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

/// The served web's `Permissions-Policy` (D6): `feature=(self)` for each
/// granted row with a web name, `feature=()` for every other one the table
/// has. Built from the table's own names, so no app text reaches the header.
pub fn permissions_policy(spec: &str) -> String {
    let granted = device_grants(spec).unwrap_or_default();
    DEVICES
        .iter()
        .filter_map(|d| {
            let feature = d.web?;
            let on = granted.iter().any(|g| std::ptr::eq(g.device, d));
            Some(format!("{feature}=({})", if on { "self" } else { "" }))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn the_policy_allows_what_is_granted_and_denies_the_rest() {
        assert_eq!(
            permissions_policy("device.microphone p\ndevice.speech-recognition q"),
            "microphone=(self), camera=(), geolocation=()"
        );
        assert_eq!(
            permissions_policy(""),
            "microphone=(), camera=(), geolocation=()"
        );
    }
}
