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

	"github.com/plu-nz/passage/internal/github"
	"github.com/plu-nz/passage/internal/gitvault"
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
			return "", errors.New("HOME não está definido")
		}
		base = filepath.Join(home, ".config")
	}
	if !filepath.IsAbs(base) {
		return "", errors.New("XDG_CONFIG_HOME precisa ser absoluto")
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
			return "", "", errors.New("Configuração do cofre inválida. Corrija settings.json")
		}
		return s.VaultPath, "selection", nil
	}
	if !errors.Is(err, os.ErrNotExist) {
		return "", "", errors.New("Não foi possível ler as configurações do Passage")
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return "", "", errors.New("HOME não está definido")
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
		return errors.New("PASSWORD_STORE_DIR está definido e prevalece sobre a seleção no Passage")
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
		return errors.New("Não foi possível criar a pasta de configuração")
	}
	data, _ := json.Marshal(settings{VaultPath: path})
	f, err := os.CreateTemp(parent, ".settings-*.tmp")
	if err != nil {
		return errors.New("Não foi possível salvar a configuração")
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
		return VaultInfo{}, errors.New("Escolha um cofre GNU pass existente com .gpg-id")
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
		return VaultInfo{}, errors.New("URL do remoto Git inválida")
	}
	if err := validateNewPath(path); err != nil {
		return VaultInfo{}, err
	}
	if _, err := os.Lstat(path); err == nil {
		return VaultInfo{}, errors.New("Escolha um diretório que ainda não existe para clonar o cofre")
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
		return VaultInfo{}, errors.New("Não foi possível clonar o cofre. Verifique URL, acesso e branch main. Se Git criou o diretório, revise-o antes de tentar novamente")
	}
	canonical, err := filepath.EvalSymlinks(path)
	if err != nil {
		return VaultInfo{}, errors.New("Cofre clonado, mas caminho indisponível")
	}
	if !IsInitialized(canonical) {
		return VaultInfo{}, errors.New("Repositório clonado sem .gpg-id na raiz. Revise o diretório; ele não foi selecionado")
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
		return errors.New("Escolha um diretório novo com caminho absoluto")
	}
	if info, err := os.Lstat(path); err == nil {
		if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
			return errors.New("Link ou arquivo inválido para criar cofre")
		}
		entries, err := os.ReadDir(path)
		if err != nil {
			return err
		}
		if len(entries) != 0 {
			return errors.New("O diretório do novo cofre precisa estar vazio")
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	} else {
		parent := filepath.Dir(path)
		info, err := os.Stat(parent)
		if err != nil || !info.IsDir() {
			return errors.New("O diretório pai precisa existir")
		}
	}
	return nil
}
func CreateVault(path, keyID string) (VaultInfo, error) {
	if err := allowed(); err != nil {
		return VaultInfo{}, err
	}
	if !fingerprintPattern.MatchString(keyID) {
		return VaultInfo{}, errors.New("Selecione uma chave GPG privada válida")
	}
	found := false
	for _, key := range GPGKeys() {
		if strings.EqualFold(key.ID, keyID) {
			found = true
			break
		}
	}
	if !found {
		return VaultInfo{}, errors.New("Selecione uma chave GPG privada capaz de cifrar")
	}
	if err := validateNewPath(path); err != nil {
		return VaultInfo{}, err
	}
	cmd := exec.Command("pass", "init", keyID)
	cmd.Env = append(os.Environ(), "PASSWORD_STORE_DIR="+path, "GIT_TERMINAL_PROMPT=0")
	if err := cmd.Run(); err != nil {
		return VaultInfo{}, errors.New("Não foi possível criar o cofre. Verifique GPG e permissões")
	}
	canonical, err := filepath.EvalSymlinks(path)
	if err != nil {
		return VaultInfo{}, err
	}
	if !IsInitialized(canonical) {
		return VaultInfo{}, errors.New("GNU pass não criou .gpg-id")
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
		return CreatedKeyVault{}, errors.New("GNU pass e GPG precisam estar instalados")
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
		return "", errors.New("Nome ou e-mail inválido para a chave GPG")
	}
	parts := strings.Split(email, "@")
	if len(parts) != 2 || parts[0] == "" || !strings.Contains(parts[1], ".") {
		return "", errors.New("E-mail inválido para a chave GPG")
	}
	for _, r := range email {
		if !(r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= '0' && r <= '9' || strings.ContainsRune("._+@-", r)) {
			return "", errors.New("E-mail inválido para a chave GPG")
		}
	}
	return name + " <" + email + ">", nil
}
func generateKey(uid string) (string, error) {
	out, err := exec.Command("gpg", "--batch", "--pinentry-mode", "ask", "--status-fd", "1", "--quick-generate-key", uid, "default", "default", "never").Output()
	fp := createdFingerprint(out)
	if err != nil {
		if fp != "" {
			return "", fmt.Errorf("GPG não concluiu geração; a chave %s pode existir. Verifique o pinentry", fp)
		}
		return "", errors.New("GPG não criou a chave. Verifique o pinentry")
	}
	if fp == "" {
		return "", errors.New("GPG concluiu sem informar fingerprint; confira suas chaves")
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
		return CreatedKeyVault{}, fmt.Errorf("Chave GPG %s criada, mas cofre não concluído: %w. A chave foi preservada", fp, err)
	}
	return CreatedKeyVault{Info: info, Fingerprint: fp}, nil
}
