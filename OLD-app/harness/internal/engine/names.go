package engine

import (
	"crypto/rand"
	"fmt"
	"math/big"
	"regexp"
	"strings"
)

const participantNameMaxLen = 32

var customParticipantPattern = regexp.MustCompile(`^[a-zA-Z0-9][a-zA-Z0-9 _-]{1,31}$`)

// SanitizeDisplayName normalizes a human-entered participant label.
func SanitizeDisplayName(name string) (string, error) {
	name = strings.TrimSpace(name)
	if name == "" {
		return "", fmt.Errorf("display name cannot be empty")
	}
	if len(name) > participantNameMaxLen {
		return "", fmt.Errorf("display name must be %d characters or fewer", participantNameMaxLen)
	}
	if !customParticipantPattern.MatchString(name) {
		return "", fmt.Errorf("display name must start with a letter or number and use letters, numbers, spaces, _, or - only")
	}
	return name, nil
}

// ValidateParticipantName checks a custom external-agent name.
func ValidateParticipantName(name string) error {
	name = strings.TrimSpace(name)
	if name == "" {
		return fmt.Errorf("name cannot be empty")
	}
	if len(name) > participantNameMaxLen {
		return fmt.Errorf("name must be %d characters or fewer", participantNameMaxLen)
	}
	if !customParticipantPattern.MatchString(name) {
		return fmt.Errorf("name must start with a letter or number and use letters, numbers, spaces, _, or - only")
	}
	return nil
}

// GenerateParticipantName returns a random machine-friendly name such as agent-k7m2x9.
func GenerateParticipantName(prefix string) string {
	prefix = strings.TrimSpace(prefix)
	if prefix == "" {
		prefix = "agent"
	}
	suffix := randomAlphaNum(6)
	return fmt.Sprintf("%s-%s", prefix, suffix)
}

func randomAlphaNum(n int) string {
	const alphabet = "abcdefghijklmnopqrstuvwxyz0123456789"
	out := make([]byte, n)
	for i := range out {
		idx, err := rand.Int(rand.Reader, big.NewInt(int64(len(alphabet))))
		if err != nil {
			out[i] = alphabet[i%len(alphabet)]
			continue
		}
		out[i] = alphabet[idx.Int64()]
	}
	return string(out)
}

func (e *Engine) generateUniqueParticipantName(prefix string) string {
	for i := 0; i < 32; i++ {
		name := GenerateParticipantName(prefix)
		if !e.IsObserverRegistered(name) {
			return name
		}
	}
	return GenerateParticipantName(prefix)
}
