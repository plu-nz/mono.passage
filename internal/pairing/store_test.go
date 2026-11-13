package pairing

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

func TestPairingLockRevocationAndVaultIsolation(t *testing.T) {
	dir := t.TempDir()
	store, err := OpenAt(dir)
	if err != nil {
		t.Fatal(err)
	}
	const origin = "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/"
	const installation = "first-installation"
	code, _, err := store.Pair(origin, installation, "/vault/a")
	if err != nil || len(code) != 12 {
		t.Fatalf("pair: code=%q err=%v", code, err)
	}
	if _, err := store.Approve("wrong-code"); err == nil {
		t.Fatal("wrong code approved")
	}
	if _, err := store.Approve(code); err != nil {
		t.Fatal(err)
	}
	token, err := store.Poll(origin, installation, "/vault/a")
	if err != nil || len(token) != 64 {
		t.Fatalf("poll: token length=%d err=%v", len(token), err)
	}
	if again, err := store.Poll(origin, installation, "/vault/a"); err != nil || again != "" {
		t.Fatalf("token issued again: %q %v", again, err)
	}
	if allowed, _ := store.Authorized(origin, installation, token, "/vault/b"); allowed {
		t.Fatal("token from vault A authorized for vault B")
	}
	if allowed, _ := store.Authorized(origin, installation, token, "/vault/a"); !allowed {
		t.Fatal("valid token rejected")
	}
	if err := store.SetLocked(true); err != nil {
		t.Fatal(err)
	}
	reopened, err := OpenAt(dir)
	if err != nil {
		t.Fatal(err)
	}
	if locked, _ := reopened.Locked(); !locked {
		t.Fatal("lock not persisted")
	}
	if allowed, _ := reopened.Authorized(origin, installation, token, "/vault/a"); allowed {
		t.Fatal("locked store authorized token")
	}
	if err := reopened.SetLocked(false); err != nil {
		t.Fatal(err)
	}
	removed, err := reopened.Revoke(origin, installation)
	if err != nil || !removed {
		t.Fatalf("revoke: removed=%v err=%v", removed, err)
	}
	if allowed, _ := reopened.Authorized(origin, installation, token, "/vault/a"); allowed {
		t.Fatal("revoked token accepted")
	}
	path := filepath.Join(dir, "native-host.json")
	info, err := os.Stat(path)
	if err != nil || info.Mode().Perm() != 0600 {
		t.Fatalf("state permissions: %v %v", info, err)
	}
	var raw map[string]json.RawMessage
	data, _ := os.ReadFile(path)
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatal(err)
	}
	if string(raw["pending"]) != "[]" || string(raw["authorized"]) != "[]" {
		t.Fatalf("Rust-compatible arrays required: %s", data)
	}
}

func TestReadsRustStateAndRejectsUnscopedLegacyPairing(t *testing.T) {
	dir := t.TempDir()
	legacy := `{"pending":[],"authorized":[{"origin":"chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/","installation_id":"old","token_hash":"abc"}],"locked":false}`
	if err := os.WriteFile(filepath.Join(dir, "native-host.json"), []byte(legacy), 0600); err != nil {
		t.Fatal(err)
	}
	store, err := OpenAt(dir)
	if err != nil {
		t.Fatal(err)
	}
	paired, _, err := store.Status("chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/", "old", "/vault")
	if err != nil || paired {
		t.Fatalf("legacy unscoped authorization accepted: paired=%v err=%v", paired, err)
	}
}

func TestReadsScopedRustStateWithoutRePairing(t *testing.T) {
	dir := t.TempDir()
	const token = "old-rust-token"
	const origin = "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/"
	legacy := fmt.Sprintf(`{"pending":[],"authorized":[{"origin":%q,"installation_id":"old","token_hash":%q,"vault_path":"/vault"}],"locked":false}`, origin, fmt.Sprintf("%x", tokenHash(token)))
	if err := os.WriteFile(filepath.Join(dir, "native-host.json"), []byte(legacy), 0600); err != nil {
		t.Fatal(err)
	}
	store, err := OpenAt(dir)
	if err != nil {
		t.Fatal(err)
	}
	if valid, err := store.Authorized(origin, "old", token, "/vault"); err != nil || !valid {
		t.Fatalf("Rust token not accepted: valid=%v err=%v", valid, err)
	}
	if valid, _ := store.Authorized(origin, "old", token, "/another"); valid {
		t.Fatal("Rust token accepted for another vault")
	}
}
