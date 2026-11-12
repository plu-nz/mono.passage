package vault

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io/fs"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/plu-nz/passage/internal/setup"
)

type Entry struct {
	ID       string   `json:"id"`
	Title    string   `json:"title"`
	Username *string  `json:"username"`
	URLs     []string `json:"urls"`
}
type Secret struct {
	Password       string   `json:"password"`
	Details        string   `json:"details"`
	OtherDetails   string   `json:"other_details"`
	Username       *string  `json:"username"`
	URLs           []string `json:"urls"`
	CiphertextHash string   `json:"ciphertext_hash"`
}
type Vault struct{ root string }

func OpenEnv() (*Vault, error) {
	path, _, err := setup.ActiveVaultPath()
	if err != nil {
		return nil, err
	}
	return Open(path)
}
func Open(path string) (*Vault, error) {
	info, err := os.Stat(path)
	if err != nil || !info.IsDir() {
		return nil, fmt.Errorf("Cofre não encontrado em %s", path)
	}
	root, err := filepath.EvalSymlinks(path)
	if err != nil {
		return nil, err
	}
	root, err = filepath.Abs(root)
	if err != nil {
		return nil, err
	}
	return &Vault{root: root}, nil
}
func (v *Vault) Root() string { return v.root }
func ValidateID(id string) error {
	if id == "" || strings.HasSuffix(id, ".gpg") || strings.ContainsAny(id, "\\\x00") || filepath.IsAbs(id) {
		return errors.New("Identificador de entrada inválido")
	}
	for _, part := range strings.Split(id, "/") {
		if part == "" || part == "." || part == ".." || strings.HasPrefix(part, "-") {
			return errors.New("Identificador de entrada inválido")
		}
	}
	if strings.IndexFunc(id, unicode.IsControl) >= 0 {
		return errors.New("Identificador de entrada inválido")
	}
	return nil
}
func (v *Vault) safeTarget(id string) (string, error) {
	if err := ValidateID(id); err != nil {
		return "", err
	}
	current := v.root
	for _, part := range strings.Split(id+".gpg", "/") {
		current = filepath.Join(current, part)
		info, err := os.Lstat(current)
		if err == nil && info.Mode()&os.ModeSymlink != 0 {
			return "", errors.New("Links simbólicos não são permitidos no caminho da entrada")
		}
		if err != nil && !errors.Is(err, os.ErrNotExist) {
			return "", err
		}
	}
	return current, nil
}
func (v *Vault) existingFile(id string) (string, error) {
	target, err := v.safeTarget(id)
	if err != nil {
		return "", err
	}
	info, err := os.Stat(target)
	if err != nil || !info.Mode().IsRegular() {
		return "", errors.New("Entrada não encontrada")
	}
	canonical, err := filepath.EvalSymlinks(target)
	if err != nil {
		return "", err
	}
	if !strings.HasPrefix(canonical, v.root+string(os.PathSeparator)) {
		return "", errors.New("Entrada fora do cofre")
	}
	return canonical, nil
}
func (v *Vault) List() ([]Entry, error) {
	entries := []Entry{}
	err := filepath.WalkDir(v.root, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			if path != v.root && d.Name() == ".git" {
				return filepath.SkipDir
			}
			return nil
		}
		if d.Type()&os.ModeSymlink != 0 || !d.Type().IsRegular() || !strings.HasSuffix(d.Name(), ".gpg") {
			return nil
		}
		relative, err := filepath.Rel(v.root, path)
		if err != nil {
			return err
		}
		id := strings.TrimSuffix(filepath.ToSlash(relative), ".gpg")
		if ValidateID(id) != nil {
			return nil
		}
		entries = append(entries, Entry{ID: id, Title: strings.TrimSuffix(d.Name(), ".gpg"), URLs: []string{}})
		return nil
	})
	if err != nil {
		return nil, err
	}
	sort.Slice(entries, func(i, j int) bool { return strings.ToLower(entries[i].ID) < strings.ToLower(entries[j].ID) })
	return entries, nil
}
func hashFile(path string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	sum := sha256.Sum256(data)
	return hex.EncodeToString(sum[:]), nil
}
func (v *Vault) readContent(id string) (string, string, error) {
	file, err := v.existingFile(id)
	if err != nil {
		return "", "", err
	}
	before, err := hashFile(file)
	if err != nil {
		return "", "", err
	}
	cmd := exec.Command("pass", "show", id)
	cmd.Env = append(os.Environ(), "PASSWORD_STORE_DIR="+v.root)
	out, err := cmd.Output()
	if err != nil {
		return "", "", errors.New("Não foi possível abrir a entrada. Verifique a chave GPG e o gpg-agent")
	}
	after, err := hashFile(file)
	if err != nil {
		return "", "", err
	}
	if after != before {
		return "", "", errors.New("A entrada mudou durante a leitura. Tente novamente")
	}
	if !utf8Valid(out) {
		return "", "", errors.New("A entrada não contém texto UTF-8")
	}
	return string(out), before, nil
}
func utf8Valid(b []byte) bool { return utf8.Valid(b) }
func (v *Vault) Read(id string) (Secret, error) {
	content, hash, err := v.readContent(id)
	if err != nil {
		return Secret{}, err
	}
	secret := ParseSecret(content)
	secret.CiphertextHash = hash
	return secret, nil
}
func ParseSecret(content string) Secret {
	password, details, _ := strings.Cut(content, "\n")
	result := Secret{Password: password, Details: strings.TrimRightFunc(details, unicode.IsSpace), URLs: []string{}}
	var other strings.Builder
	loginSeen := false
	urlSeen := false
	for _, segment := range splitInclusive(details) {
		line := strings.TrimSuffix(segment, "\n")
		key, value, ok := strings.Cut(line, ":")
		isField := false
		if ok {
			switch strings.ToLower(strings.TrimSpace(key)) {
			case "login", "username":
				if !loginSeen {
					loginSeen = true
					value = strings.TrimSpace(value)
					if value != "" {
						result.Username = &value
					}
					isField = true
				}
			case "url":
				value = strings.TrimSpace(value)
				if value != "" {
					result.URLs = append(result.URLs, value)
				}
				if !urlSeen {
					urlSeen = true
					isField = true
				}
			}
		}
		if !isField {
			other.WriteString(segment)
		}
	}
	result.OtherDetails = other.String()
	return result
}
func splitInclusive(s string) []string {
	if s == "" {
		return nil
	}
	parts := strings.SplitAfter(s, "\n")
	if parts[len(parts)-1] == "" {
		parts = parts[:len(parts)-1]
	}
	return parts
}
func compose(password, username, siteURL, details string) (string, error) {
	if password == "" || len(password) > 16384 || strings.ContainsAny(password, "\r\n\x00") {
		return "", errors.New("Senha da entrada inválida")
	}
	if len(username) > 4096 || strings.ContainsAny(username, "\r\n\x00") {
		return "", errors.New("Login da entrada inválido")
	}
	if len(siteURL) > 4096 || strings.ContainsAny(siteURL, "\r\n\x00") {
		return "", errors.New("URL da entrada inválida")
	}
	if strings.TrimSpace(siteURL) != "" {
		parsed, err := url.Parse(strings.TrimSpace(siteURL))
		if err != nil || parsed.Hostname() == "" || (parsed.Scheme != "http" && parsed.Scheme != "https") {
			return "", errors.New("Use uma URL HTTP ou HTTPS válida")
		}
	}
	if len(details) > 1048576 || strings.ContainsRune(details, '\x00') {
		return "", errors.New("Outros campos da entrada são inválidos")
	}
	var out strings.Builder
	out.WriteString(password + "\n")
	if strings.TrimSpace(username) != "" {
		out.WriteString("login: " + strings.TrimSpace(username) + "\n")
	}
	if strings.TrimSpace(siteURL) != "" {
		out.WriteString("url: " + strings.TrimSpace(siteURL) + "\n")
	}
	if details != "" {
		out.WriteString(details)
		if !strings.HasSuffix(details, "\n") {
			out.WriteByte('\n')
		}
	}
	return out.String(), nil
}
func replaceKnown(original, password, username, siteURL string) (string, error) {
	if _, err := compose(password, username, siteURL, ""); err != nil {
		return "", err
	}
	_, details, _ := strings.Cut(original, "\n")
	var body strings.Builder
	loginSeen := false
	urlSeen := false
	for _, segment := range splitInclusive(details) {
		line := strings.TrimSuffix(segment, "\n")
		key, _, ok := strings.Cut(line, ":")
		switch {
		case ok && !loginSeen && (strings.EqualFold(strings.TrimSpace(key), "login") || strings.EqualFold(strings.TrimSpace(key), "username")):
			loginSeen = true
			if strings.TrimSpace(username) != "" {
				body.WriteString("login: " + strings.TrimSpace(username) + "\n")
			}
		case ok && !urlSeen && strings.EqualFold(strings.TrimSpace(key), "url"):
			urlSeen = true
			if strings.TrimSpace(siteURL) != "" {
				body.WriteString("url: " + strings.TrimSpace(siteURL) + "\n")
			}
		default:
			body.WriteString(segment)
		}
	}
	var out strings.Builder
	out.WriteString(password + "\n")
	if !loginSeen && strings.TrimSpace(username) != "" {
		out.WriteString("login: " + strings.TrimSpace(username) + "\n")
	}
	if !urlSeen && strings.TrimSpace(siteURL) != "" {
		out.WriteString("url: " + strings.TrimSpace(siteURL) + "\n")
	}
	out.WriteString(body.String())
	return out.String(), nil
}
func (v *Vault) insert(id, content string) error {
	if content == "" || strings.ContainsRune(content, '\x00') {
		return errors.New("Conteúdo de entrada inválido")
	}
	cmd := exec.Command("pass", "insert", "-m", "-f", id)
	cmd.Env = append(os.Environ(), "PASSWORD_STORE_DIR="+v.root)
	cmd.Stdin = strings.NewReader(content)
	cmd.Stdout = nil
	cmd.Stderr = nil
	if err := cmd.Run(); err != nil {
		return errors.New("Não foi possível salvar a entrada. Verifique a chave GPG")
	}
	return nil
}
func (v *Vault) Create(id, content string) error {
	target, err := v.safeTarget(id)
	if err != nil {
		return err
	}
	if _, err = os.Lstat(target); err == nil {
		return errors.New("A entrada já existe")
	}
	if !errors.Is(err, os.ErrNotExist) {
		return err
	}
	return v.insert(id, content)
}
func (v *Vault) CreateLogin(id, password, username, siteURL, details string) (Secret, error) {
	content, err := compose(password, username, siteURL, details)
	if err != nil {
		return Secret{}, err
	}
	if err = v.Create(id, content); err != nil {
		return Secret{}, err
	}
	secret, err := v.Read(id)
	if err != nil {
		return Secret{}, fmt.Errorf("Entrada criada, mas não foi possível reabri-la: %w", err)
	}
	return secret, nil
}
func (v *Vault) checkHash(id, expected string) error {
	file, err := v.existingFile(id)
	if err != nil {
		return err
	}
	if len(expected) != 64 {
		return errors.New("Versão da entrada inválida")
	}
	if _, err = hex.DecodeString(expected); err != nil {
		return errors.New("Versão da entrada inválida")
	}
	current, err := hashFile(file)
	if err != nil {
		return err
	}
	if current != expected {
		return errors.New("A entrada foi alterada. Atualize antes de salvar")
	}
	return nil
}
func (v *Vault) Update(id, content, expectedHash string) error {
	if err := v.checkHash(id, expectedHash); err != nil {
		return err
	}
	return v.insert(id, content)
}
func (v *Vault) UpdateLogin(id, password, username, siteURL, details, expectedHash string) (Secret, error) {
	original, current, err := v.readContent(id)
	if err != nil {
		return Secret{}, err
	}
	if current != expectedHash {
		return Secret{}, errors.New("A entrada foi alterada. Atualize antes de salvar")
	}
	var content string
	if details == ParseSecret(original).OtherDetails {
		content, err = replaceKnown(original, password, username, siteURL)
	} else {
		content, err = compose(password, username, siteURL, details)
	}
	if err != nil {
		return Secret{}, err
	}
	if err = v.Update(id, content, expectedHash); err != nil {
		return Secret{}, err
	}
	secret, err := v.Read(id)
	if err != nil {
		return Secret{}, fmt.Errorf("Entrada atualizada, mas não foi possível reabri-la: %w", err)
	}
	return secret, nil
}
func (v *Vault) Remove(id, expectedHash string) error {
	if err := v.checkHash(id, expectedHash); err != nil {
		return err
	}
	cmd := exec.Command("pass", "rm", "-f", id)
	cmd.Env = append(os.Environ(), "PASSWORD_STORE_DIR="+v.root)
	if err := cmd.Run(); err != nil {
		return errors.New("Operação do pass falhou")
	}
	return nil
}
