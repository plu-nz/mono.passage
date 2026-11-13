package vault

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestValidateAndPreserveFields(t *testing.T) {
	for _, id := range []string{"../x", "x/../y", "-x", "x//y", "x.gpg", "x\ny"} {
		if ValidateID(id) == nil {
			t.Errorf("accepted %q", id)
		}
	}
	original := "old\ncustom: before\nlogin: alice\notpauth: test\nurl: https://first.example\ncustom: after\n"
	secret := ParseSecret(original)
	if secret.OtherDetails != "custom: before\notpauth: test\ncustom: after\n" {
		t.Fatalf("other details: %q", secret.OtherDetails)
	}
	updated, err := replaceKnown(original, "new", "bob", "https://new.example")
	if err != nil {
		t.Fatal(err)
	}
	if updated != "new\ncustom: before\nlogin: bob\notpauth: test\nurl: https://new.example\ncustom: after\n" {
		t.Fatalf("ordering lost: %q", updated)
	}
}
func TestDisposablePassStoreAndConflict(t *testing.T) {
	if _, err := exec.LookPath("pass"); err != nil {
		t.Skip("pass absent")
	}
	if _, err := exec.LookPath("gpg"); err != nil {
		t.Skip("gpg absent")
	}
	home := t.TempDir()
	gnupg := filepath.Join(home, "gnupg")
	if err := os.Mkdir(gnupg, 0700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("GNUPGHOME", gnupg)
	store := filepath.Join(home, "store")
	key := exec.Command("gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "", "--quick-generate-key", "Passage Go Test <go-test@example.invalid>", "default", "default", "1d")
	if out, err := key.CombinedOutput(); err != nil {
		t.Fatalf("gpg: %v %s", err, out)
	}
	out, err := exec.Command("gpg", "--with-colons", "--fingerprint", "--list-secret-keys").Output()
	if err != nil {
		t.Fatal(err)
	}
	fingerprint := ""
	for _, line := range strings.Split(string(out), "\n") {
		if strings.HasPrefix(line, "fpr:") {
			fields := strings.Split(line, ":")
			fingerprint = fields[9]
			break
		}
	}
	if fingerprint == "" {
		t.Fatal("missing fingerprint")
	}
	init := exec.Command("pass", "init", fingerprint)
	init.Env = append(os.Environ(), "PASSWORD_STORE_DIR="+store)
	if out, err := init.CombinedOutput(); err != nil {
		t.Fatalf("pass init: %v %s", err, out)
	}
	v, err := Open(store)
	if err != nil {
		t.Fatal(err)
	}
	created, err := v.CreateLogin("sites/example.com", "first", "alice", "https://example.com", "otpauth: test\ncustom: keep\n")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(store, "sites/example.com.gpg")); err != nil {
		t.Fatal(err)
	}
	if created.Password != "first" || created.Username == nil || *created.Username != "alice" {
		t.Fatalf("bad secret %+v", created)
	}
	stale := created.CiphertextHash
	external := exec.Command("pass", "insert", "-m", "-f", "sites/example.com")
	external.Env = append(os.Environ(), "PASSWORD_STORE_DIR="+store)
	external.Stdin = strings.NewReader("external\nlogin: alice\nurl: https://example.com\notpauth: test\ncustom: keep\n")
	if out, err := external.CombinedOutput(); err != nil {
		t.Fatalf("pass insert: %v %s", err, out)
	}
	if _, err := v.UpdateLogin("sites/example.com", "overwrite", "bob", "https://example.com", "", stale); err == nil {
		t.Fatal("stale update accepted")
	}
	fresh, err := v.Read("sites/example.com")
	if err != nil {
		t.Fatal(err)
	}
	if fresh.Password != "external" {
		t.Fatal("external change lost")
	}
	saved, err := v.UpdateLogin("sites/example.com", "new", "bob", "https://new.example", fresh.OtherDetails, fresh.CiphertextHash)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(saved.OtherDetails, "custom: keep") || !strings.Contains(saved.OtherDetails, "otpauth: test") {
		t.Fatalf("fields lost: %+v", saved)
	}
	if err := os.Symlink(filepath.Join(store, "sites/example.com.gpg"), filepath.Join(store, "alias.gpg")); err == nil {
		if _, err := v.Read("alias"); err == nil {
			t.Fatal("symlink accepted")
		}
	}
}
