package main

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"regexp"
	"time"
)

var profileID = regexp.MustCompile(`^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`)

// openRemote saves or reuses the deployment in the server's profile store,
// which owns device keys, and returns the location that browses it.
func (a *desktopApp) openRemote(remote string) (launchTarget, error) {
	ctx, cancel := context.WithTimeout(a.ctx, 10*time.Second)
	defer cancel()
	resp, err := a.server.request(ctx, http.MethodPost, "/api/v1/desktop/servers/open", map[string]string{"url": remote})
	if err != nil {
		return launchTarget{}, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return launchTarget{}, errors.New("could not open the saved BoxBox server; use an HTTPS deployment URL without a query or fragment")
	}
	var profile struct {
		ID string `json:"id"`
	}
	if err := json.NewDecoder(io.LimitReader(resp.Body, 1<<20)).Decode(&profile); err != nil || !profileID.MatchString(profile.ID) {
		return launchTarget{}, errors.New("invalid saved server response")
	}
	return launchTarget{Path: "@remote/" + profile.ID}, nil
}
