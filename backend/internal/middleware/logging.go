package middleware

import (
	"net/http"
	"time"

	chimiddleware "github.com/go-chi/chi/v5/middleware"
	"github.com/rs/zerolog/log"
)

// RequestLogger records request metadata without query strings. Query
// parameters can contain short-lived bearer tokens for streams and WebSockets.
func RequestLogger(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		started := time.Now()
		wrapped := chimiddleware.NewWrapResponseWriter(w, r.ProtoMajor)
		next.ServeHTTP(wrapped, r)

		log.Info().
			Str("request_id", chimiddleware.GetReqID(r.Context())).
			Str("method", r.Method).
			Str("path", r.URL.Path).
			Int("status", wrapped.Status()).
			Int("bytes", wrapped.BytesWritten()).
			Dur("duration", time.Since(started)).
			Msg("HTTP request")
	})
}
