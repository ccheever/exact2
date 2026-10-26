# Code review: relay deploy (R4 prep), security pass (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b64f9b280d`.
- **Method:** one brief (sha256 `ff1c1977793b312c54ecb1c40bb337928bfdcab16bce9439bdcabe43da4b8287`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **“Keys only” is not verified.** [relay/deploy/remote.sh:67](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:67)  
   `sshd -t` correctly precedes reload, but checks syntax, not the effective authentication policy. OpenSSH uses the first value encountered; an earlier setting or applicable `Match` block can defeat this drop-in. I confirmed that `PasswordAuthentication yes` followed by `no` still evaluates to `yes`. Validate effective settings with `sshd -T -C` for relevant users/connections, reject conflicting configuration, and verify a fresh key login before tightening the firewall.

2. **Firewall replacement fails open.** [relay/deploy/remote.sh:90](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:90)  
   `ufw reset` disables the firewall before the replacement rules are installed. Every differing configuration creates an exposure window; a subsequent command failure leaves UFW disabled. Allowing 22 before `enable` protects the eventual SSH rule, not this interval. Update active rules without disabling protection, or retain an independently enforced deny policy throughout replacement and verify the final policy.

3. **A first deployment does not survive reboot.** [relay/deploy/remote.sh:186](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:186), [relay/deploy/remote.sh:252](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:252)  
   Provisioning skips `enable` when no release exists. The subsequent successful release restarts the service but never enables it. Thus the documented `provision` → `release` sequence leaves the public relay disabled at boot. Enable the unit after the first release passes health checks.

4. **The deployed gate can miss TLS interception of pairing.** [scripts/tamber-relay-gate.mjs:129](/tmp/rv-rt1-r4b/scripts/tamber-relay-gate.mjs:129)  
   `saw.set(to, fingerprint)` overwrites earlier observations. Line 143 consequently checks only the transaction request’s certificate, after pairing and hello have already sent credentials. A relay presenting another trusted certificate for those earlier requests, then passing the transaction through, can receive `MET`. Check the expected local certificate fingerprint **before sending each request**, including the separate long-poll connection and both accounts’ tool/canvas connections; retain failures rather than overwriting them.

**Non-blocking**

- **The sandbox permits persistent authorization changes.** [remote.sh:63](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:63), [remote.sh:178](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:178)  
  The service owns the accounts directory and `accounts.json`, so compromised relay code can add accounts, replace hashes, and rewrite revocation history. Watching the directory requires no write permission. Make authorization files and their parent root-managed; provide narrowly scoped write access for the precreated `labels.seen` file. Preserve atomic account replacement and directory watching. Resource limits would also contain memory/process exhaustion better.

- **Compromise still exposes substantial authority.** [remote.sh:161](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:161)  
  The relay and DNS hook can read the control key and DNS token, execute programs as the service user, and make outbound connections, including to local services. DNS authority can obtain fresh certificates for relay-controlled session names; absence of stored session keys does not prevent future interception of unpinned clients. Scope the provider token narrowly. The existing capability limit and filesystem protections do prevent ordinary writes to installed binaries, system configuration, and root’s home.

- **Secret permissions and token output are generally sound.** [remote.sh:122](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:122), [tamber-relay-server.mjs:560](/tmp/rv-rt1-r4b/scripts/tamber-relay-server.mjs:560)  
  The control key and initial `dns.env` are root/service-group readable under restricted directories. Accounts start at `0640`, become `0600` after CLI replacement, and `labels.seen` is created `0600`. `accounts-add` generates its token internally and sends it through SSH stdout; these scripts do not put it in argv, shell history, or the service journal. Terminal recording or output redirection can retain it. **`EnvironmentFile=` alone normally exposes its pathname through `systemctl show`, not its contents through `Environment=`**; the token nevertheless enters the process and child environments.

- **Root smoke execution unnecessarily enlarges the release’s privileges.** [remote.sh:240](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:240)  
  The digest is checked before execution, and private uploads prevent an unrelated local user substituting the file. This is not an unauthenticated upload-to-root route. Nevertheless, compromised build output executes unrestricted code through `--help`, before the service sandbox applies. Copy it into root-owned staging and smoke-test it as the service user under equivalent restrictions. The digest establishes byte integrity, not trustworthy source/toolchain provenance. Initial SSH trust also uses `accept-new`; independently pinning the host key strengthens bootstrap.

- **Upload cleanup does not cover transfer/setup failure.** [deploy.sh:50](/tmp/rv-rt1-r4b/relay/deploy/deploy.sh:50), [deploy.sh:64](/tmp/rv-rt1-r4b/relay/deploy/deploy.sh:64)  
  Remote cleanup traps exist only after `remote.sh` starts. Failed SCP, interrupted deployment, or failure to start the remote command leaves uploads—including potentially the private key—in `/tmp`. They remain protected by the `0700` directory, but are not “removed whatever happens.” Add cleanup immediately after directory creation and a recovery/expiry policy for unreachable-host leftovers.

- **IPv6 and lock-out checks need explicit postconditions.** [remote.sh:83](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:83)  
  Seeing port 22 in `sshd -T` does not prove the operator can reach it through provider rules, forwarding, or the active SSH listener. UFW’s existing IPv6 configuration is neither enforced nor checked after rebuilding. Verify effective rules for both families and a fresh usable SSH connection. The relay itself binds only IPv4. The common-port TCP probe is not an inventory of listeners, UDP exposure, IPv6 exposure, or provider-firewall rules.

- **Health can accept an unknown-SNI connection that never closes.** [remote.sh:304](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:304)  
  `timeout` failure is discarded, and absence of a certificate becomes “closed.” An in-memory probe of this function returned `health: OK` for a simulated timeout. Require observed closure and reject timeout. The certificate check also establishes SAN matching, not public trust. Separately, [deploy.sh:73](/tmp/rv-rt1-r4b/relay/deploy/deploy.sh:73) uses `x509 -ext`, unsupported by this Mac’s system LibreSSL, and its connection probes have no overall deadline. Select a supported parser and bound them.

- **The remaining R4 results have narrower meanings than full deployment qualification.** [tamber-relay-gate.mjs:105](/tmp/rv-rt1-r4b/scripts/tamber-relay-gate.mjs:105)  
  The polling counter observes handler entry, not registration of a held long poll. Plain-HTTP testing counts connection errors as successful rejection. The capture observes encrypted traffic through a local tap, not remote memory or logs. Cross-account routing, TXT refusal, cookie rejection, and pairing-limit checks do exercise the deployed connection, but paired-client revocation does not test relay-account revocation. Supplied certificates bypass ACME entirely; without them, each run creates and deletes fresh production ACME state. Add precise readiness/deadline checks, identify skipped issuance explicitly, and reuse certificate state.

No exploitable command injection or traversal was found through the normal deployment arguments: quoting, generated upload paths, digest-derived release names, and root-owned release directories constrain those paths. The broad cleanup glob at [remote.sh:225](/tmp/rv-rt1-r4b/relay/deploy/remote.sh:225) should still be restricted to exact generated upload-directory names.

No provider DNS-hook implementation or lego acquisition/renewal procedure exists in the reviewed scripts, so those steps remain unverified.

Review was read-only and offline. Shell syntax passed; failure-path probes used mocked operations. No deployment or network gate was run.
