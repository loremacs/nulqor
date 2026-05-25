package lmstudio

import "testing"

func TestNormalizeBaseURL(t *testing.T) {
	tests := map[string]string{
		"":                        "http://localhost:1234/v1",
		"http://localhost:1234":   "http://localhost:1234/v1",
		"http://localhost:1234/":  "http://localhost:1234/v1",
		"http://localhost:1234/v1": "http://localhost:1234/v1",
		"http://127.0.0.1:1234":   "http://127.0.0.1:1234/v1",
	}

	for input, want := range tests {
		if got := NormalizeBaseURL(input); got != want {
			t.Fatalf("NormalizeBaseURL(%q) = %q, want %q", input, got, want)
		}
	}
}
