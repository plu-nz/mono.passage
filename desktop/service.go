package main

import (
	"errors"
	"os/exec"
	"regexp"

	"github.com/plu-nz/mono.passage/internal/github"
	"github.com/plu-nz/mono.passage/internal/gitvault"
	"github.com/plu-nz/mono.passage/internal/otp"
	"github.com/plu-nz/mono.passage/internal/pairing"
	"github.com/plu-nz/mono.passage/internal/setup"
	"github.com/plu-nz/mono.passage/internal/vault"
)

// DesktopService exposes a fixed set of Passage actions to the embedded UI.
type DesktopService struct{}

type VaultState struct {
	Kind   string `json:"kind"`
	Reason string `json:"reason,omitempty"`
}

func (*DesktopService) VaultState() VaultState {
	if err := exec.Command("pass", "--version").Run(); err != nil {
		return VaultState{Kind: "unavailable", Reason: "GNU pass is not installed or is not in the PATH."}
	}
	v, err := vault.OpenEnv()
	if err != nil {
		return VaultState{Kind: "unavailable", Reason: err.Error()}
	}
	if !setup.IsInitialized(v.Root()) {
		return VaultState{Kind: "unavailable", Reason: "The directory is not yet initialized with GNU pass (.gpg-id missing)."}
	}
	return VaultState{Kind: "available"}
}

func (*DesktopService) VaultInfo() (setup.VaultInfo, error) { return setup.VaultInfoCurrent() }
func (*DesktopService) GPGKeys() []setup.GPGKey             { return setup.GPGKeys() }
func (*DesktopService) SelectVault(path string) (setup.VaultInfo, error) {
	return setup.SelectVault(path)
}
func (*DesktopService) CreateVault(path, keyID string) (setup.VaultInfo, error) {
	return setup.CreateVault(path, keyID)
}
func (*DesktopService) CreateKeyAndVault(path, name, email string) (setup.CreatedKeyVault, error) {
	return setup.CreateKeyAndVault(path, name, email)
}
func (*DesktopService) ListEntries() ([]vault.Entry, error) {
	v, err := vault.OpenEnv()
	if err != nil {
		return nil, err
	}
	return v.List()
}
func (*DesktopService) ReadEntry(id string) (vault.Secret, error) {
	v, err := vault.OpenEnv()
	if err != nil {
		return vault.Secret{}, err
	}
	return v.Read(id)
}
func (*DesktopService) CreateLoginEntry(id, password, username, url, details string) (vault.Secret, error) {
	v, err := vault.OpenEnv()
	if err != nil {
		return vault.Secret{}, err
	}
	return v.CreateLogin(id, password, username, url, details)
}
func (*DesktopService) UpdateLoginEntry(id, password, username, url, details, expectedHash string) (vault.Secret, error) {
	v, err := vault.OpenEnv()
	if err != nil {
		return vault.Secret{}, err
	}
	return v.UpdateLogin(id, password, username, url, details, expectedHash)
}
func (*DesktopService) CreateEntry(id, content string) error {
	v, err := vault.OpenEnv()
	if err != nil {
		return err
	}
	return v.Create(id, content)
}
func (*DesktopService) UpdateEntry(id, content, expectedHash string) error {
	v, err := vault.OpenEnv()
	if err != nil {
		return err
	}
	return v.Update(id, content, expectedHash)
}
func (*DesktopService) RemoveEntry(id, expectedHash string) error {
	v, err := vault.OpenEnv()
	if err != nil {
		return err
	}
	return v.Remove(id, expectedHash)
}
func (*DesktopService) GenerateTOTP(id string) (otp.Code, error) {
	v, err := vault.OpenEnv()
	if err != nil {
		return otp.Code{}, err
	}
	secret, err := v.Read(id)
	if err != nil {
		return otp.Code{}, err
	}
	return otp.Generate(secret)
}
func (*DesktopService) CloneVault(path, url string) (setup.VaultInfo, error) {
	return setup.CloneVault(path, url)
}
func (*DesktopService) GitInitialize() (gitvault.Status, error) {
	v, err := vault.OpenEnv()
	if err != nil {
		return gitvault.Status{}, err
	}
	return gitvault.Initialize(v.Root())
}
func (*DesktopService) GitStatus() (gitvault.Status, error) {
	g, err := openGit()
	if err != nil {
		return gitvault.Status{}, err
	}
	return g.Status()
}
func (*DesktopService) GitSetRemote(url string) (gitvault.Status, error) {
	g, err := openGit()
	if err != nil {
		return gitvault.Status{}, err
	}
	return g.SetRemote(url)
}
func (*DesktopService) GitPublishNewVault() (gitvault.Status, error) {
	g, err := openGit()
	if err != nil {
		return gitvault.Status{}, err
	}
	return g.PublishNewVault()
}
func (*DesktopService) GitCommitChanges() (gitvault.Status, error) {
	g, err := openGit()
	if err != nil {
		return gitvault.Status{}, err
	}
	return g.CommitChanges()
}
func (*DesktopService) GitMergePreservingBoth() (gitvault.Status, error) {
	g, err := openGit()
	if err != nil {
		return gitvault.Status{}, err
	}
	return g.MergePreservingBoth()
}
func (*DesktopService) GitPull() (gitvault.Status, error) {
	g, err := openGit()
	if err != nil {
		return gitvault.Status{}, err
	}
	if err := g.Pull(); err != nil {
		return gitvault.Status{}, err
	}
	return g.Status()
}
func (*DesktopService) GitPush() (gitvault.Status, error) {
	g, err := openGit()
	if err != nil {
		return gitvault.Status{}, err
	}
	if err := g.Push(); err != nil {
		return gitvault.Status{}, err
	}
	return g.Status()
}
func openGit() (*gitvault.GitVault, error) {
	v, err := vault.OpenEnv()
	if err != nil {
		return nil, err
	}
	return gitvault.Open(v.Root())
}

