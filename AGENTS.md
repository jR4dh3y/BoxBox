# Working on BoxBox

BoxBox is a self-hosted file manager. A Go API server (`backend/`) embeds a SvelteKit app (`frontend/`).

Read the code that runs before you change it. Prefer the smallest complete change.

## Commands

- Backend, from `backend/`: `go vet ./...` and `go test ./...`. The service tests take about 80 seconds. Add `-race` for concurrency changes.
- Frontend, from `frontend/`: `bun run format`, `bun run check`, `bun run lint`, `bun run test`, `bun run build`. Tests use `node:test` and run under `bun test`. A test that imports `.svelte.ts` code needs the preload in `frontend/test/`, which `bun run test` already uses.
- Add a package with `bun add`. Do not edit `package.json` by hand.

## Code

- Use strong types. Do not use `any`. Do not use `as` to silence the compiler.
- Parse external data (WebSocket frames, JSON) into typed values at the boundary. Trust the types inside.
- Model states as unions, not as a bag of optional fields.
- Every Svelte component has `lang="ts"`. Use Tailwind for styling.
- Keep logic out of components. Put it in `frontend/src/lib/`.
- Split files by the knowledge each one owns. Do not write monolithic files.
- Before you add code, check whether it exists or whether you can extend what exists.
- Remove dead code and old paths when you replace them. Do not keep compatibility code for removed behaviour.
- Keep comments short. Explain a reason or a contract, not what the code does.

## Backend

- Resolve every virtual path (`media/movies/a.mkv`) with `mounts.resolve` in `backend/internal/service/mounts.go`. Do not call the validator from a service.
- Keep the confinement helpers in `path_safety.go` for real paths that are already resolved, such as job revalidation and share sub-folders. They are not a replacement for `mounts.resolve`, and you must not remove them.
- Map service errors to HTTP in `backend/internal/handler/errors.go`.

## Frontend

- State uses runes in `.svelte.ts` files. Do not add `svelte/store`.
- A method that an `$effect` calls must not read the state it changes. Wrap that read in `untrack`.
- The browse page is wired from `lib/browse/`: location, data, actions, uploads and selection.

## Verify

- Test the changed behaviour at the nearest real boundary. A build does not prove runtime behaviour.
- State each check that passed and each flow you did not test.

## Pull requests

- Explain the user problem first. Then explain the solution.
- Do not commit, push or merge without permission.
