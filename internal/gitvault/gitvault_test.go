package gitvault

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func run(t *testing.T, dir string, command string, args ...string) string {
	t.Helper()
	c := exec.Command(command, args...)
	if dir != "" {
		c.Dir = dir
	}
	out, err := c.CombinedOutput()
	if err != nil {
		t.Fatalf("%s %v: %v: %s", command, args, err, out)
	}
	return string(out)
}

func encrypt(t *testing.T, store, fingerprint, value string) {
	t.Helper()
	c := exec.Command("gpg", "--batch", "--yes", "--trust-model", "always", "--recipient", fingerprint, "--encrypt", "--output", filepath.Join(store, "login.gpg"))
	c.Stdin = strings.NewReader(value)
	if out, err := c.CombinedOutput(); err != nil {
		t.Fatalf("encrypt: %v: %s", err, out)
	}
}

func TestPublishCommitAndPreserveDivergentCiphertext(t *testing.T) {
	for _, command := range []string{"git", "gpg", "pass"} {
		if _, err := exec.LookPath(command); err != nil {
			t.Skip(command + " unavailable")
		}
	}
	root := t.TempDir()
	gnupg := filepath.Join(root, "gnupg")
	if err := os.Mkdir(gnupg, 0700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("GNUPGHOME", gnupg)
	t.Setenv("GIT_CONFIG_GLOBAL", filepath.Join(root, "gitconfig"))
	run(t, "", "git", "config", "--global", "user.name", "Passage Test")
	run(t, "", "git", "config", "--global", "user.email", "passage@example.invalid")
	run(t, "", "gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "", "--quick-generate-key", "Passage Test <passage@example.invalid>", "default", "default", "1d")
	fingerprint := ""
	for _, line := range strings.Split(run(t, "", "gpg", "--with-colons", "--fingerprint", "--list-secret-keys"), "\n") {
		if strings.HasPrefix(line, "fpr:") {
			fingerprint = strings.Split(line, ":")[9]
			break
		}
	}
	if fingerprint == "" {
		t.Fatal("missing fingerprint")
	}
	store := filepath.Join(root, "store")
	t.Setenv("PASSWORD_STORE_DIR", store)
	run(t, "", "pass", "init", fingerprint)
	encrypt(t, store, fingerprint, "initial\n")
	remote := filepath.Join(root, "remote.git")
	run(t, "", "git", "init", "--bare", remote)
	if _, err := Initialize(store); err != nil {
		t.Fatal(err)
	}
	first, err := Open(store)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := first.SetRemote(remote); err != nil {
		t.Fatal(err)
	}
	if _, err := first.PublishNewVault(); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(store, "notes.txt"), []byte("plaintext"), 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := first.CommitChanges(); err == nil {
		t.Fatal("plaintext file accepted for Git commit")
	}
	if err := os.Remove(filepath.Join(store, "notes.txt")); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(store, "login.gpg"), []byte("plaintext"), 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := first.CommitChanges(); err == nil {
		t.Fatal("invalid ciphertext accepted for Git commit")
	}
	encrypt(t, store, fingerprint, "initial\n")
	peer := filepath.Join(root, "peer")
	run(t, "", "git", "clone", "--branch", "main", remote, peer)
	second, err := Open(peer)
	if err != nil {
		t.Fatal(err)
	}
	encrypt(t, store, fingerprint, "local\n")
	if _, err := first.CommitChanges(); err != nil {
		t.Fatal(err)
	}
	encrypt(t, peer, fingerprint, "remote\n")
	if _, err := second.CommitChanges(); err != nil {
		t.Fatal(err)
	}
	if err := second.Push(); err != nil {
		t.Fatal(err)
	}
	status, err := first.MergePreservingBoth()
	if err != nil {
		t.Fatal(err)
	}
	if status.Ahead == 0 || status.Behind != 0 {
		t.Fatalf("unexpected merge status: %+v", status)
	}
	matches, err := filepath.Glob(filepath.Join(store, "login.remote-*.gpg"))
	if err != nil || len(matches) != 1 {
		t.Fatalf("expected encrypted remote copy: %v %v", matches, err)
	}
	if got := strings.TrimSpace(run(t, "", "pass", "show", "login")); got != "local" {
		t.Fatalf("local version lost: %q", got)
	}
	out, err := exec.Command("gpg", "--batch", "--decrypt", matches[0]).Output()
	if err != nil || strings.TrimSpace(string(out)) != "remote" {
		t.Fatalf("remote version lost: %q (%v)", out, err)
	}
}