// RefreshSeconds is the entry list reload interval; 0 means off.
func (*DesktopService) RefreshSeconds() (int, error) { return setup.RefreshSeconds() }
func (*DesktopService) SetRefreshSeconds(seconds int) error {
	return setup.SetRefreshSeconds(seconds)
}

func (*DesktopService) GithubStatus() github.StatusInfo {
	v, _ := vault.OpenEnv()
	if v == nil {
		return github.Status("")
	}
	return github.Status(v.Root())
}
func (*DesktopService) GithubLogin() error { return github.StartLogin() }
func (*DesktopService) GithubConfigureVault() (github.StatusInfo, error) {
	v, err := vault.OpenEnv()
	if err != nil {
		return github.StatusInfo{}, err
	}
	return github.ConfigureVault(v.Root())
}

var pairingCode = regexp.MustCompile(`^[a-fA-F0-9]{12}$`)
var extensionOrigin = regexp.MustCompile(`^chrome-extension://[a-p]{32}/$`)

func (*DesktopService) ListPairings() ([]pairing.Pairing, error) {
	v, err := vault.OpenEnv()
	if err != nil {
		return nil, err
	}
	s, err := pairing.OpenDefault()
	if err != nil {
		return nil, err
	}
	return s.ListCurrent(v.Root())
}
func (*DesktopService) ApprovePairing(code string) error {
	if !pairingCode.MatchString(code) {
		return errors.New("Invalid pairing code")
	}
	s, err := pairing.OpenDefault()
	if err != nil {
		return err
	}
	_, err = s.Approve(code)
	return err
}
func (*DesktopService) RevokePairing(origin, installationID string) error {
	if !extensionOrigin.MatchString(origin) || installationID == "" || len(installationID) > 128 {
		return errors.New("Invalid pairing")
	}
	s, err := pairing.OpenDefault()
	if err != nil {
		return err
	}
	_, err = s.Revoke(origin, installationID)
	return err
}
func (*DesktopService) BrowserAccessState() (bool, error) {
	s, err := pairing.OpenDefault()
	if err != nil {
		return false, err
	}
	return s.Locked()
}
func (*DesktopService) LockBrowserAccess() error {
	s, err := pairing.OpenDefault()
	if err != nil {
		return err
	}
	return s.SetLocked(true)
}
func (*DesktopService) UnlockBrowserAccess() error {
	s, err := pairing.OpenDefault()
	if err != nil {
		return err
	}
	return s.SetLocked(false)
}
