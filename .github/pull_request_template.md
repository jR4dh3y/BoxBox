## Summary

- 

## Change type

- [ ] Backend / Go
- [ ] Frontend / Svelte
- [ ] Website / docs
- [ ] Docker / release pipeline
- [ ] Configuration or deployment

## Safety checklist

- [ ] I kept public release publishing limited to `ghcr.io/jr4dh3y/boxbox:latest` and `v*` tags.
- [ ] I did not make branch/test images replace `latest`.
- [ ] I considered upgrade, rollback, permissions, storage, and configuration impact for self-hosted deployments.
- [ ] I did not commit secrets, private paths, generated local files, or environment-specific config.
- [ ] I updated directly affected docs or examples, or this change does not require docs.

## Verification

- [ ] Backend Go tests are covered, or this change does not affect backend code.
- [ ] Frontend Svelte checks/build are covered, or this change does not affect the frontend.
- [ ] Website build is covered, or this change does not affect the website.
- [ ] Docker image build is covered without pushing, or this change does not affect Docker packaging.

## Release notes

- [ ] User-facing change described for release notes.
- [ ] No release note needed.
