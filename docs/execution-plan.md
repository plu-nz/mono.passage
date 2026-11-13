# Execution plan: Linux desktop and extensions (history)

The desktop and host have been migrated to Wails 3, Solid, and Go. For the current state and migration testing, see [migration plan](wails3-solid-migration-plan.md). The Tauri/Rust tasks below record the sequence of the previous MVP.

This plan turns [README](../README.md) requirements into verifiable deliverables. Covers the Linux application, Brave/Chrome extensions, interoperability with GNU `pass` in the terminal, Git, security and distribution. Android, web client and iOS are excluded from this plan. Estimates are the approximate effort of a person familiar with Rust, TypeScript, and Chromium extensions, not promised deadlines.

## Expected outcome

A local installation of Passage uses an existing GNU `pass` vault. The user unlocks his GPG key by the system mechanism once; Brave and Chrome can request filling from the same local service, without an independent password per extension. Each extension installation must be authorized and can be revoked. The user can also read and edit entries on the desktop, use `pass` in the terminal, and synchronize encrypted files with a private Git, with explicit conflicts.

## Implementation rules

- Preserve the format `.gpg`, the directory structure, `.gpg-id`, and the behavior of `pass`; Do not implement your own OpenPGP encryption in MVP.
- Treat the local Rust process as the authority to access the vault. The Tauri interface and extensions are clients; share domain rules, entry format, and protocol in testable code.
- Do not consider the GPG key blocked just because the interface hid the password: the `gpg-agent` cache is independent.
- Do not store open credentials in logs, temporary files, Git, extension storage, or telemetry.
- Test real integration with Brave and Chrome and a disposable vault before marking a step as complete.

## Milestone 0 — Environment and Functional Reference (1–2 days)

1. Create a disposable vault with test GPG key, subfolder entries, multiple recipients via `.gpg-id`, URLs, user, TOTP, and test private remote Git. Do not use real credentials.
2. Exercise `pass show`, `insert`, `edit`, `rm`, `git pull` and `git push`; record the initial field convention and expected behavior of `gpg-agent`.
3. Run `pnpm check`, build the desktop and extension, Rust tests and installation using the current release. Log execution dependencies and reproducible failures.
4. Validate desktop app against test vault: `PASSWORD_STORE_DIR`, missing `pass`, incorrect password in GPG, update after external change and symbolic files.

**Accepted:** reproducible test script and disposable vault; reading on the desktop and via the terminal confirmed; documented current limitations. This step establishes the reference for the following ones.

## Milestone 1 — Vault Core and Entry Format (3–5 days)

1. Extract from the Tauri command the Rust logic of finding, listing, reading, creating, updating, and removing entries. Validate IDs, canonical path, symlinks, empty names, subprocess errors and UTF-8 result.
2. Define an interoperable text convention: first line as password; fields `login`, `url` and `otpauth` in the following lines. Preserve unknown lines and existing order when editing. Document how an old item without metadata appears.
3. Specify source normalization and matching: schema, host, subdomain, and port; avoid matching by insecure suffix (e.g. `example.com.evil.test`). The tab domain will be obtained by the extension, never accepted from the page content.
4. Implement operations via `pass` and round-trip testing with `pass show`, including editing via the terminal followed by updating on the desktop. Handle changed inputs externally before writing.
5. Define stable errors for missing vault, unavailable key, invalid item, edit conflict and Git, without secreting messages.

**Accept:** creating, editing and removing by core keeps files readable by GNU `pass`; unknown fields survive editing; Malicious entries do not escape the vault. Automated tests only use disposable vaults.

## Milestone 2 — Local service and extension protocol (4–7 days)

1. Implement Native Messaging host executable in Rust, with JSON messages framed by the Chromium protocol, size limits, protocol version, timeouts and clean termination. Reuse the Milestone 1 core without relying on an open Tauri window.
2. Decide and set the authorization model: each installation of the extension identifies itself and requests pairing on the desktop; explicit user authorization; local credential per protected installation in the user profile; listing and revocation. An extension ID allowed in the host manifest is necessary, but not sufficient for instance authorization.
3. Set minimum orders `status`, `pair`, `listForOrigin` and `readForOrigin`. Link each sensitive request to the tab origin and authorized scope; validate version, identity, pairing status, and order size on the host.
4. Avoid indiscriminate enumeration of entries by the browser. The host only returns candidates for the requested source and only releases the password after explicit action in the extension. Do not send secrets in response to `ping` or while the popup is just opening.
5. Record process decision: Native Messaging host initiated on demand for each browser, with persisted authorizations and system-shared GPG. If a Tauri window is required to approve pairing, expose only that local stream.
6. Perform protocol testing with truncated input, invalid JSON, incompatible version, invalid origin, unpaired identity, and revocation during a session.

**Accept:** Brave and Chrome can call the installed host; an unpaired installation does not read inputs; revocation prevents subsequent requests; Closing the Tauri app does not render the integration inoperative. There is never a password in the error output or logs.

## Milestone 3 — Brave and Chrome Extensions and Autofill (4–7 days)

1. Distribute the same WXT/Manifest V3 base for both browsers. Pin extension IDs per distribution channel and register each ID in host manifests, without broad origin permissions by default.
2. In the popup, obtain the URL and identity of the active tab via the extension's API, request candidates from the host, display title/user and allow choosing an entry. Only ask for site permission when necessary.
3. Implement user/password filling script after click. Check tab, frame and origin immediately before injection; refuse privileged pages and change navigation. Do not autocomplete or send secret to DOM without user action.
4. Support usual forms, two-step login and fields with input/change events. Treat absence or ambiguity of fields without filling an arbitrary destination; inform the user when it is necessary to copy manually.
5. Clean up references to the secret in the popup/background after the operation, within the limits of the JavaScript runtime. Do not persist passwords in `storage`, history or console.
6. Test pairing, selection, and filling in Brave and Chrome with clean profiles and controlled test pages. Include trying a similar site, iframe from another source, and navigation between selection and completion.

