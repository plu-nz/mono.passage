package github

import (
	"encoding/json"
	"errors"
	"os"
	"os/exec"
	"strings"

	"github.com/plu-nz/mono.passage/internal/gitvault"
)

type StatusInfo struct {
	CLIInstalled       bool    `json:"cli_installed"`
	Login              *string `json:"login"`
	VaultGitConfigured bool    `json:"vault_git_configured"`
}

func Status(root string) StatusInfo {
	installed := exec.Command("gh", "--version").Run() == nil
	var login *string
	if installed {
		out, err := exec.Command("gh", "auth", "status", "--active", "--json", "hosts").Output()
		if err == nil {
			login = activeLogin(out)
		}
	}
	configured := root != "" && helperConfigured(root)
	return StatusInfo{CLIInstalled: installed, Login: login, VaultGitConfigured: configured}
}
func activeLogin(data []byte) *string {
	var payload struct {
		Hosts map[string][]struct {
			Login  string `json:"login"`
			Active bool   `json:"active"`
		} `json:"hosts"`
	}
	if json.Unmarshal(data, &payload) != nil {
		return nil
	}
	for _, account := range payload.Hosts["github.com"] {
		if account.Active && account.Login != "" {
			login := account.Login
			return &login
		}
	}
	return nil
}
func helperConfigured(root string) bool {
	if _, err := gitvault.Open(root); err != nil {
		return false
	}
	cmd := exec.Command("git", "config", "--local", "--get-all", "credential.https://github.com.helper")
	cmd.Dir = root
	out, err := cmd.Output()
	if err != nil {
		return false
	}
	for _, line := range strings.Split(string(out), "\n") {
		if line == "!gh auth git-credential" {
			return true
		}
	}
	return false
}
func StartLogin() error {
	if !Status("").CLIInstalled {
		return errors.New("Install GitHub CLI (gh) to connect your account")
	}
	cmd := exec.Command("xdg-terminal-exec", "--title=Passage - GitHub", "--", "gh", "auth", "login", "--hostname", "github.com", "--web", "--git-protocol", "https")
	cmd.Stdin = nil
	cmd.Stdout = nil
	cmd.Stderr = nil
	if err := cmd.Start(); err != nil {
		return errors.New("Unable to open terminal. Run gh auth login --web in terminal")
	}
	return cmd.Process.Release()
}
func ConfigureVault(root string) (StatusInfo, error) {
	if Status(root).Login == nil {
		return StatusInfo{}, errors.New("Connect your GitHub account before activating it in this vault")
	}
	g, err := gitvault.Open(root)
	if err != nil {
		return StatusInfo{}, err
	}
	if err = g.ConfigureGitHubHelper(); err != nil {
		return StatusInfo{}, err
	}
	return Status(root), nil
}
func CloneWithGitHubHelper(args ...string) ([]byte, error) {
	cmd := exec.Command("git", args...)
	cmd.Env = append(os.Environ(), "GIT_TERMINAL_PROMPT=0", "GIT_ASKPASS=/bin/false")
	return cmd.Output()
}
