# Full Migration Plan: Wails 3 + Solid

**Planning branch:** `plan/wails3-solid-migration`
**Status:** implementation completed on this branch; local validation and Ubuntu CI completed.
**Objective:** replace the Tauri 2/React desktop and the Native Messaging Rust host with Wails 3/Solid and Go, maintaining GNU `pass`/GPG/Git as the source of truth, the operation in the terminal and the current extension in Brave/Chrome.

## Migration result

- Desktop in Solid/Vite and Wails 3.0.0, compiled with `-tags gtk3` and WebKit2GTK 4.1 to maintain support for this installation's Linux and the Ubuntu 22.04 runner. `@wailsio/runtime` is fixed at 3.1.0; the bridge was exercised in the real application.
- Go core in `internal/{vault,setup,gitvault,github,otp,pairing}` shared by desktop and Go host. Rust, Tauri and React sources have been removed.
- Native Messaging Protocol v1, extension ID and executable names preserved. The host integration test and real tests on Brave and Chrome passed with disposable profiles.
- Release installation tested twice with a local server and disposable HOME. Checksums, manifest and lock state were preserved.
- Branch CI passed on Ubuntu 22.04 ([run 43332456516](https://github.com/plu-nz/mono.passage/actions/runs/43332456516)). The Wails desktop and Go host were also installed on the local user; the hashes match the generated binaries and the command `passage` opened the window. The public installer continues to point to the `main` branch release until the migration is integrated and published.

Local measurement on 11/12/2026, with warm cache: desktop Go build 2.343 s; TypeScript check and Vite build 2,773 s. The Wails binary was 17,719,872 bytes; the Tauri binary installed before migration is 13,589,880 bytes. These are point-in-time measurements on this computer, not a comparison of clean builds or memory in use.

## 1. Scope and architectural decisions

- `src/desktop`: Solid + TypeScript + Vite no frontend; Wails 3 + Go no processo desktop.
- `src/native-host`: separate Go executable, with the same installed name (`passage-native-host`) and the same Native Messaging v1 protocol. Desktop and host import a shared Go core, without relying on an open window.
- `src/extension`: Keep WXT/TypeScript and published ID. Change only if a protocol adjustment is essential; any incompatible changes require versioned migration and browser testing.
- `packages/core`: keep TypeScript contracts useful for the extension and UI; generate or check types from the Go→TypeScript bridge to avoid divergent definitions.
- GNU `pass`, GPG, `pinentry`, `gpg-agent`, Git and `gh` remain responsible for their respective data/authentication. Do not reimplement OpenPGP or import private keys into the Passage process.
- Preserve `PASSWORD_STORE_DIR`, `~/.password-store`, `XDG_CONFIG_HOME/passage/settings.json`, pairing state, remote URLs, `.gpg` and `.gpg-id` files. Migration should not require reconnecting browser or recreating vault.
- Keep two executables in the package initially. This preserves the installation and prevents a browser invocation from initializing the graphics runtime. Consolidating into a binary can only be a later, measured and tested optimization.
- Pin an exact stable version of Wails 3 to `go.mod`, CLI and npm runtime; Do not use `@latest` in the CI. Check the Linux version and dependencies at the beginning of the implementation.

## 2. Current inventory and contracts to preserve

| Area | Current source | Contract to be preserved |
| --- | --- | --- |
| Vault | `src/desktop/src-tauri/src/vault.rs`, `setup.rs` | Choose/create/clone the vault; validation of ID, path and symlinks; read/write by `pass`; ciphertext hash for concurrency; parsing user, URL and extra fields. |
| Git/GitHub | `git.rs`, `github.rs` | Commit only `.gpg`/`.gpg-id`, clear text opt-out, fast-forward, initial publish, backups and merge that preserves both encrypted versions; authentication delegated to `gh`. |
| TOTP | `otp.rs` | Parsing of `otpauth://`, algorithms/periods/digits, RFC 6238 vectors, secret in memory only. |
| Pairing | `pairing.rs`, `src/native-host/src/main.rs` | Approval code, token per installation and vault, revocation, blocking, HTTPS origin, double authorization check. |
| Extension | `src/extension` | v1 messages, fixed ID, existing pairing, explicit choice before filling out, tab and URL check. |
| UI | `src/desktop/src/main.tsx`, `style.css` | Current flows and states, secret clearing in 30 s, green visual identity and compact layout. |
| Distribution | `scripts/*.sh`, `.github/workflows/release-linux.yml` | `~/.local/bin/passage`, host, extension, Brave/Chrome manifests, checksum, installer without clone and update without deleting pairings. |

Before porting code, register fixtures **without real secrets** for entry formats, JSON state, host messages, and expected command output/errors. Check all commands exposed in `src/desktop/src-tauri/src/lib.rs` and messages in `src/native-host/src/main.rs`; The table above is a summary, not the exhaustive list.

## 3. Execution steps

### Step 0 — Baseline and Linux feasibility

1. Freeze current behavior with throwaway vault, key, Git remote, and browser profiles. Run current tests and record pre-existing failures.
2. Measure clean build, backend rebuild, frontend-only build, boot time, package size and Tauri memory usage on the same hardware. Separate hot/cold cache and register toolchain version; the 6 min 30 s compilation made with new cache is just initial data.
3. Make a minimal Wails 3 + Solid spike on the target Linux, with Go→UI call, window, `pinentry` and external process. Check `wails3 doctor`, GTK4/WebKitGTK 6.0, Wayland/X11 and packaging. On this machine, `webkitgtk-6.0` was not available in `pkg-config` on 11/12/2026; resolve this or explicitly set the distro floor before approving the port.
4. Document supported Linux matrix. The legacy GTK3 path should not support the new distribution if the chosen Wails version does not already support it. Run `passage` without internet access and confirm that the built-in frontend opens.

**Gate:** Wails prototype runs on target Linux and the list of supported distros is explicit. If it fails, keep the branch as a study and do not replace the installed app.

### Step 1 — Core Go without UI

1. Create an internal Go module (`internal/vault`, `internal/git`, `internal/otp`, `internal/pairing`, `internal/config` or equivalent), with small interfaces for subprocesses and filesystem. Avoid coupling to Wails.
2. Port vault discovery and selection, protection against symlinks and path traversal, listing, reading, creation, editing and removal. Use `pass` and GPG like today. Preserve the order and content of unedited lines.
3. Port Git and GitHub CLI with the same opt-outs, backup refs, and file limits. Never execute shell commands with interpolated text; use process arguments.
4. Port TOTP and pairing/blocking state. Read data persisted by the Rust version without reformatting or invalidating existing tokens. If there is a schema change, perform idempotent migration with backup and rollback testing.
5. Test parity against disposable fixtures and vaults: GPG errors, concurrent edit, symlink, extra files in Git, divergent remote, `.gpg-id` conflict, and lock change while reading.

**Gate:** Go core passes behavior and security tests before being exposed to the frontend or extension.

### Step 2 — Go Native Messaging host

1. Port Chromium framing (4 little-endian bytes + JSON), message limits, `requestId`, generic errors, timeouts and process cleanup. Reserve `stdout` exclusively for the protocol; logs in `stderr` without secrets.
2. Preserve the v1 protocol and existing administration CLI commands (`list`, `list-json`, `lock-status`, `lock`, `unlock`, approval/revocation as per complete inventory).
3. Reuse the Go core to solve vault and entry. Revalidate token, origin and lock immediately before responding; preserve the token's link to the vault.
4. Directly test the binary with `src/native-host/tests/integration.py`, adapting only the executable path. Then test real Brave and Chrome on disposable profiles, including same-tab browsing, revocation, blocking, and paired installation.

**Gate:** Existing extension works without mandatory update, without re-pairing, and without weakening the controls documented in `docs/security-review.md`.

### Step 3 — Wails 3 + Solid desktop

1. Create the Wails shell in `src/desktop`, register Go services with explicit methods and generate typed bindings. Do not expose arbitrary access to filesystem, external command or secret via the bridge.
2. Port React UI to Solid components by flow: vault/configuration, list and detail, create/edit/delete, TOTP, Git/GitHub and extensions. Preserve current CSS/branding, loading states and errors.
3. Implement service adapter in the frontend to reduce component coupling to Wails bindings; Maintain shareable TypeScript contracts with the extension when it makes sense.
4. Preserve the cleanliness of secrets displayed after 30 s, clear state when changing vaults/entrance and when blocking extension. Do not claim that state clearing guarantees erases process memory or cache of the `gpg-agent`.
5. Test keyboard, focus, compact window scrolling, no vault/key/Git states, and `pinentry` dialogs. Ensure your app doesn't lock the UI during `pass`, Git, or `gh`.

**Gate:** all current desktop flows work in Solid/Wails with the same local data and no security regressions.

### Step 4 — Installation, CI, and Transition

1. Update `scripts/install-linux-user.sh`, `package-linux.sh`, release workflow, binary paths, icon, `.desktop` and documentation. Maintain the installed names and release file format as long as possible.
2. Test update on a Tauri/Rust installation with vault and disposable pairings: install Wails/Go, launch app, read/write, use Brave/Chrome, update again. Do not delete `~/.config/passage` or the vault.
3. Test installation without repository cloned by `curl -LsSf https://passage.plu.nz/install.sh | sh` against test release. Check SHA-256, registered host and existing extension.
4. Measure clean build/rebuild again, size, startup and memory under the same conditions as step 0. Document the actual result, including if Go does not bring relevant gains on Linux because of CGO/WebKit.
5. Remove Tauri/React/Rust and dependencies only after all gates. Update `README.md`, `docs/decisions.md`, execution plan and security review. Publish release only after CI and local update testing pass.

**Final Gate:** the new package replaces the old one without loss of vault, GPG authentication, pairing tokens or `pass` compatibility on the terminal.

## 4. Security and compatibility: blocking criteria

- No secrets, GitHub tokens, or GPG passphrases in logs, error messages, telemetry, or fixtures.
- No changes to the extension identity or Native Messaging manifest that force new pairing without an explicit plan.
- No `pass show` triggered just by listing entries; decrypt only after user selection/action.
- No access from the visited web page to the Wails bridge or host beyond the extension protocol and existing checks.
- Preserve secure path guarantees, input validation, hash concurrency, and Git limits. Parity failure blocks removal of Rust binary.
- Review the WebView Wails content and loading policy; load only embedded assets and development endpoints in dev mode. Perform specific review of the Go surface exposed to JavaScript.

## 5. Sequence of deliveries and estimation for planning

| Delivery | Dependency | Relative size | Evidence of completion |
| --- | --- | --- | --- |
| Baseline and spike Linux | none | small | dependency matrix and reproducible benchmarks |
| Vault Go Core/Git/TOTP | baseline | big | parity testing and disposable vault |
| Host Go | core | medium | protocol v1 and Brave/Chrome validated |
| Solid + Wails | core | big | equivalent desktop streams |
| CI, installer and test release | host + desktop | medium | Tauri→Wails upgrade and clone-free installation |
| Cleanup and stable release | all gates | small | documentation and final package verified |

Estimates are relative; Calendar time depends mainly on the Rust logic port (~3.5 thousand lines between desktop and host), availability of WebKitGTK 6.0 on Linux targets and parity tests. The React→Solid switch is a smaller part of the total migration.

## 6. Out of scope

Own Android/iOS client, pure web, biometrics, new vault features and publication of the extension in store. The Go architecture should not impede these works, but Wails mobile is not yet an assumed stable basis for them. Pure terminal remains available via GNU `pass`; No need to replace your CLI.
