package lmstudio

import "strings"

// NormalizeBaseURL trims input and ensures an OpenAI-compatible /v1 suffix.
func NormalizeBaseURL(endpoint string) string {
	endpoint = strings.TrimSpace(endpoint)
	if endpoint == "" {
		return "http://localhost:1234/v1"
	}

	endpoint = strings.TrimRight(endpoint, "/")
	lower := strings.ToLower(endpoint)
	if strings.HasSuffix(lower, "/v1") {
		return endpoint
	}

	return endpoint + "/v1"
}
