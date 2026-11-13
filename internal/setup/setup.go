package setup

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/plu-nz/mono.passage/internal/github"
	"github.com/plu-nz/mono.passage/internal/gitvault"
)

type GPGKey struct {
	ID    string `json:"id"`
	Label string `json:"label"`
}
type VaultInfo struct {
	Path          string   `json:"path"`
	Source        string   `json:"source"`
	Initialized   bool     `json:"initialized"`
	RecipientIDs  []string `json:"recipientIds"`
	GPGKeys       []GPGKey `json:"gpgKeys"`
	PassInstalled bool     `json:"passInstalled"`
	GPGInstalled  bool     `json:"gpgInstalled"`
}
type CreatedKeyVault struct {
	Info        VaultInfo `json:"info"`
	Fingerprint string    `json:"fingerprint"`
}
type settings struct {
	VaultPath string `json:"vaultPath"`
}

var fingerprintPattern = regexp.MustCompile(`^[a-fA-F0-9]{40}([a-fA-F0-9]{24})?$`)

func settingsPath() (string, error) {
	base := os.Getenv("XDG_CONFIG_HOME")
	if base == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			return "", errors.New("HOME is not defined")
		}
		base = filepath.Join(home, ".config")
	}
	if !filepath.IsAbs(base) {
		return "", errors.New("XDG_CONFIG_HOME must be an absolute path")
	}
	return filepath.Join(base, "passage", "settings.json"), nil
}
func ActiveVaultPath() (string, string, error) {
	if p, ok := os.LookupEnv("PASSWORD_STORE_DIR"); ok {
		return p, "environment", nil
	}
	config, err := settingsPath()
	if err != nil {
		return "", "", err
	}
	data, err := os.ReadFile(config)
	if err == nil {
		var s settings
		if json.Unmarshal(data, &s) != nil || !filepath.IsAbs(s.VaultPath) {
			return "", "", errors.New("Invalid vault configuration. Fix settings.json")
		}
		return s.VaultPath, "selection", nil
	}
	if !errors.Is(err, os.ErrNotExist) {
		return "", "", errors.New("Unable to read Passage settings")
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return "", "", errors.New("HOME is not defined")
	}
	return filepath.Join(home, ".password-store"), "default", nil
}
func IsInitialized(path string) bool {
	info, err := os.Stat(path)
	if err != nil || !info.IsDir() {
		return false
	}
	info, err = os.Lstat(filepath.Join(path, ".gpg-id"))
	return err == nil && info.Mode().IsRegular()
}
func VaultInfoCurrent() (VaultInfo, error) {
	path, source, err := ActiveVaultPath()
	if err != nil {
		return VaultInfo{}, err
	}
	recipients := []string{}
	if IsInitialized(path) {
		data, _ := os.ReadFile(filepath.Join(path, ".gpg-id"))
		for _, line := range strings.Split(string(data), "\n") {
			line = strings.TrimSpace(line)
			if line != "" && !strings.HasPrefix(line, "#") {
				recipients = append(recipients, line)
			}
		}
	}
	return VaultInfo{Path: path, Source: source, Initialized: IsInitialized(path), RecipientIDs: recipients, GPGKeys: GPGKeys(), PassInstalled: available("pass"), GPGInstalled: available("gpg")}, nil
}
func available(name string) bool { return exec.Command(name, "--version").Run() == nil }
func allowed() error {
	if _, ok := os.LookupEnv("PASSWORD_STORE_DIR"); ok {
		return errors.New("PASSWORD_STORE_DIR is defined and takes precedence over the selection in Passage")
	}
	return nil
}
func persist(path string) error {
	config, err := settingsPath()
	if err != nil {
		return err
	}
	parent := filepath.Dir(config)
	if err = os.MkdirAll(parent, 0700); err != nil {
		return errors.New("Unable to create configuration folder")
	}
	data, _ := json.Marshal(settings{VaultPath: path})
	f, err := os.CreateTemp(parent, ".settings-*.tmp")
	if err != nil {
		return errors.New("Unable to save configuration")
	}
	defer os.Remove(f.Name())
	if err = f.Chmod(0600); err != nil {
		f.Close()
		return err
	}
	if _, err = f.Write(data); err != nil {
		f.Close()
		return err
	}
	if err = f.Sync(); err != nil {
		f.Close()
		return err
	}
	if err = f.Close(); err != nil {
		return err
	}
	return os.Rename(f.Name(), config)
}
func SelectVault(path string) (VaultInfo, error) {
	if err := allowed(); err != nil {
		return VaultInfo{}, err
	}
	if !filepath.IsAbs(path) || !IsInitialized(path) {
		return VaultInfo{}, errors.New("Choose an existing GNU pass vault with .gpg-id")
	}
	canonical, err := filepath.EvalSymlinks(path)
	if err != nil {
		return VaultInfo{}, err
	}
	if err = persist(canonical); err != nil {
		return VaultInfo{}, err
	}
	return VaultInfoCurrent()
}

