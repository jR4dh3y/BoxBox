package handler

import (
	"net/http"
	"sort"
	"strings"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func RegisterDesktopChooser(r chi.Router, chooser *service.DesktopChooser) {
	r.Get("/chooser", func(w http.ResponseWriter, r *http.Request) {
		list, changed := chooser.List()
		ids := make([]string, 0, len(list))
		for _, request := range list {
			ids = append(ids, request.ID)
		}
		sort.Strings(ids)
		if r.URL.Query().Has("known") && r.URL.Query().Get("known") == strings.Join(ids, ",") {
			select {
			case <-changed:
			case <-r.Context().Done():
				return
			case <-time.After(20 * time.Second):
			}
			list, _ = chooser.List()
		}
		writeJSON(w, list, http.StatusOK)
	})
	r.Get("/chooser/{id}", func(w http.ResponseWriter, r *http.Request) {
		request, err := chooser.Get(chi.URLParam(r, "id"))
		if err != nil {
			HandleServiceError(w, err)
			return
		}
		writeJSON(w, request, http.StatusOK)
	})
	r.Get("/chooser/{id}/directory", func(w http.ResponseWriter, r *http.Request) {
		listing, err := chooser.Directory(r.Context(), chi.URLParam(r, "id"), r.URL.Query().Get("path"))
		if err != nil {
			HandleServiceError(w, err)
			return
		}
		writeJSON(w, directoryListing(listing), http.StatusOK)
	})
	r.Post("/chooser/{id}", func(w http.ResponseWriter, r *http.Request) {
		var selection service.ChooserSelection
		if err := decodeJSONBody(w, r, &selection); err != nil {
			writeError(w, "Invalid request body", model.ErrCodeValidationError, http.StatusBadRequest)
			return
		}
		err := chooser.Select(r.Context(), chi.URLParam(r, "id"), selection)
		if err != nil {
			HandleServiceError(w, err)
			return
		}
		writeJSON(w, desktopActionResponse{OK: true}, http.StatusOK)
	})
}
