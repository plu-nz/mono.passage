package otp

import (
	"encoding/base32"
	"fmt"
	"strings"
	"testing"
)

func TestRFC6238(t *testing.T) {
	vectors := []struct{ secret, algorithm, want string }{{"12345678901234567890", "SHA1", "94287082"}, {"12345678901234567890123456789012", "SHA256", "46119246"}, {"1234567890123456789012345678901234567890123456789012345678901234", "SHA512", "90693936"}}
	for _, v := range vectors {
		encoded := strings.TrimRight(base32.StdEncoding.EncodeToString([]byte(v.secret)), "=")
		uri := fmt.Sprintf("otpauth://totp/Test?secret=%s&algorithm=%s&digits=8&period=30", encoded, v.algorithm)
		got, err := CodeAt(uri, 59)
		if err != nil || got.Code != v.want {
			t.Fatalf("%s got %v, %v, want %s", v.algorithm, got, err, v.want)
		}
	}
}
func TestDefaultsAndInvalid(t *testing.T) {
	uri := "otpauth://totp/Test?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"
	got, err := CodeAt(uri, 59)
	if err != nil || got.Code != "287082" || got.Remaining != 1 || got.ValidUntil != 60 {
		t.Fatalf("got %+v %v", got, err)
	}
	for _, bad := range []string{"otpauth://hotp/Test?secret=JBSWY3DPEHPK3PXP", "otpauth://totp/Test?secret=bad!", "otpauth://totp/Test?secret=ABC&secret=DEF", "otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&period=0"} {
		if _, err := CodeAt(bad, 59); err == nil {
			t.Errorf("accepted %s", bad)
		}
	}
}
