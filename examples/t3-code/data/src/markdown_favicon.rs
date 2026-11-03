// A web link's favicon, as MarkdownExternalLink's MarkdownLinkFavicon picks it (T3 Code 5d3d78ab93; MIT, see
// LICENSE-T3): a source-control host's mark ("brand:github", …; sourceControlClients.findByPublicDomain), else the
// public favicon service's icon (shared/favicon.ts faviconUrlForOrigin) for a host that may be disclosed to it
// (shared/hostClassification.ts isPublicFaviconHost), else "" (the globe). An IPv6 literal gets the globe: a privacy-safe
// false negative where upstream would ask the service about a public address.

/// findByPublicDomain's public hosts and their marks; a subdomain counts.
const SOURCE_CONTROL_DOMAINS: [(&str, &str); 5] = [
    ("github.com", "github"),
    ("gitlab.com", "gitlab"),
    ("codeberg.org", "forgejo"),
    ("git.cafe", "gitcafe"),
    ("bitbucket.org", "bitbucket"),
];

fn link_favicon(href: &str) -> String {
    let Some(host) = web_hostname(href) else {
        return String::new();
    };
    if let Some((_, mark)) = SOURCE_CONTROL_DOMAINS
        .iter()
        .find(|(domain, _)| host == *domain || host.ends_with(&format!(".{domain}")))
    {
        return format!("brand:{mark}");
    }
    if !public_favicon_host(&host) {
        return String::new();
    }
    let encoded: String = host
        .bytes()
        .map(|byte| match byte {
            b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (byte as char).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect();
    format!("https://www.google.com/s2/favicons?domain={encoded}&sz=32")
}

/// An http(s) URL's hostname, lowercased (`URL.hostname`): no user info or port; None for an IPv6 literal.
fn web_hostname(href: &str) -> Option<String> {
    let lower = href.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority.rsplit('@').next().unwrap_or("");
    if host_port.starts_with('[') {
        return None;
    }
    let host = host_port.split(':').next().unwrap_or("");
    (!host.is_empty()).then(|| host.to_string())
}

fn public_favicon_host(host: &str) -> bool {
    // One trailing dot is an absolute DNS name; more can hide a numeric form such as 127.1.
    if host.ends_with("..") {
        return false;
    }
    let host = host.trim_end_matches('.');
    let under = |suffix: &str| host == suffix || host.ends_with(&format!(".{suffix}"));
    if host == "localhost"
        || under("localhost")
        || host.ends_with(".local")
        || under("home.arpa")
        || !host.contains('.')
        || host.ends_with(".ts.net")
        || ["alt", "example", "internal", "invalid", "onion", "test"]
            .iter()
            .any(|suffix| under(suffix))
    {
        return false;
    }
    let parts: Vec<Option<u8>> = host.split('.').map(|part| part.parse().ok()).collect();
    if parts.len() == 4 && parts.iter().all(Option::is_some) {
        let [a, b, c, _] = [parts[0], parts[1], parts[2], parts[3]].map(Option::unwrap);
        let special = a == 0
            || a == 10
            || a == 127
            || (a == 100 && (64..=127).contains(&b))
            || (a == 172 && (16..=31).contains(&b))
            || (a == 192 && b == 168)
            || (a == 169 && b == 254)
            || (a == 198 && (18..=19).contains(&b))
            || a >= 224
            || (a == 192 && b == 0 && (c == 0 || c == 2))
            || (a == 192 && b == 88 && c == 99)
            || (a == 198 && b == 51 && c == 100)
            || (a == 203 && b == 0 && c == 113);
        return !special;
    }
    true
}

#[cfg(test)]
mod favicon_tests {
    use super::link_favicon;

    #[test]
    fn a_public_host_asks_the_favicon_service() {
        assert_eq!(
            link_favicon("https://Example.com/plan"),
            "https://www.google.com/s2/favicons?domain=example.com&sz=32"
        );
        assert_eq!(
            link_favicon("http://user@docs.rs:8080/x?y#z"),
            "https://www.google.com/s2/favicons?domain=docs.rs&sz=32"
        );
    }

    #[test]
    fn a_source_control_host_has_its_mark() {
        assert_eq!(link_favicon("https://github.com/o/r"), "brand:github");
        assert_eq!(link_favicon("https://gist.github.com/x"), "brand:github");
        assert_eq!(link_favicon("https://codeberg.org/x"), "brand:forgejo");
        assert_eq!(
            link_favicon("https://notgithub.com/"),
            "https://www.google.com/s2/favicons?domain=notgithub.com&sz=32"
        );
    }

    #[test]
    fn a_private_or_reserved_host_keeps_the_globe() {
        for href in [
            "http://localhost:3000/",
            "http://127.0.0.1/",
            "http://192.168.1.4/",
            "http://printer.local/",
            "https://intranet/",
            "https://box.tail1234.ts.net/",
            "https://site.example/",
            "https://name.test/",
            "http://[2001:db8::1]/",
            "mailto:someone@example.com",
        ] {
            assert_eq!(link_favicon(href), "", "{href}");
        }
        assert_eq!(
            link_favicon("http://8.8.8.8/"),
            "https://www.google.com/s2/favicons?domain=8.8.8.8&sz=32"
        );
    }
}
