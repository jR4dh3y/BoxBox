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
FROM oven/bun:1-alpine AS frontend-builder

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
FROM golang:1.24-alpine AS backend-builder

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
FROM alpine:3.24

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

# Create writable runtime directories
RUN mkdir -p /data /tmp/boxbox && chmod 1777 /tmp /tmp/boxbox

# Store upload chunk temp files on a dedicated writable path
ENV TMPDIR=/tmp/boxbox

# Expose the server port (port 80 for Traefik compatibility)
EXPOSE 80

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -qO- http://localhost:80/health | grep -q 'ok' || exit 1

# Run the server
ENTRYPOINT ["/app/server"]
