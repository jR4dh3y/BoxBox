package main

import (
	"os"

	"golang.org/x/crypto/bcrypt"
)

// setDevCredentials gives --dev the secret and the user that config validation
// demands. They exist only in this process; --dev bypasses authentication.
func setDevCredentials() error {
	// Validation rejects hashes cheaper than bcrypt.DefaultCost, so --dev pays it too.
	hash, err := bcrypt.GenerateFromPassword([]byte("development-mode"), bcrypt.DefaultCost)
	if err != nil {
		return err
	}
	if err := os.Setenv("BOXBOX_JWT_SECRET", "boxbox-development-mode-not-for-production"); err != nil {
		return err
	}
	return os.Setenv("BOXBOX_USERS_dev", string(hash))
}
