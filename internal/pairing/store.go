package pairing

import (
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"syscall"
	"time"
)

const pairTTL = 5 * time.Minute

// Pairing is an extension installation authorized for one canonical vault path.
type Pairing struct {
	Origin         string `json:"origin"`
	InstallationID string `json:"installationId"`
	VaultPath      string `json:"vaultPath"`
}

type pending struct {
	Origin         string  `json:"origin"`
	InstallationID string  `json:"installation_id"`
	Code           string  `json:"code"`
	ExpiresAt      int64   `json:"expires_at"`
	Approved       bool    `json:"approved"`
	VaultPath      *string `json:"vault_path"`
}

type authorized struct {
	Origin         string  `json:"origin"`
	InstallationID string  `json:"installation_id"`
	TokenHash      string  `json:"token_hash"`
	VaultPath      *string `json:"vault_path"`
}

type state struct {
	Pending    []pending    `json:"pending"`
	Authorized []authorized `json:"authorized"`
	Locked     bool         `json:"locked"`
}

type Store struct{ dir string }

func OpenDefault() (*Store, error) {
	base := os.Getenv("XDG_CONFIG_HOME")
	if base == "" {
		home := os.Getenv("HOME")
		if home == "" {
			return nil, errors.New("HOME indisponível")
		}
		base = filepath.Join(home, ".config")
	}
	if !filepath.IsAbs(base) {
		return nil, errors.New("XDG_CONFIG_HOME precisa ser absoluto")
	}
	return OpenAt(filepath.Join(base, "passage"))
}

// OpenAt is useful for isolated tests; its argument is the Passage config directory.
func OpenAt(dir string) (*Store, error) {
	if !filepath.IsAbs(dir) {
		return nil, errors.New("diretório de configuração precisa ser absoluto")
	}
	if info, err := os.Lstat(dir); err == nil && !info.IsDir() {
		return nil, errors.New("diretório de configuração inválido")
	} else if err != nil && !errors.Is(err, os.ErrNotExist) {
		return nil, err
	}
	if err := os.MkdirAll(dir, 0700); err != nil {
		return nil, err
	}
	if err := os.Chmod(dir, 0700); err != nil {
		return nil, err
	}
	return &Store{dir: dir}, nil
}

func randomHex(n int) (string, error) {
	buf := make([]byte, n)
	if _, err := rand.Read(buf); err != nil {
		return "", err
	}
	return hex.EncodeToString(buf), nil
}

func tokenHash(token string) [32]byte { return sha256.Sum256([]byte(token)) }