func CloneVault(path, rawURL string) (VaultInfo, error) {
	if err := allowed(); err != nil {
		return VaultInfo{}, err
	}
	rawURL = strings.TrimSpace(rawURL)
	if rawURL == "" || strings.HasPrefix(rawURL, "-") || strings.ContainsAny(rawURL, "\x00\r\n") {
		return VaultInfo{}, errors.New("Invalid Git remote URL")
	}
	if err := validateNewPath(path); err != nil {
		return VaultInfo{}, err
	}
	if _, err := os.Lstat(path); err == nil {
		return VaultInfo{}, errors.New("Choose a directory that doesn't yet exist to clone the vault")
	}
	useGitHub := strings.HasPrefix(rawURL, "https://github.com/") && github.Status("").Login != nil
	args := []string{}
	if useGitHub {
		args = append(args, "-c", "credential.https://github.com.helper=", "-c", "credential.https://github.com.helper=!gh auth git-credential")
	}
	args = append(args, "clone", "--no-tags", "--single-branch", "--branch", "main", "--", rawURL, path)
	cmd := exec.Command("git", args...)
	cmd.Env = append(os.Environ(), "GIT_TERMINAL_PROMPT=0", "GIT_ASKPASS=/bin/false")
	if err := cmd.Run(); err != nil {
		return VaultInfo{}, errors.New("Unable to clone the vault. Check URL, access and branch main. If Git created the directory, review it before trying again")
	}
	canonical, err := filepath.EvalSymlinks(path)
	if err != nil {
		return VaultInfo{}, errors.New("Vault cloned but path unavailable")
	}
	if !IsInitialized(canonical) {
		return VaultInfo{}, errors.New("Cloned repository has no .gpg-id at its root. Review the directory; it was not selected")
	}
	if useGitHub {
		git, err := gitvault.Open(canonical)
		if err != nil {
			return VaultInfo{}, err
		}
		if err := git.ConfigureGitHubHelper(); err != nil {
			return VaultInfo{}, err
		}
	}
	if err := persist(canonical); err != nil {
		return VaultInfo{}, err
	}
	return VaultInfoCurrent()
}
func validateNewPath(path string) error {
	if !filepath.IsAbs(path) || filepath.Clean(path) == "/" {
		return errors.New("Choose a new directory with absolute path")
	}
	if info, err := os.Lstat(path); err == nil {
		if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
			return errors.New("Invalid link or file to create vault")
		}
		entries, err := os.ReadDir(path)
		if err != nil {
			return err
		}
		if len(entries) != 0 {
			return errors.New("The new vault directory must be empty")
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	} else {
		parent := filepath.Dir(path)
		info, err := os.Stat(parent)
		if err != nil || !info.IsDir() {
			return errors.New("The parent directory must exist")
		}
	}
	return nil
}
func CreateVault(path, keyID string) (VaultInfo, error) {
	if err := allowed(); err != nil {
		return VaultInfo{}, err
	}
	if !fingerprintPattern.MatchString(keyID) {
		return VaultInfo{}, errors.New("Select a valid GPG private key")
	}
	found := false
	for _, key := range GPGKeys() {
		if strings.EqualFold(key.ID, keyID) {
			found = true
			break
		}
	}
	if !found {
		return VaultInfo{}, errors.New("Select a private GPG key that can encrypt")
	}
	if err := validateNewPath(path); err != nil {
		return VaultInfo{}, err
	}
	cmd := exec.Command("pass", "init", keyID)
	cmd.Env = append(os.Environ(), "PASSWORD_STORE_DIR="+path, "GIT_TERMINAL_PROMPT=0")
	if err := cmd.Run(); err != nil {
		return VaultInfo{}, errors.New("Unable to create vault. Check GPG and permissions")
	}
	canonical, err := filepath.EvalSymlinks(path)
	if err != nil {
		return VaultInfo{}, err
	}
	if !IsInitialized(canonical) {
		return VaultInfo{}, errors.New("GNU pass did not create .gpg-id")
	}
	if err = persist(canonical); err != nil {
		return VaultInfo{}, err
	}
	return VaultInfoCurrent()
}
func GPGKeys() []GPGKey {
	out, err := exec.Command("gpg", "--batch", "--with-colons", "--fingerprint", "--list-secret-keys").Output()
	if err != nil {
		return []GPGKey{}
	}
	return parseGPGKeys(out)
}
func parseGPGKeys(out []byte) []GPGKey {
	keys := []GPGKey{}
	current := -1
	for _, line := range strings.Split(string(out), "\n") {
		f := strings.Split(line, ":")
		if len(f) == 0 {
			continue
		}
		switch f[0] {
		case "sec":
			current = -1
			if len(f) > 11 && !strings.ContainsAny(f[1], "redi") && strings.Contains(f[11], "E") && !strings.Contains(f[11], "D") {
				keys = append(keys, GPGKey{})
				current = len(keys) - 1
			}
		case "fpr":
			if current >= 0 && keys[current].ID == "" && len(f) > 9 {
				keys[current].ID = f[9]
			}
		case "uid":
			if current >= 0 && keys[current].Label == "" && len(f) > 9 {
				keys[current].Label = f[9]
			}
		}
	}
	filtered := []GPGKey{}
	for _, k := range keys {
		if fingerprintPattern.MatchString(k.ID) {
			if k.Label == "" {
				k.Label = k.ID
			}
			filtered = append(filtered, k)
		}
	}
	return filtered
}
func CreateKeyAndVault(path, name, email string) (CreatedKeyVault, error) {
	if err := allowed(); err != nil {
		return CreatedKeyVault{}, err
	}
	if err := validateNewPath(path); err != nil {
		return CreatedKeyVault{}, err
	}
	if !available("gpg") || !available("pass") {
		return CreatedKeyVault{}, errors.New("GNU pass and GPG must be installed")
	}
	uid, err := validatedUID(name, email)
	if err != nil {
		return CreatedKeyVault{}, err
	}
	return createKeyAndVaultWith(path, uid, generateKey)
}
func validatedUID(name, email string) (string, error) {
	name = strings.TrimSpace(name)
	email = strings.TrimSpace(email)
	if name == "" || len(name) > 100 || strings.ContainsAny(name, "<>()\r\n\x00") || len(email) > 254 || !strings.Contains(email, "@") || strings.ContainsAny(email, " \r\n\x00") {
		return "", errors.New("Invalid name or email for GPG key")
	}
	parts := strings.Split(email, "@")
	if len(parts) != 2 || parts[0] == "" || !strings.Contains(parts[1], ".") {
		return "", errors.New("Invalid email for GPG key")
	}
	for _, r := range email {
		if !(r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= '0' && r <= '9' || strings.ContainsRune("._+@-", r)) {
			return "", errors.New("Invalid email for GPG key")
		}
	}
	return name + " <" + email + ">", nil
}
func generateKey(uid string) (string, error) {
	out, err := exec.Command("gpg", "--batch", "--pinentry-mode", "ask", "--status-fd", "1", "--quick-generate-key", uid, "default", "default", "never").Output()
	fp := createdFingerprint(out)
	if err != nil {
		if fp != "" {
			return "", fmt.Errorf("GPG did not complete generation; the key %s may exist. Check pinentry", fp)
		}
		return "", errors.New("GPG did not create the key. Check pinentry")
	}
	if fp == "" {
		return "", errors.New("GPG finished without reporting a fingerprint; check your keys")
	}
	return fp, nil
}
func createdFingerprint(out []byte) string {
	for _, line := range bytes.Split(out, []byte{'\n'}) {
		const prefix = "[GNUPG:] KEY_CREATED "
		if bytes.HasPrefix(line, []byte(prefix)) {
			f := strings.Fields(string(line))
			if len(f) > 0 && fingerprintPattern.MatchString(f[len(f)-1]) {
				return f[len(f)-1]
			}
		}
	}
	return ""
}
func createKeyAndVaultWith(path, uid string, generate func(string) (string, error)) (CreatedKeyVault, error) {
	fp, err := generate(uid)
	if err != nil {
		return CreatedKeyVault{}, err
	}
	info, err := CreateVault(path, fp)
	if err != nil {
		return CreatedKeyVault{}, fmt.Errorf("GPG key %s created but vault not completed: %w. The key has been preserved", fp, err)
	}
	return CreatedKeyVault{Info: info, Fingerprint: fp}, nil
}
