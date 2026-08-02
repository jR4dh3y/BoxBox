// Package middleware provides HTTP middleware for the BoxBox API.
package middleware

import (
	"net"
	"net/http"
	"net/netip"
	"strings"
	"sync"
	"time"

	"golang.org/x/time/rate"
)

// RateLimiter provides per-IP rate limiting for HTTP requests.
// It uses a token bucket algorithm to allow bursts while enforcing
// a sustained rate limit.
type RateLimiter struct {
	limiters        map[string]*clientLimiter
	mu              sync.RWMutex
	rps             float64
	burst           int
	idleTTL         time.Duration
	lastCleanup     time.Time
	cleanupInterval time.Duration
}

type clientLimiter struct {
	limiter  *rate.Limiter
	lastSeen time.Time
}

// NewRateLimiter creates a new rate limiter with the specified requests per second.
// The burst size is set to 2x the RPS to allow short bursts.
// If rps is 0 or negative, rate limiting is effectively disabled (very high limit).
func NewRateLimiter(rps float64) *RateLimiter {
	if rps <= 0 {
		rps = 1000 // Effectively disabled
	}
	burst := int(rps * 2)
	if burst < 1 {
		burst = 1
	}
	return &RateLimiter{
		limiters:        make(map[string]*clientLimiter),
		rps:             rps,
		burst:           burst,
		idleTTL:         30 * time.Minute,
		lastCleanup:     time.Now(),
		cleanupInterval: 5 * time.Minute,
	}
}

// getLimiter returns the rate limiter for the given IP address,
// creating one if it doesn't exist.
func (rl *RateLimiter) getLimiter(ip string) *rate.Limiter {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	now := time.Now()
	if entry, exists := rl.limiters[ip]; exists {
		entry.lastSeen = now
		return entry.limiter
	}

	limiter := rate.NewLimiter(rate.Limit(rl.rps), rl.burst)
	rl.limiters[ip] = &clientLimiter{limiter: limiter, lastSeen: now}
	return limiter
}

// Allow returns true if the request from the given IP should be allowed.
func (rl *RateLimiter) Allow(ip string) bool {
	rl.cleanupIfDue()
	return rl.getLimiter(ip).Allow()
}

func (rl *RateLimiter) cleanupIfDue() {
	rl.mu.RLock()
	due := time.Since(rl.lastCleanup) >= rl.cleanupInterval
	rl.mu.RUnlock()
	if !due {
		return
	}
	rl.cleanup()
}

// RateLimit returns a middleware that limits requests per IP address.
// Requests that exceed the rate limit receive a 429 Too Many Requests response.
func RateLimit(rps float64, trustedProxyValues ...string) func(http.Handler) http.Handler {
	limiter := NewRateLimiter(rps)
	trustedProxies := parseTrustedProxies(trustedProxyValues)

	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			ip := getClientIPFromTrustedProxies(r, trustedProxies)

			if !limiter.Allow(ip) {
				w.Header().Set("Content-Type", "application/json")
				w.Header().Set("Retry-After", "1")
				w.WriteHeader(http.StatusTooManyRequests)
				w.Write([]byte(`{"error":"Too many requests","code":"rate_limit_exceeded"}`))
				return
			}

			next.ServeHTTP(w, r)
		})
	}
}

// getClientIP extracts the client IP address from the socket address.
func getClientIP(r *http.Request) string {
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err == nil {
		return host
	}

	return r.RemoteAddr
}

func getClientIPFromTrustedProxies(r *http.Request, trusted []netip.Prefix) string {
	remote := getClientIP(r)
	remoteIP, err := netip.ParseAddr(remote)
	if err != nil || !isTrustedProxy(remoteIP, trusted) {
		return remote
	}

	forwarded := strings.Split(r.Header.Get("X-Forwarded-For"), ",")
	for i := len(forwarded) - 1; i >= 0; i-- {
		candidate, err := netip.ParseAddr(strings.TrimSpace(forwarded[i]))
		if err != nil {
			continue
		}
		if !isTrustedProxy(candidate, trusted) {
			return candidate.String()
		}
	}
	if realIP, err := netip.ParseAddr(strings.TrimSpace(r.Header.Get("X-Real-IP"))); err == nil {
		return realIP.String()
	}
	return remote
}

func parseTrustedProxies(values []string) []netip.Prefix {
	var prefixes []netip.Prefix
	for _, value := range values {
		for _, item := range strings.Split(value, ",") {
			item = strings.TrimSpace(item)
			if item == "" {
				continue
			}
			if prefix, err := netip.ParsePrefix(item); err == nil {
				prefixes = append(prefixes, prefix.Masked())
				continue
			}
			if address, err := netip.ParseAddr(item); err == nil {
				prefixes = append(prefixes, netip.PrefixFrom(address, address.BitLen()))
			}
		}
	}
	return prefixes
}

func isTrustedProxy(address netip.Addr, trusted []netip.Prefix) bool {
	for _, prefix := range trusted {
		if prefix.Contains(address) {
			return true
		}
	}
	return false
}

// StartCleanup starts a background goroutine that periodically removes
// stale rate limiters to prevent memory growth.
func (rl *RateLimiter) StartCleanup(interval time.Duration, stopCh <-chan struct{}) {
	go func() {
		ticker := time.NewTicker(interval)
		defer ticker.Stop()

		for {
			select {
			case <-stopCh:
				return
			case <-ticker.C:
				rl.cleanup()
			}
		}
	}()
}

// cleanup removes only idle client buckets, preserving active clients' rate
// history and preventing periodic brute-force windows.
func (rl *RateLimiter) cleanup() {
	rl.mu.Lock()
	defer rl.mu.Unlock()
	rl.lastCleanup = time.Now()
	cutoff := time.Now().Add(-rl.idleTTL)
	for key, entry := range rl.limiters {
		if entry.lastSeen.Before(cutoff) {
			delete(rl.limiters, key)
		}
	}
}
