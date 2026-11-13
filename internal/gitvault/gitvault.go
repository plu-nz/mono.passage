package gitvault

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
)

type Status struct {
	Branch   string  `json:"branch"`
	Upstream *string `json:"upstream"`
	Dirty    bool    `json:"dirty"`
	Ahead    uint64  `json:"ahead"`
	Behind   uint64  `json:"behind"`
}
type GitVault struct{ root string }

func Open(root string) (*GitVault, error) {
	canonical, err := filepath.EvalSymlinks(root)
	if err != nil {
		return nil, errors.New("Vault not found")
	}
	canonical, err = filepath.Abs(canonical)
	if err != nil {
		return nil, err
	}
	g := &GitVault{root: canonical}
	top, err := g.output("rev-parse", "--show-toplevel")
	if err != nil {
		return nil, err
	}
	top, err = filepath.EvalSymlinks(strings.TrimSpace(top))
	if err != nil {
		return nil, errors.New("Invalid Git repository")
	}
	if top != canonical {
		return nil, errors.New("The vault must be the root of your own Git repository")
	}
	return g, nil
}
func Initialize(root string) (Status, error) {
	canonical, err := filepath.EvalSymlinks(root)
	if err != nil {
		return Status{}, errors.New("Vault not found")
	}
	if !isRegular(filepath.Join(canonical, ".gpg-id")) {
		return Status{}, errors.New("Initialize the vault with GNU pass before Git")
	}
	if _, err = os.Stat(filepath.Join(canonical, ".git")); err == nil {
		return Status{}, errors.New("This vault already contains a Git repository")
	}
	if _, err = Open(canonical); err == nil {
		return Status{}, errors.New("This vault already belongs to a Git repository")
	}
	cmd := exec.Command("git", "init", "-b", "main")
	cmd.Dir = canonical
	if err = cmd.Run(); err != nil {
		return Status{}, errors.New("Unable to start Git repository")
	}
	g, err := Open(canonical)
	if err != nil {
		return Status{}, err
	}
	return g.Status()
}
func isRegular(path string) bool {
	info, err := os.Lstat(path)
	return err == nil && info.Mode().IsRegular()
}
func (g *GitVault) Root() string { return g.root }
func (g *GitVault) git(args ...string) ([]byte, error) {
	cmd := exec.Command("git", args...)
	cmd.Dir = g.root
	cmd.Env = append(os.Environ(), "GIT_TERMINAL_PROMPT=0", "GIT_ASKPASS=/bin/false")
	return cmd.Output()
}
func (g *GitVault) output(args ...string) (string, error) {
	out, err := g.git(args...)
	if err != nil {
		return "", fmt.Errorf("Git failed %s. Check repository, remote and authentication", args[0])
	}
	return string(out), nil
}
func (g *GitVault) tryOutput(args ...string) (string, bool) {
	out, err := g.git(args...)
	return string(out), err == nil
}
func (g *GitVault) Status() (Status, error) {
	branch, err := g.output("symbolic-ref", "--quiet", "--short", "HEAD")
	if err != nil {
		return Status{}, err
	}
	branch = strings.TrimSpace(branch)
	if branch == "" {
		return Status{}, errors.New("Git has no active branch")
	}
	dirtyOut, err := g.output("status", "--porcelain=v1", "--untracked-files=all")
	if err != nil {
		return Status{}, err
	}
	s := Status{Branch: branch, Dirty: dirtyOut != ""}
	upstream, ok := g.tryOutput("rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}")
	if ok {
		upstream = strings.TrimSpace(upstream)
		s.Upstream = &upstream
		counts, err := g.output("rev-list", "--left-right", "--count", "HEAD...@{upstream}")
		if err != nil {
			return Status{}, err
		}
		parts := strings.Fields(counts)
		if len(parts) != 2 {
			return Status{}, errors.New("Invalid Git status")
		}
		s.Ahead, err = strconv.ParseUint(parts[0], 10, 64)
		if err != nil {
			return Status{}, errors.New("Invalid Git status")
		}
		s.Behind, err = strconv.ParseUint(parts[1], 10, 64)
		if err != nil {
			return Status{}, errors.New("Invalid Git status")
		}
	}
	return s, nil
}
func (g *GitVault) SetRemote(rawURL string) (Status, error) {
	rawURL = strings.TrimSpace(rawURL)
	if rawURL == "" || strings.HasPrefix(rawURL, "-") || strings.ContainsAny(rawURL, "\x00\r\n") {
		return Status{}, errors.New("Invalid Git remote URL")
	}
	if _, ok := g.tryOutput("remote", "get-url", "origin"); ok {
		return Status{}, errors.New("The origin remote already exists. Change it via Git to avoid accidental overwriting")
	}
	if _, err := g.output("remote", "add", "origin", rawURL); err != nil {
		return Status{}, err
	}
	return g.Status()
}
func (g *GitVault) ConfigureGitHubHelper() error {
	if _, err := g.output("config", "--local", "--replace-all", "credential.https://github.com.helper", ""); err != nil {
		return err
	}
	_, err := g.output("config", "--local", "--add", "credential.https://github.com.helper", "!gh auth git-credential")
	return err
}
func (g *GitVault) requireCleanUpstream() (Status, error) {
	s, err := g.Status()
	if err != nil {
		return Status{}, err
	}
	if s.Dirty {
		return Status{}, errors.New("The vault has uncommitted Git changes. Resolve or commit them before syncing")
	}
	if s.Upstream == nil {
		return Status{}, errors.New("Configure an upstream branch before syncing")
	}
	return s, nil
}
func (g *GitVault) remote(branch string) (string, error) {
	remote, err := g.output("config", "--get", "branch."+branch+".remote")
	remote = strings.TrimSpace(remote)
	if err != nil || remote == "" || remote == "." {
		return "", errors.New("Invalid Git remote")
	}
	return remote, nil
}
func (g *GitVault) Pull() error {
	before, err := g.requireCleanUpstream()
	if err != nil {
		return err
	}
	remote, err := g.remote(before.Branch)
	if err != nil {
		return err
	}
	if _, err = g.output("fetch", "--no-tags", remote); err != nil {
		return err
	}
	after, err := g.Status()
	if err != nil {
		return err
	}
	if after.Behind == 0 {
		return nil
	}
	if after.Ahead != 0 {
		return errors.New("Local and remote histories diverged. Resolve conflict manually in Git")
	}
	_, err = g.output("merge", "--ff-only", "@{upstream}")
	return err
}
func (g *GitVault) Push() error {
	before, err := g.requireCleanUpstream()
	if err != nil {
		return err
	}
	remote, err := g.remote(before.Branch)
	if err != nil {
		return err
	}
	if _, err = g.output("fetch", "--no-tags", remote); err != nil {
		return err
	}
	after, err := g.Status()
	if err != nil {
		return err
	}
	if after.Behind != 0 {
		return errors.New("The remote has new commits. Pull before pushing; no version was overwritten")
	}
	if after.Ahead == 0 {
		return nil
	}
	mergeRef, err := g.output("config", "--get", "branch."+after.Branch+".merge")
	if err != nil {
		return err
	}
	mergeRef = strings.TrimSpace(mergeRef)
	if !strings.HasPrefix(mergeRef, "refs/heads/") {
		return errors.New("Invalid remote branch")
	}
	_, err = g.output("push", remote, "HEAD:"+mergeRef)
	return err
}
func (g *GitVault) ValidateCiphertext(data []byte) error {
	cmd := exec.Command("gpg", "--batch", "--no-tty", "--list-packets")
	cmd.Stdin = bytes.NewReader(data)
	out, err := cmd.Output()
	if err != nil && len(out) == 0 {
		return errors.New("A .gpg entry does not contain encrypted OpenPGP data")
	}
	packets := string(out)
	if !(strings.Contains(packets, ":pubkey enc packet:") || strings.Contains(packets, ":symkey enc packet:")) || !(strings.Contains(packets, ":encrypted data packet:") || strings.Contains(packets, ":aead encrypted packet:")) {
		return errors.New("A .gpg entry does not contain encrypted OpenPGP data")
	}
	return nil
}
