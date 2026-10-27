# OSS Scanner environment

This directory contains BoxBox's build environment and threat model for Anthropic's [OSS Scanner](https://github.com/anthropics/oss-scanner).

The scanner image retains the source checkout, Go and Bun toolchains, installed dependencies, test files, and a server binary with debug information. The production image remains separate.

## Check the project image

From the repository root:

```sh
docker build -f .oss-scanner/Dockerfile -t boxbox-oss-scanner .
docker run --rm --network none --cpus 2 --memory 8g boxbox-oss-scanner \
  bash -c 'set -e; cd /src/frontend; bun run test; cd /src/backend; go test ./...'
```

`Dockerfile.dockerignore` overrides the production build's ignore rules so the scanner receives tests and documentation. Local dependencies, generated outputs, and environment files are excluded.

## Enrollment configuration

BoxBox's [enrollment submission](https://github.com/anthropics/oss-scanner/pull/1056) is managed in Anthropic's repository. After these files are published on BoxBox's `master` branch, its `project.yaml` can reference:

```yaml
dockerfile: .oss-scanner/Dockerfile
threat_model: .oss-scanner/threat_model.md
```

That change must also remove the corresponding copies beside `project.yaml`. Each file must be provided in exactly one place.

The project-image check above does not install Anthropic's additional scanner tools. Run `tools/validate.py` and `tools/check boxbox --no-shell` from the enrollment repository to check that full environment.
