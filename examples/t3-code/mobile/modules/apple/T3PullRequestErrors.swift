#if os(iOS)
// Source: T3PanelsNative.swift:156 at 887b2491b182f851b11253655f6aa84fe2a26708.
// Pure protocol-error text extracted app-locally because the desktop owner imports AppKit.
import Foundation
enum T3PullRequestErrors {
    private static let requirement: [String: (missing: String, unauthenticated: String)] = [
        "github": ("GitHub CLI (`gh`) is required to browse change requests on this host. Install it from https://cli.github.com/ and reload.",
                   "GitHub CLI is not authenticated. Run `gh auth login` and retry."),
        "forgejo": ("Install Forgejo CLI (`fj` 0.6 or later) from https://codeberg.org/forgejo-contrib/forgejo-cli or Gitea CLI (`tea` 0.16 or later) from https://gitea.com/gitea/tea to browse Forgejo pull requests.",
                    "Authenticate your Forgejo or Gitea server with `fj --host <server-url> auth add-token` on the T3 Code server. If fj is missing or unconfigured for that server, use `tea login add`. A configured fj account must be repaired with fj."),
        "gitlab": ("GitLab CLI (`glab`) is required to browse change requests on this host. Install it from https://gitlab.com/gitlab-org/cli and reload.",
                   "GitLab CLI is not authenticated. Run `glab auth login` and retry."),
        "azure-devops": ("Azure CLI (`az`) with the Azure DevOps extension is required. Install `az`, then run `az extension add --name azure-devops`.",
                         "Azure CLI is not signed in. Run `az login` and retry."),
        "bitbucket": ("Bitbucket needs API credentials on the server. Add them in Settings → Source Control.",
                      "Bitbucket rejected the configured credentials. Check them in Settings → Source Control."),
    ]
    static func unavailable(reason: String, provider: String) -> String {
        let known = requirement[provider]
        switch reason {
        case "cli-missing": return known?.missing ?? "The tool this host is read through is not installed or set up."
        case "cli-unauthenticated": return known?.unauthenticated ?? "This host has no working credentials."
        case "provider-unsupported": return "Change requests cannot be browsed for this project's host yet."
        default: return ""
        }
    }
}

#endif