func (s *Store) withState(fn func(*state) error) error {
	lockPath := filepath.Join(s.dir, "native-host.lock")
	if info, err := os.Lstat(lockPath); err == nil && info.Mode()&os.ModeSymlink != 0 {
		return errors.New("arquivo de lock inválido")
	} else if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	lock, err := os.OpenFile(lockPath, os.O_CREATE|os.O_RDWR, 0600)
	if err != nil {
		return err
	}
	defer lock.Close()
	if err := syscall.Flock(int(lock.Fd()), syscall.LOCK_EX); err != nil {
		return err
	}
	defer syscall.Flock(int(lock.Fd()), syscall.LOCK_UN)

	path := filepath.Join(s.dir, "native-host.json")
	if info, err := os.Lstat(path); err == nil && info.Mode()&os.ModeSymlink != 0 {
		return errors.New("estado local inválido")
	} else if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	current := state{Pending: []pending{}, Authorized: []authorized{}}
	data, err := os.ReadFile(path)
	if err == nil {
		if len(data) > 1024*1024 || json.Unmarshal(data, &current) != nil {
			return errors.New("estado local inválido")
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	}
	normalize(&current)
	before, err := json.Marshal(current)
	if err != nil {
		return err
	}
	valid := current.Pending[:0]
	for _, item := range current.Pending {
		if item.ExpiresAt > time.Now().Unix() {
			valid = append(valid, item)
		}
	}
	current.Pending = valid
	result := fn(&current)
	normalize(&current)
	after, err := json.Marshal(current)
	if err != nil {
		return err
	}
	if string(after) == string(before) {
		return result
	}
	output, err := os.CreateTemp(s.dir, "native-host-*.tmp")
	if err != nil {
		return err
	}
	defer os.Remove(output.Name())
	if err := output.Chmod(0600); err != nil {
		output.Close()
		return err
	}
	if _, err := output.Write(after); err != nil {
		output.Close()
		return err
	}
	if err := output.Sync(); err != nil {
		output.Close()
		return err
	}
	if err := output.Close(); err != nil {
		return err
	}
	if err := os.Rename(output.Name(), path); err != nil {
		return err
	}
	if dir, err := os.Open(s.dir); err == nil {
		_ = dir.Sync()
		_ = dir.Close()
	}
	return result
}

func normalize(current *state) {
	if current.Pending == nil {
		current.Pending = []pending{}
	}
	if current.Authorized == nil {
		current.Authorized = []authorized{}
	}
}

func (s *Store) Pair(origin, installationID, vaultPath string) (string, int64, error) {
	if origin == "" || installationID == "" || vaultPath == "" {
		return "", 0, errors.New("pareamento inválido")
	}
	var code string
	var expires int64
	err := s.withState(func(current *state) error {
		var err error
		code, err = randomHex(6)
		if err != nil {
			return err
		}
		expires = time.Now().Add(pairTTL).Unix()
		items := current.Pending[:0]
		for _, item := range current.Pending {
			if item.Origin != origin || item.InstallationID != installationID {
				items = append(items, item)
			}
		}
		current.Pending = append(items, pending{Origin: origin, InstallationID: installationID, Code: code, ExpiresAt: expires, VaultPath: &vaultPath})
		return nil
	})
	return code, expires, err
}

func (s *Store) Approve(code string) (Pairing, error) {
	var approved Pairing
	err := s.withState(func(current *state) error {
		for i := range current.Pending {
			item := &current.Pending[i]
			if item.Code == code && item.ExpiresAt > time.Now().Unix() && item.VaultPath != nil {
				item.Approved = true
				approved = Pairing{Origin: item.Origin, InstallationID: item.InstallationID, VaultPath: *item.VaultPath}
				return nil
			}
		}
		return errors.New("código inválido ou expirado")
	})
	return approved, err
}

func (s *Store) Poll(origin, installationID, vaultPath string) (string, error) {
	var token string
	err := s.withState(func(current *state) error {
		for i, item := range current.Pending {
			if item.Origin != origin || item.InstallationID != installationID || !item.Approved || item.VaultPath == nil || *item.VaultPath != vaultPath {
				continue
			}
			var err error
			token, err = randomHex(32)
			if err != nil {
				return err
			}
			current.Pending = append(current.Pending[:i], current.Pending[i+1:]...)
			items := current.Authorized[:0]
			for _, allowed := range current.Authorized {
				if allowed.Origin != origin || allowed.InstallationID != installationID {
					items = append(items, allowed)
				}
			}
			current.Authorized = append(items, authorized{Origin: origin, InstallationID: installationID, TokenHash: fmt.Sprintf("%x", tokenHash(token)), VaultPath: &vaultPath})
			return nil
		}
		return nil
	})
	return token, err
}

func (s *Store) Status(origin, installationID, vaultPath string) (paired, locked bool, err error) {
	err = s.withState(func(current *state) error {
		locked = current.Locked
		for _, item := range current.Authorized {
			if item.Origin == origin && item.InstallationID == installationID && item.VaultPath != nil && *item.VaultPath == vaultPath {
				paired = true
				break
			}
		}
		return nil
	})
	return
}

func (s *Store) Authorized(origin, installationID, token, vaultPath string) (bool, error) {
	valid := false
	err := s.withState(func(current *state) error {
		if current.Locked {
			return nil
		}
		digest := tokenHash(token)
		for _, item := range current.Authorized {
			if item.Origin != origin || item.InstallationID != installationID || item.VaultPath == nil || *item.VaultPath != vaultPath {
				continue
			}
			stored, err := hex.DecodeString(item.TokenHash)
			if err == nil && len(stored) == len(digest) && subtle.ConstantTimeCompare(stored, digest[:]) == 1 {
				valid = true
				break
			}
		}
		return nil
	})
	return valid, err
}

func (s *Store) ListCurrent(vaultPath string) ([]Pairing, error) {
	items := []Pairing{}
	err := s.withState(func(current *state) error {
		for _, item := range current.Authorized {
			if item.VaultPath != nil && *item.VaultPath == vaultPath {
				items = append(items, Pairing{Origin: item.Origin, InstallationID: item.InstallationID, VaultPath: *item.VaultPath})
			}
		}
		return nil
	})
	return items, err
}

func (s *Store) ListAll() ([]Pairing, error) {
	items := []Pairing{}
	err := s.withState(func(current *state) error {
		for _, item := range current.Authorized {
			path := ""
			if item.VaultPath != nil {
				path = *item.VaultPath
			}
			items = append(items, Pairing{Origin: item.Origin, InstallationID: item.InstallationID, VaultPath: path})
		}
		return nil
	})
	return items, err
}

func (s *Store) Revoke(origin, installationID string) (bool, error) {
	removed := false
	err := s.withState(func(current *state) error {
		items := current.Authorized[:0]
		for _, item := range current.Authorized {
			if item.Origin == origin && item.InstallationID == installationID {
				removed = true
			} else {
				items = append(items, item)
			}
		}
		current.Authorized = items
		return nil
	})
	return removed, err
}

func (s *Store) Locked() (bool, error) {
	locked := false
	err := s.withState(func(current *state) error { locked = current.Locked; return nil })
	return locked, err
}

func (s *Store) SetLocked(locked bool) error {
	return s.withState(func(current *state) error { current.Locked = locked; return nil })
}