**Accept:** an entry from the same vault populates correctly in both browsers after a single applicable GPG authentication; each extension is paired/revoked independently; an unmatched site receives no secret.

## Milestone 4 — Complete desktop, Git, and terminal (5–8 days)

1. Add to desktop creation, editing and deletion with proper confirmation, preservation of unknown fields and update after external changes. View user, URLs and metadata without revealing password by default.
2. Implement Git status, pull and push for user-configured remote, without taking over GitHub. Display branch, remote, local changes, and credential errors usefully without echoing tokens.
3. To synchronize, detect unsaved local work and remote divergence. In conflict on the same `.gpg` entry, stop the operation and offer an explicit choice between versions or backup; never replace silently. Do not attempt to merge ciphertext as text.
4. Ensure that `pass git` and changes made directly to the repository continue to work. Update the UI when the vault changes on disk or provide clear and reliable update.
5. Document GPG key backup/recovery and remote access; highlight that entry names and Git history are visible to anyone accessing the repository.

**Accept:** creating and editing in Passage reflects in `pass` and vice versa; synchronization with GitHub and generic Git server works with authentication already configured on the system; reproduced conflict does not lose either version.

## Milestone 5 — Lockdown, TOTP, and biometrics (3–6 days for lockout/TOTP; biometrics dependent on prototype)

1. Define what the “block” button controls: clear UI secrets and suspend sensitive requests from extensions immediately. Display the status of `gpg-agent` separately, as it may continue to release `pass show` on the terminal.
2. Choose session and reauthorization policy for extension, including expiration, screen lock, and revocation. Check the actual limitations of the GPG cache before promising a one-time unlock under all circumstances.
3. Support TOTP per convention compatible with `pass-otp` when present; Generate in-memory-only codes and test interoperability. Don't change the files just to show the code.
4. Prototype Linux biometrics with test GPG key. The fingerprint must release protected cryptographic material or a system mechanism that releases it; a biometric confirmation on an already opened key does not satisfy unlocking after restart. Evaluate integration with PAM/FIDO2 or secure storage available via distribution and password/recovery key fallback.
5. Only assume biometrics as a requirement delivered after demonstrating the first access after boot, recovery without reader and behavior consistent with `gpg-agent` and `pass` on the terminal. If the prototype does not achieve this, document the limitation and keep it out of the stable release.

**Accept:** Passage blocking cuts access to extensions even with GPG cached; UI correctly describes what the lock covers; TOTP matches `pass-otp`; Biometrics is end-to-end proof or remains explicitly experimental.

## Milestone 6 — Distribution, security, and release (3–5 days)

1. Package Tauri app, Native Messaging host, manifests for Brave/Chrome and extensions as versioned set. The remote installer must install/update each part and offer clean removal, without requiring a clone of the repository.
2. Automate build, types, Rust/TypeScript tests, artifacts and checksums in CI. Fix dependency versions and consider signing artifacts before recommending `curl | sh` as the main installation path.
3. Review extension permissions, injection surfaces, protocol, local storage of pairings, subprocesses, protection against arbitrary paths and accidental exposure of secrets. Perform manual review with a disposable vault.
4. Run final matrix: Brave and Chrome on new and already used profiles; local vault with and without Git; absence of `pass`; GPG with and without cache; lock/revocation; installer without cloned repo; upgrade from a previous version.
5. Update README, installation page and decisions with actually available features and known limitations. Publish release only after matrix passes.

**Accept:** installing on Linux x86_64 by `passage.plu.nz/install.sh` leaves the desktop and connection with both browsers usable; artifacts are reproducible from commit; README does not promise outstanding functions.

## Order, dependencies and deliveries

| Sequence | Depends on | Reviewable delivery | Effort |
| --- | --- | --- | --- |
| 0. Functional reference | Current code | Test vault and test script | 1–2 days |
| 1. Core and format | 0 | CRUD compatible with `pass` | 3–5 days |
| 2. Host and pairing | 1 | Authorized local protocol | 4–7 days |
| 3. Extensions | 2 | Brave/Chrome autofill | 4–7 days |
| 4. Desktop and Git | 1; revised integrations with 2–3 | Editing and sync without silent data loss | 5–8 days |
| 5. Blocking and TOTP | 2–4 | Session policy and codes; separate biometric prototype | 3–6 days + research |
| 6. Release | 0–5 | Full installer, CI and documentation | 3–5 days |

The critical path to validate the idea is **0 → 1 → 2 → 3**. After this flow works in both browsers, move on to Git, locking and stable distribution. Total estimated work set: **23–40 business days**, plus biometric investigation, which has no reliable estimate prior to prototype.

## Decisions to be recorded throughout execution

- Final convention of fields `login`, `url` and `otpauth`; behavior with old entries and multiple URLs.
- Stable IDs and distribution channel for extensions; host installation format and location in each browser.
- Installation identity, authorization storage, and pairing interaction with the desktop window.
- Lock policy regarding `gpg-agent`, host session and initial unlock.
- Git strategy for dirty directory, conflict and recovery; choice of Git library or CLI.
- TOTP compatibility with `pass-otp` and biometrics feasibility after boot.

Each milestone must update [decisions.md](decisions.md), README, and acceptance criteria with the observed outcome, including security or scope deviations.
