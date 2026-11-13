package otp

import (
	"crypto/hmac"
	"crypto/sha1"
	"crypto/sha256"
	"crypto/sha512"
	"encoding/base32"
	"encoding/binary"
	"errors"
	"fmt"
	"hash"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/plu-nz/passage/internal/vault"
)

type Code struct {
	Code       string `json:"code"`
	Period     uint64 `json:"period"`
	Remaining  uint64 `json:"remaining"`
	ValidUntil uint64 `json:"validUntil"`
}

func Generate(secret vault.Secret) (Code, error) {
	uri := findURI(secret)
	if uri == "" {
		return Code{}, errors.New("Esta entrada não contém um URI TOTP compatível com pass-otp")
	}
	return CodeAt(uri, uint64(time.Now().Unix()))
}
func findURI(secret vault.Secret) string {
	for _, line := range append([]string{secret.Password}, strings.Split(secret.Details, "\n")...) {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "otpauth://") {
			return line
		}
		if key, value, ok := strings.Cut(line, ":"); ok && strings.EqualFold(key, "otpauth") && strings.HasPrefix(strings.TrimSpace(value), "otpauth://") {
			return strings.TrimSpace(value)
		}
	}
	return ""
}
func CodeAt(uri string, now uint64) (Code, error) {
	if len(uri) > 8192 {
		return Code{}, errors.New("URI TOTP muito longo")
	}
	parsed, err := url.Parse(uri)
	if err != nil {
		return Code{}, errors.New("URI TOTP inválido")
	}
	if parsed.Scheme != "otpauth" || parsed.Host != "totp" || strings.Trim(parsed.Path, "/") == "" {
		return Code{}, errors.New("Somente URI otpauth://totp é suportado")
	}
	values, err := url.ParseQuery(parsed.RawQuery)
	if err != nil {
		return Code{}, errors.New("URI TOTP inválido")
	}
	for _, key := range []string{"secret", "algorithm", "digits", "period"} {
		if len(values[key]) > 1 {
			return Code{}, errors.New("Parâmetro TOTP duplicado")
		}
	}
	secret := values.Get("secret")
	if secret == "" {
		return Code{}, errors.New("URI TOTP sem segredo")
	}
	secret = strings.ToUpper(strings.Join(strings.Fields(secret), ""))
	secret = strings.TrimRight(secret, "=")
	key, err := base32.StdEncoding.WithPadding(base32.NoPadding).DecodeString(secret)
	if err != nil || len(key) == 0 {
		return Code{}, errors.New("Segredo TOTP Base32 inválido")
	}
	algorithm := strings.ToUpper(values.Get("algorithm"))
	if algorithm == "" {
		algorithm = "SHA1"
	}
	digits := 6
	if value := values.Get("digits"); value != "" {
		digits, err = strconv.Atoi(value)
		if err != nil {
			return Code{}, errors.New("Quantidade de dígitos TOTP inválida")
		}
	}
	if digits < 6 || digits > 8 {
		return Code{}, errors.New("Quantidade de dígitos TOTP inválida")
	}
	period := uint64(30)
	if value := values.Get("period"); value != "" {
		period, err = strconv.ParseUint(value, 10, 64)
		if err != nil {
			return Code{}, errors.New("Período TOTP inválido")
		}
	}
	if period < 1 || period > 3600 {
		return Code{}, errors.New("Período TOTP inválido")
	}
	var hashFactory func() hash.Hash
	switch algorithm {
	case "SHA1":
		hashFactory = sha1.New
	case "SHA256":
		hashFactory = sha256.New
	case "SHA512":
		hashFactory = sha512.New
	default:
		return Code{}, errors.New("Algoritmo TOTP não suportado")
	}
	mac := hmac.New(hashFactory, key)
	var counter [8]byte
	binary.BigEndian.PutUint64(counter[:], now/period)
	mac.Write(counter[:])
	digest := mac.Sum(nil)
	offset := int(digest[len(digest)-1] & 0xf)
	number := binary.BigEndian.Uint32(digest[offset:offset+4]) & 0x7fffffff
	modulus := uint32(1)
	for range digits {
		modulus *= 10
	}
	remaining := period - now%period
	return Code{Code: fmt.Sprintf("%0*d", digits, number%modulus), Period: period, Remaining: remaining, ValidUntil: now + remaining}, nil
}
