# Contributing to BoxBox

Thanks for helping improve BoxBox. Keep changes focused, easy to review, and safe for people running the Docker image on their own servers.

## Before opening an issue

- Use bug reports for reproducible defects.
- Use Discussions for setup help, questions, broad ideas, and deployment tips.
- Search existing issues and discussions first.
- Redact secrets, tokens, hostnames, and private paths from logs or config.

## Development setup

Requirements:

- Go 1.24+
- Bun 1.1+
- Node.js 22+

Install dependencies:

```bash
bun install --cwd frontend
bun install --cwd website
cd backend && go mod download
```

Run the app locally:

```bash
bun run dev
```

## Checks before a pull request

Run the checks that match your change:

```bash
cd backend && go test ./...
bun run --cwd frontend check
bun run --cwd frontend build
bun run --cwd website build
```

For Docker or release changes, also verify the image builds:

```bash
docker build -t boxbox:local .
```

## Branches and releases

- Do normal work on feature branches.
- Use `test/*` branches when you want GitHub to build a clearly unstable test image.
- Test images use tags like `ghcr.io/jr4dh3y/boxbox:branch-test-my-change` and never replace `latest`.
- Public releases come from `v*` tags after changes are merged to `master`.

See [docs/release.md](docs/release.md) for the full release flow.

## Code guidelines

- Reuse existing handlers, services, utilities, components, and config before adding new ones.
- Keep handlers thin; put business logic in services.
- Use the filesystem abstraction and validate paths against configured mount points.
- Use Svelte 5 patterns already present in the frontend.
- Keep PRs small and explain user-facing behavior, upgrade impact, and rollback notes when relevant.
