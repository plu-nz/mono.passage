package main

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/url"
	"os"
	"regexp"
	"strings"

	"github.com/plu-nz/passage/internal/pairing"
	"github.com/plu-nz/passage/internal/vault"
)

const maxMessage = 64 * 1024

var extensionPattern = regexp.MustCompile(`^chrome-extension://[a-p]{32}/$`)

func main() {
	if err := run(os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run(args []string) error {
	store, err := pairing.OpenDefault()
	if err != nil {
		return err
	}
	if len(args) == 0 {
		return errors.New("missing extension source")
	}
	switch args[0] {
	case "list":
		items, err := store.ListAll()
		if err != nil {
			return err
		}
		for _, item := range items {
			fmt.Printf("%s %s\n", item.Origin, item.InstallationID)
		}
		return nil
	case "list-json":
		active, err := vault.OpenEnv()
		if err != nil {
			return err
		}
		items, err := store.ListCurrent(active.Root())
		if err != nil {
			return err
		}
		type item struct {
			Origin         string `json:"origin"`
			InstallationID string `json:"installationId"`
		}
		result := make([]item, 0, len(items))
		for _, p := range items {
			result = append(result, item{p.Origin, p.InstallationID})
		}
		return json.NewEncoder(os.Stdout).Encode(result)
	case "approve":
		if len(args) != 2 {
			return errors.New("uso: passage-native-host approve CODIGO")
		}
		item, err := store.Approve(args[1])
		if err != nil {
			return err
		}
		fmt.Printf("Pairing approved: %s (%s) — %s\n", item.Origin, item.InstallationID, item.VaultPath)
		return nil
	case "revoke":
		if len(args) != 3 || !extensionPattern.MatchString(args[1]) || args[2] == "" {
			return errors.New("usage: passage-native-host revoke ORIGIN ID")
		}
		removed, err := store.Revoke(args[1], args[2])
		if err != nil {
			return err
		}
		if !removed {
			return errors.New("installation not found")
		}
		fmt.Println("Access revoked.")
		return nil
	case "lock-status":
		locked, err := store.Locked()
		if err != nil {
			return err
		}
		return json.NewEncoder(os.Stdout).Encode(map[string]bool{"locked": locked})
	case "lock", "unlock":
		locked := args[0] == "lock"
		if err := store.SetLocked(locked); err != nil {
			return err
		}
		if locked {
			fmt.Println("Extension access blocked.")
		} else {
			fmt.Println("Extension access unlocked.")
		}
		return nil
	}
	if !extensionPattern.MatchString(args[0]) {
		return errors.New("invalid extension source")
	}
	for {
		request, err := readMessage(os.Stdin)
		if errors.Is(err, io.EOF) {
			return nil
		}
		if err != nil {
			return err
		}
		response := handle(store, args[0], request)
		if err := writeMessage(os.Stdout, response); err != nil {
			return err
		}
	}
}

func readMessage(input io.Reader) (map[string]json.RawMessage, error) {
	var header [4]byte
	if _, err := io.ReadFull(input, header[:]); err != nil {
		return nil, err
	}
	size := binary.LittleEndian.Uint32(header[:])
	if size == 0 || size > maxMessage {
		return nil, errors.New("mensagem muito grande")
	}
	data := make([]byte, size)
	if _, err := io.ReadFull(input, data); err != nil {
		return nil, err
	}
	var request map[string]json.RawMessage
	if err := json.Unmarshal(data, &request); err != nil || request == nil {
		return nil, errors.New("Invalid JSON")
	}
	return request, nil
}

func writeMessage(output io.Writer, value any) error {
	data, err := json.Marshal(value)
	if err != nil {
		return err
	}
	if len(data) > maxMessage {
		return errors.New("resposta muito grande")
	}
	var header [4]byte
	binary.LittleEndian.PutUint32(header[:], uint32(len(data)))
	_, err = io.Copy(output, bytes.NewReader(append(header[:], data...)))
	return err
}

func field(request map[string]json.RawMessage, name string) (string, error) {
	var value string
	if err := json.Unmarshal(request[name], &value); err != nil || len(value) == 0 || len(value) > 4096 {
		return "", fmt.Errorf("invalid %s field", name)
	}
	return value, nil
}

func handle(store *pairing.Store, extension string, request map[string]json.RawMessage) map[string]any {
	requestID, _ := field(request, "requestId")
	response, err := dispatch(store, extension, request)
	if err != nil {
		return map[string]any{"type": "error", "requestId": requestID, "message": "Pedido recusado"}
	}
	response["requestId"] = requestID
	return response
}

func dispatch(store *pairing.Store, extension string, request map[string]json.RawMessage) (map[string]any, error) {
	if _, err := field(request, "requestId"); err != nil {
		return nil, err
	}
	kind, err := field(request, "type")
	if err != nil {
		return nil, err
	}
	if kind == "ping" {
		return map[string]any{"type": "pong", "version": 1}, nil
	}
	id, err := field(request, "installationId")
	if err != nil {
		return nil, err
	}
	active, err := vault.OpenEnv()
	if err != nil {
		return nil, err
	}
	root := active.Root()
	switch kind {
	case "status":
		paired, locked, err := store.Status(extension, id, root)
		return map[string]any{"type": "status", "version": 1, "paired": paired, "locked": locked}, err
	case "pair":
		code, expiry, err := store.Pair(extension, id, root)
		return map[string]any{"type": "pairPending", "code": code, "expiresAt": expiry}, err
	case "pairStatus":
		token, err := store.Poll(extension, id, root)
		if token == "" {
			return map[string]any{"type": "pairPending"}, err
		}
		return map[string]any{"type": "paired", "token": token}, err
	case "listForOrigin", "readForOrigin":
		token, err := field(request, "token")
		if err != nil {
			return nil, err
		}
		originValue, err := field(request, "origin")
		if err != nil {
			return nil, err
		}
		origin, err := siteOrigin(originValue, true)
		if err != nil {
			return nil, err
		}
		allowed, err := store.Authorized(extension, id, token, root)
		if err != nil || !allowed {
			return nil, errors.New("unauthorized extension")
		}
		entries, err := active.List()
		if err != nil {
			return nil, err
		}
		if kind == "listForOrigin" {
			matched := []map[string]any{}
			for _, entry := range entries {
				secret, err := active.Read(entry.ID)
				if err != nil || !matchesSite(secret.URLs, origin) {
					continue
				}
				matched = append(matched, map[string]any{"id": entry.ID, "title": entry.Title, "username": secret.Username})
			}
			allowed, err = store.Authorized(extension, id, token, root)
			if err != nil || !allowed {
				return nil, errors.New("access locked or revoked")
			}
			return map[string]any{"type": "entries", "entries": matched}, nil
		}
		entryID, err := field(request, "id")
		if err != nil {
			return nil, err
		}
		found := false
		for _, entry := range entries {
			if entry.ID == entryID {
				found = true
				break
			}
		}
		if !found {
			return nil, errors.New("entry unavailable")
		}
		secret, err := active.Read(entryID)
		if err != nil || !matchesSite(secret.URLs, origin) {
			return nil, errors.New("entry unavailable")
		}
		allowed, err = store.Authorized(extension, id, token, root)
		if err != nil || !allowed {
			return nil, errors.New("access locked or revoked")
		}
		return map[string]any{"type": "secret", "id": entryID, "username": secret.Username, "password": secret.Password}, nil
	}
	return nil, errors.New("pedido desconhecido")
}

func siteOrigin(raw string, exact bool) (string, error) {
	u, err := url.Parse(raw)
	if err != nil || u.Scheme != "https" || u.Hostname() == "" || u.User != nil || u.Opaque != "" {
		return "", errors.New("invalid origin")
	}
	if exact && (u.Path != "" && u.Path != "/" || u.RawQuery != "" || u.Fragment != "") {
		return "", errors.New("invalid origin")
	}
	host := strings.ToLower(u.Hostname())
	if strings.Contains(host, ":") {
		host = "[" + host + "]"
	}
	if port := u.Port(); port != "" && port != "443" {
		host += ":" + port
	}
	return "https://" + host, nil
}

func matchesSite(urls []string, origin string) bool {
	for _, raw := range urls {
		candidate, err := siteOrigin(raw, false)
		if err == nil && candidate == origin {
			return true
		}
	}
	return false
}
