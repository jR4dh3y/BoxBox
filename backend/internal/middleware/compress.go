package middleware

import chimiddleware "github.com/go-chi/chi/v5/middleware"

// JSONCompression gzips JSON responses for clients that accept it. Use it on large listings,
// which repeat the same keys for every entry, and not on small bodies, which gzip makes bigger.
// Only application/json is compressed, so file contents and media streams are untouched.
var JSONCompression = chimiddleware.Compress(5, "application/json")
