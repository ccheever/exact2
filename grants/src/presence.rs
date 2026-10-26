//! A `secret.keep` line's presence policy (LLP 1133.010 D4, D10): the words
//! after the name that make the platform's keystore release the value only
//! after the person's check.
//!
//! ```text
//! secret.keep discord-token user-presence optional apple.orWatch android.strongBox
//! ```
//!
//! One policy word, then `optional`, then platform words, each at most once
//! and in that order. The I/O grammar admits the name as it admits any kept
//! secret; the policy is read by the runner and the hosts that keep it.

/// The runner's own prefix for a presence name's occupancy and the host's
/// record of a move (LLP 1133.010 D4): no grant may name it.
pub const PRESENCE_PREFIX: &str = "exact.presence.";

/// Which check releases the value (D4's table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Policy {
    /// Biometry or the device credential; survives a change of enrolment.
    UserPresence,
    /// Biometry only; survives a change of enrolment.
    BiometryAny,
    /// Biometry only; lost when enrolment changes.
    BiometryCurrentSet,
    /// The passcode, PIN, pattern or password only.
    DeviceCredential,
}

impl Policy {
    /// Every policy, in the order a refusal lists them.
    pub const ALL: [Policy; 4] = [
        Policy::UserPresence,
        Policy::BiometryAny,
        Policy::BiometryCurrentSet,
        Policy::DeviceCredential,
    ];

    /// The grant word.
    pub fn word(self) -> &'static str {
        match self {
            Policy::UserPresence => "user-presence",
            Policy::BiometryAny => "biometry-any",
            Policy::BiometryCurrentSet => "biometry-current-set",
            Policy::DeviceCredential => "device-credential",
        }
    }

    /// The policy a grant word names.
    pub fn from_word(word: &str) -> Option<Policy> {
        Policy::ALL.into_iter().find(|p| p.word() == word)
    }

    /// Whether the check may use biometry, so may need Face ID's permission.
    pub fn may_use_biometry(self) -> bool {
        self != Policy::DeviceCredential
    }
}

/// The platform words a presence line may carry after its policy (D10).
pub const PLATFORM_WORDS: [&str; 5] = [
    "apple.accessible=whenPasscodeSetThisDeviceOnly",
    "apple.accessible=whenUnlockedThisDeviceOnly",
    "apple.orWatch",
    "apple.orCompanion",
    "android.strongBox",
];

/// A presence line's policy and words.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Presence {
    /// The check.
    pub policy: Policy,
    /// The person chooses whether the name is locked (`store.protect`).
    pub optional: bool,
    /// The platform words, as written.
    pub words: Vec<String>,
}

impl Presence {
    /// The words after the name, as a line writes them.
    pub fn line_words(&self) -> String {
        let mut out = self.policy.word().to_string();
        if self.optional {
            out.push_str(" optional");
        }
        for word in &self.words {
            out.push(' ');
            out.push_str(word);
        }
        out
    }

    /// Whether the line carries `word`.
    pub fn has(&self, word: &str) -> bool {
        self.words.iter().any(|w| w == word)
    }
}

/// Read the words after a `secret.keep` name: none is a plain secret, else
/// one policy, then `optional`, then platform words. A refusal starts with
/// its code, `grant-policy-unknown`.
pub fn presence_words(words: &[&str]) -> Result<Option<Presence>, String> {
    let Some((first, rest)) = words.split_first() else {
        return Ok(None);
    };
    let Some(policy) = Policy::from_word(first) else {
        let why = if *first == "optional" {
            "`optional` needs a presence policy before it".to_string()
        } else if PLATFORM_WORDS.contains(first) {
            format!("`{first}`: a platform word needs a presence policy before it")
        } else {
            format!("`{first}` is not a presence policy")
        };
        return Err(format!(
            "grant-policy-unknown: {why}; write one of {} after the name, then `optional`, then platform words",
            Policy::ALL.map(Policy::word).join(", "),
        ));
    };
    let mut optional = false;
    let mut platform: Vec<String> = Vec::new();
    for word in rest {
        if *word == "optional" && !optional && platform.is_empty() {
            optional = true;
        } else if PLATFORM_WORDS.contains(word) {
            let key = word.split('=').next().unwrap_or(word);
            if platform.iter().any(|w| w.split('=').next() == Some(key)) {
                return Err(format!("grant-policy-unknown: `{key}` is written twice"));
            }
            platform.push((*word).to_string());
        } else if *word == "optional" || Policy::from_word(word).is_some() {
            return Err(format!(
                "grant-policy-unknown: `{word}` is out of order: one policy, then `optional`, then platform words"
            ));
        } else {
            return Err(format!(
                "grant-policy-unknown: `{word}` is not a presence word ({})",
                PLATFORM_WORDS.join(", ")
            ));
        }
    }
    Ok(Some(Presence {
        policy,
        optional,
        words: platform,
    }))
}

