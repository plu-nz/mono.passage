# Local connector (prototype)

The Native Messaging host serves Brave and Chrome using the user's GNU `pass` vault. It is launched by the browser on demand, without requiring the desktop window to be open. The desktop and CLI share pairing state in the Go package `internal/pairing`.

The release installer installs `passage-native-host` alongside the desktop. When the release contains the extension, the installer automatically registers the fixed ID `edmjpcolampgjbnmldchmnkinmhakjgm` in browsers; you still need to load the extension folder in each browser and approve each pairing. To register another build manually, enter the IDs displayed by browsers:

```sh
passage-install-native-host "$HOME/.local/bin/passage-native-host" BRAVE_ID CHROME_ID
```

You can also set `PASSAGE_BRAVE_EXTENSION_ID` and `PASSAGE_CHROME_EXTENSION_ID` before running the installer, to override the default ID in one or both browsers. Older releases without a host in the archive remain installable but cannot connect to browsers.

## Build and register

```sh
go build -o native-host/build/passage-native-host ./native-host
sh scripts/install-native-host.sh \
  "$PWD/native-host/build/passage-native-host" \
  ID_SHOWN_IN_BRAVE_EXTENSIONS \
  ID_SHOWN_IN_CHROME_EXTENSIONS
```

Extension IDs appear in each browser's manager after loading the unpacked extension. The script registers manifests for stable Brave and Chrome; also registers existing Brave Beta, Nightly and Brave Origin profiles. Python 3 is required.

To test the protocol without touching the vault or real profiles:

```sh
go test ./internal/pairing ./native-host
python3 native-host/tests/integration.py "$PWD/native-host/build/passage-native-host"
```

## Pair and revoke

The popup requests pairing and shows a 12-character code that expires in 5 minutes. Confirm on the desktop interface or via the terminal:

```sh
passage-native-host approve CODE
passage-native-host list
passage-native-host revoke chrome-extension://EXTENSION_ID/ INSTALLATION_ID
```

`list` shows the source and installation ID; `revoke` blocks follow-up requests. The token issued upon approval is sent once to the browser and stored only as a hash on the host. The local state is at `~/.config/passage/native-host.json` (or `XDG_CONFIG_HOME/passage`) with user permissions.

Each pairing is only valid for the vault selected when approved. When choosing another vault on the desktop, the extension needs to be paired again to access it. The desktop interface shows only pairings for the current vault; `list` in CLI shows all to allow revocation. If you reselect a previous vault, the old pairing for that vault remains valid until revoked.

The desktop can block extension access with `passage-native-host lock` and allow them again with `passage-native-host unlock`. The state persists after closing the application or restarting the computer; `passage-native-host lock-status` queries it. Blocking cuts off new requests even when the `gpg-agent` still has the key in cache. It does not block the direct use of `pass` on the terminal or delete a password that is already visible on a browser page.

## Current limitations

- The extension needs to use the tab origin obtained from the browser API and validate the tab again before populating. The host requires HTTPS origin and exact match to a `url` field of the entry.
- The host decrypts entry metadata to find matches; `pass`/GPG may request the key passphrase at this time. A large vault can slow down the search.
- The origin of the extension is reported by the browser as an argument to the process. Manifests limit the IDs accepted by the browser; another local process from the same user can invoke the executable directly and, therefore, token and approval are still required.
- The protocol and installer have not yet been subjected to the final Brave/Chrome testing matrix. Use a disposable vault at this stage.
