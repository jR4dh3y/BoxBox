package main

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"log"
	"net/http"
	"net/url"
	"regexp"
	"slices"
	"strings"
	"time"

	"github.com/egoist/mygo"
)

var chooserID = regexp.MustCompile(`^[a-f0-9]{32}$`)

// watchChoosers keeps one window open for each file chooser request the
// server's portal holds. The server answers when the set changes.
func (a *desktopApp) watchChoosers() {
	known := ""
	for a.ctx.Err() == nil {
		ids, err := a.chooserRequests(known)
		if err != nil {
			if a.ctx.Err() != nil {
				return
			}
			// A transient server failure must not strand later portal requests.
			log.Printf("File chooser poll failed; retrying: %v", err)
			select {
			case <-a.ctx.Done():
			case <-time.After(500 * time.Millisecond):
			}
			continue
		}
		mygo.RunOnMain(func() { a.showChoosers(ids) })
		known = strings.Join(ids, ",")
	}
}

func (a *desktopApp) chooserRequests(known string) ([]string, error) {
	resp, err := a.server.request(a.ctx, http.MethodGet, "/api/v1/desktop/chooser?known="+url.QueryEscape(known), nil)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, errors.New(resp.Status)
	}
	var list []struct {
		ID string `json:"id"`
	}
	if err := json.NewDecoder(io.LimitReader(resp.Body, 1<<20)).Decode(&list); err != nil {
		return nil, err
	}
	ids := make([]string, 0, len(list))
	for _, request := range list {
		if !chooserID.MatchString(request.ID) {
			return nil, errors.New("invalid chooser request")
		}
		ids = append(ids, request.ID)
	}
	slices.Sort(ids)
	return ids, nil
}

// showChoosers closes the windows of finished requests and opens new ones.
func (a *desktopApp) showChoosers(ids []string) {
	for id, win := range a.choosers {
		if !slices.Contains(ids, id) {
			delete(a.choosers, id)
			win.Close()
		}
	}
	for _, id := range ids {
		if _, open := a.choosers[id]; open {
			continue
		}
		win := a.newWindow("Choose a file · BoxBox", a.server.bootstrapURL("/chooser?id="+id), "chooser")
		a.choosers[id] = win
		win.OnClosed(func() {
			if a.choosers[id] != win {
				return
			}
			// Closed by the user: the requesting app gets a cancellation.
			delete(a.choosers, id)
			go a.cancelChooser(id)
		})
	}
}

func (a *desktopApp) cancelChooser(id string) {
	ctx, cancel := context.WithTimeout(a.ctx, 5*time.Second)
	defer cancel()
	resp, err := a.server.request(ctx, http.MethodPost, "/api/v1/desktop/chooser/"+id, map[string]bool{"cancel": true})
	if err != nil {
		return
	}
	resp.Body.Close()
}