/// The presence policy of one `secret.keep` line, when it carries one; `None`
/// for any other line, or one that does not read.
pub fn line_presence(line: &str) -> Option<(&str, Presence)> {
    let mut words = line.split_whitespace();
    if words.next()? != "secret.keep" {
        return None;
    }
    let name = words.next()?;
    let rest: Vec<&str> = words.collect();
    presence_words(&rest).ok().flatten().map(|p| (name, p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrantSet;

    #[test]
    fn a_policy_line_admits_its_name_and_reads_its_words_in_order() {
        let set = GrantSet::parse(
            "secret.keep token user-presence optional apple.orWatch android.strongBox\nsecret.keep plain",
        )
        .unwrap();
        assert_eq!(set.kept_secrets(), ["plain", "token"]);
        let (name, presence) = line_presence(
            "secret.keep token user-presence optional apple.orWatch android.strongBox",
        )
        .unwrap();
        assert_eq!(name, "token");
        assert_eq!(presence.policy, Policy::UserPresence);
        assert!(presence.optional);
        assert_eq!(presence.words, ["apple.orWatch", "android.strongBox"]);
        assert_eq!(
            presence.line_words(),
            "user-presence optional apple.orWatch android.strongBox"
        );
        for policy in Policy::ALL {
            let line = format!("secret.keep a {}", policy.word());
            assert_eq!(line_presence(&line).unwrap().1.policy, policy);
            assert!(GrantSet::parse(&line).is_ok(), "{line}");
        }
        assert!(line_presence("secret.keep plain").is_none());
    }

    #[test]
    fn each_bad_policy_line_is_refused_by_its_code() {
        for (line, says) in [
            ("secret.keep a face-id", "`face-id` is not a presence policy"),
            ("secret.keep a optional", "`optional` needs a presence policy"),
            (
                "secret.keep a apple.orWatch",
                "a platform word needs a presence policy",
            ),
            (
                "secret.keep a user-presence apple.orWatch optional",
                "`optional` is out of order",
            ),
            (
                "secret.keep a user-presence biometry-any",
                "`biometry-any` is out of order",
            ),
            (
                "secret.keep a user-presence optional optional",
                "`optional` is out of order",
            ),
            (
                "secret.keep a user-presence apple.orWatch apple.orWatch",
                "`apple.orWatch` is written twice",
            ),
            (
                "secret.keep a user-presence apple.accessible=whenPasscodeSetThisDeviceOnly apple.accessible=whenUnlockedThisDeviceOnly",
                "`apple.accessible` is written twice",
            ),
            (
                "secret.keep a user-presence apple.accessible=always",
                "`apple.accessible=always` is not a presence word",
            ),
        ] {
            let error = GrantSet::parse(line).unwrap_err();
            assert!(
                error.contains("grant-policy-unknown") && error.contains(says),
                "{line}: {error}"
            );
        }
    }

    #[test]
    fn no_grant_names_the_runners_presence_prefix() {
        let error = GrantSet::parse("secret.keep exact.presence.token").unwrap_err();
        assert!(error.contains("the runner's own"), "{error}");
    }
}
