# v0.4.0 desktop launch validation

This follow-up is stacked on PR #182. It prepares the website, installation path and release automation; it does not publish v0.4.0 or merge the desktop PR.

| Surface | Result | Evidence |
|---|---|---|
| Landing page | PASS | Linux desktop section, actual rendered app preview, download entry point; 1440px and 390px Chromium |
| Download availability | PASS | Desktop link hidden for v0.3.1 server-only metadata and API failure; correct v0.4.0 asset link for matching metadata |
| Docs and navigation | PASS | Desktop and pairing guides reachable; long inline paths wrap on mobile without disabling code-block scrolling |
| Release API | PASS | Typed metadata parsing rejects invalid tags and off-origin assets; existing public API cache retained |
| Installer archive | PASS | Uses the included Bun runtime and embedded desktop/icon assets; installs into an isolated home with spaces |
| Update/default-handler behavior | PASS | Reinstall preserves user preferences; default file manager remains unchanged without opt-in |
| Packaged desktop runtime | PASS | Installed v0.4.0 stable archive launches real CEF and opens Settings on the Linux host |
| Server archives | PASS | Both checksums verified; ELF machine IDs match amd64 and arm64; ARM binary was not executed on an ARM device |
| Installed backend | PASS | Private bootstrap establishes its cookie; authenticated file API and desktop HTML respond |
| Release workflow | PASS static | Actionlint and seven Python tests; stable tags must match VERSION and belong to master; manual dispatch never publishes |
| Website checks | PASS | TypeScript, three metadata tests and production Astro build; six browser availability/size combinations, zero page errors |
| Desktop checks | PASS | TypeScript and 55 tests |
| Publication | NOT RUN | No public tag, GitHub release, Docker image publication or live website deployment performed locally |

The website's Astro checker currently rejects the repository's existing TypeScript 7 dependency. Validation uses TypeScript plus the production build and rendered browser checks; this change does not downgrade the website toolchain.

The stable release job builds on Ubuntu 24.04. The local CEF launch is Linux-host proof, not validation of every glibc distribution, clipboard provider or native application. Other desktop operating systems and architectures are explicitly unsupported; the ARM server artifact is separate.

Reproduce:

```sh
bun run check:desktop
bun run test:desktop
bun test website/src/lib/releases.test.ts
python3 -B -m unittest discover -s .github/scripts -p 'test_*.py'
cd website && bunx tsc --noEmit && bun run build
# From the repository root:
bun scripts/package-release.ts
bun scripts/check-release-package.ts
```

Release archives are generated under `dist/release/` and are ignored by Git. `release-manifest.json` records the source revision and whether the checkout was dirty. Official version-tag builds reject a dirty checkout.
