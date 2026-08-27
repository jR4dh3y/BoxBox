# =============================================================================
# BoxBox - Unified Multi-Stage Dockerfile
# =============================================================================
# This Dockerfile builds both frontend and backend into a single container.
# Frontend is compiled to static files and embedded in the Go binary via go:embed.
#
# Build: docker build -t boxbox .
# Run:   docker run -p 80:80 -v /your/files:/media/files boxbox
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Build Frontend with Bun
# -----------------------------------------------------------------------------
FROM oven/bun:1-alpine@sha256:07235578f79ef8c6f97d94aee7938e76f5cdba5f21ae5dbfdd3d3d38058437eb AS frontend-builder

WORKDIR /app

# Install dependencies first (layer caching optimization)
COPY frontend/package.json frontend/bun.lock ./
RUN bun install --frozen-lockfile

# Copy source and build
COPY frontend/ ./
RUN bun run build

# -----------------------------------------------------------------------------
# Stage 2: Build Backend with Go (embeds frontend assets)
# -----------------------------------------------------------------------------
FROM golang:1.26-alpine@sha256:0178a641fbb4858c5f1b48e34bdaabe0350a330a1b1149aabd498d0699ff5fb2 AS backend-builder

WORKDIR /app

# Install build dependencies
RUN apk add --no-cache git

# Download Go dependencies first (layer caching optimization)
COPY backend/go.mod backend/go.sum ./
RUN go mod download

# Copy backend source
COPY backend/ ./

# Copy frontend build into static/dist for embedding via go:embed
COPY --from=frontend-builder /app/build ./internal/static/dist/

# Build the binary with embedded static files
# CGO_ENABLED=0 for static binary, ldflags for smaller size
RUN CGO_ENABLED=0 GOOS=linux go build \
    -ldflags="-w -s" \
    -o /server \
    ./cmd/server

# -----------------------------------------------------------------------------
# Stage 3: Minimal Production Runtime
# -----------------------------------------------------------------------------
FROM alpine:3.24@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b

WORKDIR /app

# Install runtime dependencies
# - ca-certificates: HTTPS support
# - tzdata: Timezone support
# - wget: Health check
RUN apk add --no-cache ca-certificates tzdata wget

# Copy binary from builder
COPY --from=backend-builder /server /app/server

# Copy default config
COPY backend/config.yaml /app/config.yaml

# Create writable runtime directories owned by the unprivileged runtime user.
RUN addgroup -g 10001 boxbox \
    && adduser -D -H -u 10001 -G boxbox boxbox \
    && mkdir -p /data /tmp/boxbox \
    && chown -R boxbox:boxbox /app /data /tmp/boxbox \
    && chmod 0700 /tmp/boxbox

# Store upload chunk temp files on a dedicated writable path
ENV TMPDIR=/tmp/boxbox

# Expose the server port (port 80 for Traefik compatibility)
EXPOSE 80

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -qO- http://localhost:80/health | grep -q 'ok' || exit 1

USER 10001:10001

# Run the server
ENTRYPOINT ["/app/server"]
