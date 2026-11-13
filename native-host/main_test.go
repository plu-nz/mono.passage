package main

import (
	"bytes"
	"encoding/binary"
	"strings"
	"testing"
)

func TestOriginsAreExact(t *testing.T) {
	origin, err := siteOrigin("https://example.test:443/", true)
	if err != nil || origin != "https://example.test" {
		t.Fatalf("origin=%q err=%v", origin, err)
	}
	for _, invalid := range []string{"http://example.test", "https://example.test/path", "https://user@example.test", "javascript:alert(1)"} {
		if _, err := siteOrigin(invalid, true); err == nil {
			t.Errorf("accepted %q", invalid)
		}
	}
	if !matchesSite([]string{"https://example.test/login"}, "https://example.test") {
		t.Fatal("matching site missing")
	}
	if matchesSite([]string{"https://example.test.evil.invalid"}, "https://example.test") {
		t.Fatal("lookalike site matched")
	}
}

func TestNativeFrameAndSizeLimit(t *testing.T) {
	var framed bytes.Buffer
	if err := writeMessage(&framed, map[string]any{"type": "pong", "version": 1}); err != nil {
		t.Fatal(err)
	}
	request, err := readMessage(&framed)
	if err != nil || string(request["type"]) != `"pong"` {
		t.Fatalf("request=%v err=%v", request, err)
	}
	var oversize bytes.Buffer
	_ = binary.Write(&oversize, binary.LittleEndian, uint32(maxMessage+1))
	if _, err := readMessage(&oversize); err == nil || !strings.Contains(err.Error(), "grande") {
		t.Fatalf("oversize accepted: %v", err)
	}
}
