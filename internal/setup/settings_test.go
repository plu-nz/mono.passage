package setup

import (
	"os"
	"path/filepath"
	"testing"
)

func isolateSettings(t *testing.T) (config string, home string) {
	t.Helper()
	root := t.TempDir()
	home = filepath.Join(root, "home")
	t.Setenv("HOME", home)
	t.Setenv("XDG_CONFIG_HOME", filepath.Join(root, "config"))
	t.Setenv("PASSWORD_STORE_DIR", "")
	os.Unsetenv("PASSWORD_STORE_DIR")
	return filepath.Join(root, "config", "passage", "settings.json"), home
}

func TestRefreshSecondsDefaultsWithoutSettings(t *testing.T) {
	isolateSettings(t)
	got, err := RefreshSeconds()
	if err != nil || got != DefaultRefreshSeconds {
		t.Fatalf("RefreshSeconds() = %d, %v; want %d", got, err, DefaultRefreshSeconds)
	}
}

func TestRefreshSecondsWithoutVaultKeepsDefaultVault(t *testing.T) {
	_, home := isolateSettings(t)
	if err := SetRefreshSeconds(0); err != nil {
		t.Fatal(err)
	}
	if got, _ := RefreshSeconds(); got != 0 {
		t.Fatalf("RefreshSeconds() = %d; want 0", got)
	}
	path, source, err := ActiveVaultPath()
	if err != nil || source != "default" || path != filepath.Join(home, ".password-store") {
		t.Fatalf("ActiveVaultPath() = %q, %q, %v; want default vault", path, source, err)
	}
}

func TestSettingsKeepEachOther(t *testing.T) {
	isolateSettings(t)
	vault := t.TempDir()
	if err := persist(vault); err != nil {
		t.Fatal(err)
	}
	if err := SetRefreshSeconds(60); err != nil {
		t.Fatal(err)
	}
	if path, source, err := ActiveVaultPath(); err != nil || source != "selection" || path != vault {
		t.Fatalf("vault selection lost: %q, %q, %v", path, source, err)
	}
	if err := persist(vault); err != nil {
		t.Fatal(err)
	}
	if got, _ := RefreshSeconds(); got != 60 {
		t.Fatalf("refresh interval lost: %d", got)
	}
}

func TestSetRefreshSecondsRejectsUnsupportedValues(t *testing.T) {
	isolateSettings(t)
	for _, seconds := range []int{-1, 1, 7, 3600} {
		if err := SetRefreshSeconds(seconds); err == nil {
			t.Fatalf("SetRefreshSeconds(%d) accepted", seconds)
		}
	}
}

func TestInvalidSettingsFileIsReported(t *testing.T) {
	config, _ := isolateSettings(t)
	if err := os.MkdirAll(filepath.Dir(config), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(config, []byte("{"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, _, err := ActiveVaultPath(); err == nil {
		t.Fatal("ActiveVaultPath() accepted invalid settings")
	}
	if _, err := RefreshSeconds(); err == nil {
		t.Fatal("RefreshSeconds() accepted invalid settings")
	}
}

func TestNavPositionDefaultsToTopAndPersists(t *testing.T) {
	isolateSettings(t)
	if got, err := NavPosition(); err != nil || got != "top" {
		t.Fatalf("NavPosition() = %q, %v; want top", got, err)
	}
	if err := SetRefreshSeconds(60); err != nil {
		t.Fatal(err)
	}
	if err := SetNavPosition("bottom"); err != nil {
		t.Fatal(err)
	}
	if got, _ := NavPosition(); got != "bottom" {
		t.Fatalf("NavPosition() = %q; want bottom", got)
	}
	if got, _ := RefreshSeconds(); got != 60 {
		t.Fatalf("refresh interval lost: %d", got)
	}
	if err := SetNavPosition("left"); err == nil {
		t.Fatal("SetNavPosition accepted an unsupported position")
	}
}
