FROM oven/bun:1.4.3 AS bun
FROM golang:1.27-bookworm

# The Go image includes git and C/C++ build tools. Use HTTPS for scanner tooling.
RUN sed -i 's|http://deb.debian.org|https://deb.debian.org|g' /etc/apt/sources.list.d/debian.sources
COPY --from=bun /usr/local/bin/bun /usr/local/bin/bun

# Keep the checkout and toolchains available when the scanner disconnects networking.
COPY . /src
WORKDIR /src/frontend
RUN bun install --frozen-lockfile \
    && bun run build

WORKDIR /src/backend
RUN go mod download \
    && mkdir -p internal/static/dist \
    && cp -a /src/frontend/build/. internal/static/dist/ \
    && CGO_ENABLED=0 go build -o /src/backend/server ./cmd/server \
    && go test -run '^$' ./...

WORKDIR /src
CMD ["bash"]
