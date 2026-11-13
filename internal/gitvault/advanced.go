package gitvault

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

func (g *GitVault) collectVaultFiles() ([]string, error) {
	files := []string{".gpg-id"}
	err := filepath.WalkDir(g.root, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if path == g.root {
			return nil
		}
		relative, e := filepath.Rel(g.root, path)
		if e != nil {
			return e
		}
		relative = filepath.ToSlash(relative)
		if relative == ".git" && d.IsDir() {
			return filepath.SkipDir
		}
		if d.IsDir() {
			return nil
		}
		if !d.Type().IsRegular() {
			return errors.New("O cofre contém links ou arquivos especiais. Revise antes de publicar")
		}
		if relative == ".gpg-id" {
			return nil
		}
		if strings.HasSuffix(relative, ".gpg") || d.Name() == ".gpg-id" {
			files = append(files, relative)
			return nil
		}
		return errors.New("O cofre contém arquivos além de .gpg e .gpg-id. Revise antes de publicar")
	})
	sort.Strings(files)
	return files, err
}
func (g *GitVault) verifyStaged(paths []string) error {
	for _, path := range paths {
		path = strings.TrimPrefix(path, ":(literal)")
		if !strings.HasSuffix(path, ".gpg") {
			continue
		}
		out, err := g.git("show", ":0:"+path)
		if err != nil {
			if _, err = os.Stat(filepath.Join(g.root, path)); errors.Is(err, os.ErrNotExist) {
				continue
			}
			return errors.New("Não foi possível validar a entrada preparada no Git")
		}
		if err = g.ValidateCiphertext(out); err != nil {
			return err
		}
	}
	return nil
}
func validRelative(path string) bool {
	if path == "" || filepath.IsAbs(path) || strings.Contains(path, "\\") {
		return false
	}
	for _, part := range strings.Split(path, "/") {
		if part == "" || part == "." || part == ".." {
			return false
		}
	}
	return true
}
func (g *GitVault) rejectSymlinks(relative string) error {
	if !validRelative(relative) {
		return errors.New("Caminho Git inválido")
	}
	current := g.root
	for _, part := range strings.Split(relative, "/") {
		current = filepath.Join(current, part)
		info, err := os.Lstat(current)
		if err == nil && info.Mode()&os.ModeSymlink != 0 {
			return errors.New("Links simbólicos não são permitidos no cofre Git")
		}
	}
	return nil
}
func (g *GitVault) PublishNewVault() (Status, error) {
	status, err := g.Status()
	if err != nil {
		return Status{}, err
	}
	if status.Upstream != nil {
		return Status{}, errors.New("Este cofre já tem upstream. Use Enviar para sincronizar")
	}
	if _, ok := g.tryOutput("remote", "get-url", "origin"); !ok {
		return Status{}, errors.New("Adicione um remoto origin antes de publicar o cofre")
	}
	remote, err := g.output("ls-remote", "origin")
	if err != nil {
		return Status{}, err
	}
	if strings.TrimSpace(remote) != "" {
		return Status{}, errors.New("O remoto já contém histórico. Conecte esse cofre manualmente pelo Git")
	}
	if _, hasHead := g.tryOutput("rev-parse", "--verify", "HEAD"); !hasHead {
		staged, err := g.output("diff", "--cached", "--name-only")
		if err != nil {
			return Status{}, err
		}
		if staged != "" {
			return Status{}, errors.New("Há arquivos preparados no Git. Revise-os antes de publicar")
		}
		files, err := g.collectVaultFiles()
		if err != nil {
			return Status{}, err
		}
		for _, path := range files {
			if strings.HasSuffix(path, ".gpg") {
				data, err := os.ReadFile(filepath.Join(g.root, path))
				if err != nil {
					return Status{}, err
				}
				if err = g.ValidateCiphertext(data); err != nil {
					return Status{}, err
				}
			}
		}
		args := []string{"add", "--"}
		for _, path := range files {
			args = append(args, ":(literal)"+path)
		}
		if _, err = g.output(args...); err != nil {
			return Status{}, err
		}
		if err = g.verifyStaged(files); err != nil {
			unstage := []string{"rm", "--cached", "-q", "--"}
			for _, path := range files {
				unstage = append(unstage, ":(literal)"+path)
			}
			g.git(unstage...)
			return Status{}, err
		}
		if _, err = g.output("commit", "-m", "Initialize encrypted vault"); err != nil {
			return Status{}, errors.New("Falha ao criar commit. Configure user.name e user.email no Git")
		}
	} else if status.Dirty {
		return Status{}, errors.New("Há alterações locais. Registre ou descarte antes de publicar")
	}
	status, err = g.Status()
	if err != nil {
		return Status{}, err
	}
	if _, err = g.output("push", "--set-upstream", "origin", "HEAD:refs/heads/"+status.Branch); err != nil {
		return Status{}, err
	}
	return g.Status()
}
func (g *GitVault) CommitChanges() (Status, error) {
	if _, ok := g.tryOutput("rev-parse", "--verify", "HEAD"); !ok {
		return Status{}, errors.New("Publique o cofre novo antes de registrar alterações")
	}
	staged, err := g.output("diff", "--cached", "--name-only")
	if err != nil {
		return Status{}, err
	}
	if staged != "" {
		return Status{}, errors.New("Há alterações já preparadas no Git. Revise-as pelo terminal")
	}
	out, err := g.git("ls-files", "-m", "-d", "-o", "--exclude-standard", "-z")
	if err != nil {
		return Status{}, errors.New("Falha ao listar alterações Git")
	}
	paths := []string{}
	for _, path := range strings.Split(string(out), "\x00") {
		if path == "" {
			continue
		}
		if !validRelative(path) {
			return Status{}, errors.New("Caminho Git inválido")
		}
		if !strings.HasSuffix(path, ".gpg") && filepath.Base(path) != ".gpg-id" {
			return Status{}, errors.New("Há arquivos fora do formato do cofre. Revise-os pelo terminal")
		}
		if err = g.rejectSymlinks(path); err != nil {
			return Status{}, err
		}
		full := filepath.Join(g.root, path)
		if info, err := os.Lstat(full); err == nil {
			if !info.Mode().IsRegular() {
				return Status{}, errors.New("O cofre contém links ou arquivos especiais. Revise-os pelo terminal")
			}
			if strings.HasSuffix(path, ".gpg") {
				data, err := os.ReadFile(full)
				if err != nil {
					return Status{}, err
				}
				if err = g.ValidateCiphertext(data); err != nil {
					return Status{}, err
				}
			}
		}
		paths = append(paths, ":(literal)"+path)
	}
	if len(paths) == 0 {
		return Status{}, errors.New("Não há alterações no cofre para registrar")
	}
	args := append([]string{"add", "-A", "--"}, paths...)
	if _, err = g.output(args...); err != nil {
		return Status{}, err
	}
	if err = g.verifyStaged(paths); err != nil {
		args = append([]string{"reset", "-q", "--"}, paths...)
		g.git(args...)
		return Status{}, err
	}
	if _, err = g.output("commit", "-m", "Update encrypted vault"); err != nil {
		return Status{}, errors.New("Falha ao criar commit. Configure user.name e user.email no Git")
	}
	return g.Status()
}
func (g *GitVault) MergePreservingBoth() (Status, error) {
	before, err := g.requireCleanUpstream()
	if err != nil {
		return Status{}, err
	}
	remote, err := g.remote(before.Branch)
	if err != nil {
		return Status{}, err
	}
	if _, err = g.output("fetch", "--no-tags", remote); err != nil {
		return Status{}, err
	}
	status, err := g.Status()
	if err != nil {
		return Status{}, err
	}
	if status.Ahead == 0 || status.Behind == 0 {
		return Status{}, errors.New("Não há históricos divergentes para resolver")
	}
	localHead, err := g.output("rev-parse", "HEAD")
	if err != nil {
		return Status{}, err
	}
	remoteHead, err := g.output("rev-parse", "@{upstream}")
	if err != nil {
		return Status{}, err
	}
	prefix := fmt.Sprintf("refs/passage/backup/%d", time.Now().UnixNano())
	if _, err = g.output("update-ref", prefix+"-local", strings.TrimSpace(localHead)); err != nil {
		return Status{}, err
	}
	if _, err = g.output("update-ref", prefix+"-remote", strings.TrimSpace(remoteHead)); err != nil {
		return Status{}, err
	}
	_, mergeErr := g.git("merge", "--no-commit", "--no-ff", "@{upstream}")
	created := []string{}
	rollback := func(reason error) (Status, error) {
		g.git("merge", "--abort")
		for _, path := range created {
			os.Remove(path)
		}
		return Status{}, reason
	}
	unmerged, err := g.git("ls-files", "-u", "-z")
	if err != nil {
		return rollback(errors.New("Índice Git inválido"))
	}
	conflicts := map[string]bool{}
	for _, record := range strings.Split(string(unmerged), "\x00") {
		if record == "" {
			continue
		}
		meta, path, ok := strings.Cut(record, "\t")
		if !ok {
			return rollback(errors.New("Índice Git inválido"))
		}
		if !strings.HasPrefix(meta, "100644 ") && !strings.HasPrefix(meta, "100755 ") {
			return rollback(errors.New("Conflito com link ou arquivo especial. Resolva pelo Git; commits preservados"))
		}
		conflicts[path] = true
	}
	if mergeErr != nil && len(conflicts) == 0 {
		return rollback(errors.New("O Git não conseguiu iniciar a mesclagem"))
	}
	names := make([]string, 0, len(conflicts))
	for path := range conflicts {
		names = append(names, path)
	}
	sort.Strings(names)
	for _, path := range names {
		if !strings.HasSuffix(path, ".gpg") || !validRelative(path) {
			return rollback(errors.New("Conflito fora de uma entrada .gpg; commits preservados"))
		}
		if err = g.rejectSymlinks(path); err != nil {
			return rollback(err)
		}
		local, errLocal := g.git("show", ":2:"+path)
		remote, errRemote := g.git("show", ":3:"+path)
		if errLocal != nil || errRemote != nil {
			return rollback(errors.New("Conflito com remoção ou renomeação; commits preservados"))
		}
		if err = g.ValidateCiphertext(local); err != nil {
			return rollback(err)
		}
		if err = g.ValidateCiphertext(remote); err != nil {
			return rollback(err)
		}
		suffix := strings.TrimSpace(remoteHead)
		if len(suffix) < 8 {
			return rollback(errors.New("Commit remoto inválido"))
		}
		suffix = suffix[:8]
		base := strings.TrimSuffix(filepath.Base(path), ".gpg")
		backup := filepath.ToSlash(filepath.Join(filepath.Dir(path), base+".remote-"+suffix+".gpg"))
		full := filepath.Join(g.root, backup)
		file, err := os.OpenFile(full, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
		if err != nil {
			return rollback(errors.New("Já existe cópia com esse nome; commits preservados"))
		}
		created = append(created, full)
		if _, err = file.Write(remote); err != nil {
			file.Close()
			return rollback(err)
		}
		if err = file.Close(); err != nil {
			return rollback(err)
		}
		if _, err = g.output("checkout", "--ours", "--", ":(literal)"+path); err != nil {
			return rollback(err)
		}
		if _, err = g.output("add", "--", ":(literal)"+path, ":(literal)"+backup); err != nil {
			return rollback(err)
		}
	}
	remaining, err := g.output("diff", "--name-only", "--diff-filter=U")
	if err != nil {
		return rollback(err)
	}
	if remaining != "" {
		return rollback(errors.New("Restaram conflitos Git; commits preservados"))
	}
	if _, err = g.output("commit", "-m", "Merge encrypted vault preserving both versions"); err != nil {
		return rollback(err)
	}
	return g.Status()
}
