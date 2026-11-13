# Passage

Passage is a cross-platform interface in development for vaults compatible with [GNU `pass`](https://www.passwordstore.org/). The vault belongs to you: your encrypted files are synchronized through a private Git repository that can be hosted on a Git service or on your own infrastructure.

Project site: [passage.plu.nz](https://passage.plu.nz/).

This document records the requirements discussed so far. It describes the desired product, and distinguishes implemented features from planned ones.

## Code status

The desktop app allows you to choose an existing GNU pass vault, create another one with a new or already available GPG key, list, search, open, create, edit and delete entries, generate TOTP and block access from extensions. The application also supports GitHub authentication through the GitHub CLI, initializing Git in the vault, configuring a remote, committing encrypted changes, Git sync, and preserving both versions of conflicting `.gpg` entries. Approval and revocation of extensions are in the interface, and the Native Messaging host offers search by URL and autofill after a user action. The controlled form test matrix passed on Brave Origin and stable Chrome; the real GitHub login form was also tested on both browsers with dummy credentials, without submitting the form.

| Directory | Responsibility |
| --- | --- |
| `core` | Shared TypeScript types for the vault and Native Messaging protocol. |
| `desktop` | Solid/Vite interface and Wails 3/Go application for Linux. |
| `extension` | Manifest V3 extension for Brave and Chrome, built with WXT. |
| `native-host` | Local Native Messaging connector for extensions. |
| `web` | Static landing page and generator of the Worker that publishes it. |

To develop on Linux, install Node.js, pnpm, Go 1.26+, GNU `pass`, GPG, GTK3 and WebKit2GTK 4.1. The desktop uses Wails 3.0.0 with the GTK3 backend at this stage; see [Wails](https://v3.wails.io/getting-started/installation/) Linux dependencies. In the repository root:

```bash
pnpm install
pnpm check
pnpm --filter @passage/desktop build
go run -tags gtk3 ./desktop
```

To generate the extension, run `pnpm build:extension` and load `extension/.output/chrome-mv3` as the unpacked extension into `brave://extensions` or `chrome://extensions`. Follow [local connector instructions](native-host/README.md) to compile it and register the extension IDs; the code shown by the popup can be approved in the desktop interface or through the CLI. To use the desktop, install GNU `pass` and GPG. The GPG key and `gpg-agent` remain under system control.

Browser testing uses a vault, HTTPS server, and disposable profiles, with the original extension manifest. After building the extension and host, run `node extension/tests/browser-smoke.mjs BROWSER_PATH HOST_PATH HOST_INSTALLER_PATH SCENARIO`. The scenarios are `standard`, `usernameOnly`, `passwordOnly`, `autocompleteOff`, `ambiguousPassword`, `ambiguousUsername`, `hiddenPassword`, `transparentPassword`, `newPassword`, `iframe`, `noFields`, `githubLive` and `herokuLive`. Each scenario checks pairing, entry listing, autofill result and refusal after navigation. `*Live` scenarios only populate test fields on external pages; they do not send forms. `githubLive` passed Brave Origin and Chrome stable; `herokuLive` passed in Chrome stable, but Brave did not load this page in this environment. Recent stable Chrome does not support `--load-extension`; run the test with `PASSAGE_BROWSER_HEADED=1` to load the extension into the disposable profile via the debug protocol. The matrix passed on Chrome stable and Brave Origin; the test requires OpenSSL.

### Open or create a desktop vault

In the **Vault** panel, enter the absolute directory path and choose **Open existing** for a vault already initialized with `.gpg-id`. To create, choose a private GPG key from the list and click **Create Vault**, or open **Create GPG Key and Vault** to generate a new key via GPG and initialize the vault in sequence. The passphrase is requested by the system's `pinentry`; Passage does not receive it. Passage uses `pass init` to create the GNU pass-compatible structure. To open a vault from another device, import the corresponding private key before trying to read the entries.

After you create a new key, keep a secure copy of the private key and passphrase. Git only syncs encrypted files in the vault; losing your private key prevents you from recovering your passwords. Passage shows the fingerprint of the created key and does not automatically export secret material. Key creation is a separate operation from vault initialization: if the second step fails, the generated key remains in GPG and can be used to try to create the vault again.

To back up, replace `FINGERPRINT` with the fingerprint shown in the app and choose a secure destination outside of the Git repository:

```sh
umask 077
gpg --armor --export-secret-keys FINGERPRINT > /safe/path/passage-secret-key.asc
```

To restore to another device, copy the backup file to a secure location and run `gpg --import /safe/path/passage-secret-key.asc`. Then confirm the key with `gpg --list-secret-keys --fingerprint` and test the reading of an entry with `PASSWORD_STORE_DIR=/path/to/vault pass show ENTRY`. Export, import and reading were verified with a disposable key and vault. Store the backup separately from the Git vault.

The chosen directory is saved in the local Passage configuration and is used by the desktop and the extensions connector. `PASSWORD_STORE_DIR`, when defined in the process environment, has priority and prevents changing the directory via the interface. In the terminal, to use a vault outside of `~/.password-store`, define the same variable, for example `PASSWORD_STORE_DIR=/path/to/vault pass ls`.

### Create and edit entries

With the vault open, click on **New Entry** and enter the entry path, password, username and URL. Passage writes via GNU `pass`: the password is on the first line, followed by `login:` and `url:`. The **Other Pass Lines** field accepts additional metadata, such as `otpauth:` or annotations. To edit an entry, open it and click **Edit**. To remove it, click **Delete** and confirm. These operations check whether the encrypted file has changed since reading; in this case, update the entry before trying again. The opened password is removed from the interface after 30 seconds; the creation form closes after 30 seconds without changing the password. The operation does not automatically make `git push`.

### Connect a GitHub account

Install [GitHub CLI (`gh`)](https://cli.github.com/). In the **Git Sync** panel, click **Sign in with GitHub**. Passage opens `gh auth login` in the default terminal; Complete your browser login and return to Passage to click **Verify Account**. If the default terminal cannot be opened, run `gh auth login --web` manually. The token is managed by `gh`, which can use secure system storage or, if that is not available, a local file; check `gh auth status` for the location used.

If the vault is already the root of a Git repository, **Use in this vault** only configures that vault's `.git/config` to ask `gh` for the HTTPS remote credentials on `github.com`. Passage does not receive or save the token. This step does not create a remote repository or change the existing remote; configure the URL and tracking branch via Git or `pass git`. SSH remotes and private Git servers continue to use their normal authentication.

On the same dashboard, **Start Git in this vault** creates a local repository in the `main` branch if the directory doesn't already have `.git`. **Add remote origin** registers the given URL without replacing an existing remote. Create an empty private Git repository first. **Publish new vault** checks that the remote has no history, creates the first commit with only `.gpg-id` and `.gpg` files (if there is no commit yet) and configures the upstream. The action rejects extra files, symbolic links, and files already staged in Git; never force shipping. If the remote already has history, resolve this connection through Git before syncing through Passage. The GPG private key remains outside the remote. Future changes can be registered with `pass git` on the terminal.

To retrieve a vault already published on another installation, enter a directory that does not yet exist and the repository URL in the **Vault** panel, then click **Clone existing vault**. The remote must have the branch `main` and `.gpg-id` in the root. Passage selects the vault after the clone; To open the entries, import the corresponding private GPG key separately. The clone uses the Git authentication already available on the system and does not modify an existing directory.

After creating, editing or deleting entries, **Record Changes** creates a commit with only files `.gpg` and `.gpg-id`; files outside this format, symbolic links or `.gpg` without encrypted OpenPGP packets cause the operation to stop. **Download** and **Upload** remain separate actions. If the histories diverge, **Preserve Both Versions** merges the commits and keeps the local version of each conflicting `.gpg` under its original name, saving the remote version as `nome.remote-HASH.gpg`. Review these entries after the merge and submit when you are satisfied. Conflicts in `.gpg-id`, concurrent removals, and renames require manual resolution via Git. Passage creates backup refs on `refs/passage/backup/` before the merge attempt.

### TOTP and extension blocking

To generate a code, store a URI `otpauth://totp/...` in a line of the entry, as `pass-otp` does, open the entry, and click **Generate TOTP Code**. Passage also reads a `otpauth: URI` line. The code stays in memory and disappears when it expires or after closing the entry; it is not recorded in the vault.

In the **Authorized Extensions** panel, **Block Extensions** prevents further reading by the Native Messaging host, even if the GPG key is cached. **Release Extensions** restores this access. The state persists until further action; blocking does not close the `gpg-agent` cache and does not prevent `pass show` in the terminal. An order already in progress may end before the block is applied.

## Installation on Linux

On Linux x86_64, with `curl`, `tar` and `sha256sum`:

```sh
curl -LsSf https://passage.plu.nz/install.sh | sh
```

The script downloads the ready-made binary from [latest release](https://github.com/plu-nz/mono.passage/releases/latest), checks SHA-256 and installs `~/.local/bin/passage` with a menu shortcut. Starting from `v0.2.1`, the package installs the unpacked extension to `~/.local/share/passage/extension-chromium` (or to `XDG_DATA_HOME/passage/extension-chromium`) and automatically registers the host for Brave and Chrome. Upload this folder once to `brave://extensions` and `chrome://extensions`, with developer mode enabled, and approve each browser in the **Authorized Extensions** panel. The ID of the unzipped extension is fixed: `edmjpcolampgjbnmldchmnkinmhakjgm`. You don't need to clone the repository or install Node.js, pnpm or Go to use the release. GNU `pass`, GPG, Python 3, GTK3 and WebKit2GTK 4.1 are runtime dependencies. `~/.local/bin` must be in the session's `PATH`.

The same command upgrades an existing installation to the latest release. Close and open Passage after updating and click **Reload** on the extension in `brave://extensions` and `chrome://extensions` to apply the new version. The installed version changes only when a new release is published.

The extension identity public key is at `extension/identity.json`. The corresponding private key was generated outside the repository, in `~/.local/share/passage/signing/extension.pem`, with access permission only to the user. Make an **encrypted** backup of it outside the Git vault; do not include it in the repository or package. It is required if a CRX distribution signed with the same ID is adopted. The unpacked extension only works with the public key from the manifest. A future store publication needs to confirm how to preserve the same ID before migration.

To publish a new Linux version from `main`, run with write access to the repository:

```sh
gh workflow run release-linux.yml --ref main -f version=0.2.2
gh run watch
```

The [workflow manual](.github/workflows/release-linux.yml) validates that the tag does not yet exist, compiles desktop, connector and extension in Ubuntu 22.04, runs the checks and publishes the files and their SHA-256 as the latest release. The version is reported without the prefix `v` and needs to be increased with each publication. The extension is published as a file to upload manually; Store posting and permanent IDs are still pending.

To compile and install from local source, use `./scripts/install-linux-user.sh`.

See [implementation decisions and backlogs](docs/decisions.md) to review the path taken.
The [execution plan](docs/execution-plan.md) details the Linux desktop and extensions deliverables, dependencies, and acceptance criteria.

## Principles

- The vault format remains compatible with GNU `pass`: a tree of `.gpg` files encrypted with OpenPGP.
- The terminal is a first-class interface. User can continue using `pass` and `pass git` without opening Passage.
- Private Git repositories transport the encrypted files. Private PGP keys, unlock passwords, and Git access credentials do not go into this repository.
- The same vault should be usable on desktop, Android, terminal, and the web, without relying on a server that knows the passwords in plaintext.
- The project must leverage existing components of the `pass` ecosystem when they meet the requirements; Passage-specific interface and integrations will be built where there are gaps.

## Platforms and forms of access

| Access method | Requirement |
| --- | --- |
| **Desktop** | Application to consult and manage the vault. Linux is the first platform. |
| **Android** | Application to consult and manage the same vault, with integration for filling in system credentials. |
| **Pure terminal** | Direct use of GNU `pass` and its Git commands, independent of Passage applications. |
| **Pure web** | Page accessible in a browser, including on another computer, capable of opening the vault without an application or desktop connector installed. |

iOS support is desirable in the future if technically feasible, but is out of initial scope.

## MVP scope

MVP delivers **desktop application for Linux and extensions for Brave and Chrome**. It solves the original problem: filling credentials in both browsers from a single local `pass` vault, without logging into each extension separately.

- The desktop application opens the existing vault, allows you to search and query logins and controls access to extensions.
- The extensions share an implementation for Chromium browsers, are individually paired with the desktop, and allow you to revoke each pairing.
- The extension identifies the open website, presents the corresponding entries and fills in the username and password after a user action.
- The vault remains usable by GNU `pass` in the terminal. Changes made by Passage must maintain compatibility of `.gpg` files.
- Desktop offers synchronization with private Git and presents conflicts to the user without silently overwriting an entry.
- The blocking behavior must be consistent with direct access by the `pass` and the `gpg-agent` cache.

**Existing integrations during MVP:** GNU `pass` covers the terminal; the [Android Password Store](https://github.com/agrahn/Android-Password-Store) fork covers Android. Passage does not need to implement its own CLI or Android app to validate the MVP.

**Further evolution:** independent web client, own Android application if the existing one leaves gaps, iOS if viable, passkeys and a biometric unlocking model that protects the key also on the first access after starting the system. TOTP can be evaluated at MVP if the existing format works without expanding the scope of integrations.

### Desktop and browsers

- Offer search, consultation and editing of vault items in the desktop application.
- Populate Brave and Chrome logins through extensions that access your local vault.
- Allow both browsers to use the same local unlock state without requiring an independent vault login in each extension.
- Pair each extension installation with the local application or service and allow you to revoke that access.
- Evaluate fingerprint unlocking, including on the first access after starting the system. The design needs to protect the key that opens the vault; the mere confirmation of a fingerprint does not replace this protection.

### Android

- Read and edit the same `.gpg` files used by `pass` on Linux.
- Integrate passwords with the filling provided by Android and, for passkeys, with the credential provider in supported versions.
- Protect the local key using the device's secure storage and authentication features.
- Sync with the same private Git, including a clear path to configure PGP key and repository access on mobile.

### Terminal

- Preserve operations such as `pass show`, `pass insert`, `pass edit`, `pass git pull`, and `pass git push`.
- Ensure that changes made on the terminal appear on other clients after synchronization.
- Document the relationship between application blocking, `gpg-agent` cache, and terminal access. The interface should not claim to have locked the entire vault if GPG can still open it.

### Web without a desktop connector

- Allow authentication to get encrypted `.gpg` files from a private Git repository.
- Decrypt the vault in the browser with a compatible OpenPGP implementation; the web service should not receive the open key, key password, or items in plaintext.
- Allow access on a new device. The process of obtaining an **encrypted** copy of the private key and recovering it needs to be defined before implementation.
- Separate service/Git authentication from vault decryption.
- Start with consultation and copying of credentials; define editing and pushing to Git with conflict control as a later step.
- Treat code delivered to the browser as part of the security surface: controlled distribution, no third-party scripts, XSS protection, restrictive content policy, and short session for sensitive material.

## Credentials

- Logins with username and password.
- TOTP secrets and two-factor authentication code generation.
- Passkeys: evaluate the file convention of the [Android Password Store](https://github.com/agrahn/Android-Password-Store/blob/develop/PasskeyStorage.md) fork and define how to create, use and synchronize these credentials on desktop and Android, keeping the files usable by `pass`.

## Sync and recovery

- Use a **private Git repository for the vault**, separate from the `plu-nz/mono.passage` code repository.
- Accept hosting on GitHub or a private Git server, including on the user's NAS.
- Detect simultaneous changes. Conflicts in the same entry cannot be resolved silently, as `.gpg` files do not support useful textual merging.
- Provide PGP key backup and recovery. Losing your cell phone, biometric reader or device cannot be the only cause of permanent loss of your vault.
- Inform that file names, folders and commit history are visible to anyone who has access to the Git repository, although the contents of the files are encrypted.

## Validation path

1. Set up a test vault with `pass` and private Git, without real credentials.
2. Try Browserpass on Brave and Chrome to identify what the extension itself needs to add; confirm that the same vault opens in the Android Password Store.
3. Prototype the Linux application and pair it with a Chromium extension.
4. Validate interoperable reading and writing with `pass`, filling in both browsers and Git synchronization with conflicts.
5. Plan web, own Android, passkeys and advanced biometrics from the validated MVP.

## Open technical decisions

- Interface technology: Solid/Vite with Wails 3 and Go core shared with the host; extensions remain in TypeScript/WXT.
- Direct use of Browserpass or creating your own extension and connector.
- Biometric unlock model on Linux, local key storage and interaction with `gpg-agent`.
- How to provide an encrypted copy of the PGP key for emergency web access and how to authorize the web service to read private Git.
- Interoperable format for site metadata, TOTP and passkeys.
- Synchronization policy, concurrent editing and conflict recovery on each platform.

## References

- [GNU pass](https://www.passwordstore.org/)
- [Browserpass](https://github.com/browserpass/browserpass-extension)
- [Android Password Store, fork with passkeys](https://github.com/agrahn/Android-Password-Store)
- [OpenPGP.js](https://docs.openpgpjs.org/)
