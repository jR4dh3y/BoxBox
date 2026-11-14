"""Select CI jobs from changed paths without suppressing required check results."""

import os
import subprocess

JOBS = ("backend", "frontend", "website", "desktop", "docker")
DETAILS = ("backend_perf", "frontend_build", "frontend_browser", "desktop_package")
SCOPES = JOBS + DETAILS
BACKEND = {"backend", "frontend", "desktop", "docker", *DETAILS}
FRONTEND = {"frontend", "desktop", "docker", "frontend_build", "frontend_browser", "desktop_package"}
DESKTOP = {"desktop", "desktop_package"}
BROWSER = {"frontend", "frontend_build", "frontend_browser"}
SHARED = {"bun.lock", "bun.lockb", "bunfig.toml", ".npmrc", ".bun-version"}
WORKFLOWS = {
    "ci.yml": set(SCOPES),
    "docker-nightly.yml": {"docker"},
    "docker-branch.yml": {"docker"},
    "docker-publish.yml": {"website", "docker", *DESKTOP},
    "appimages.yml": DESKTOP,
    "aur.yml": DESKTOP,
}


def affected_jobs(paths):
    jobs = set()
    for path in paths:
        parts = path.split("/")
        name = parts[-1]
        package_docs = len(parts) == 2 and parts[0] in {"backend", "frontend", "desktop", "website"} and name in {"README.md", "AGENTS.md"}
        if package_docs or (len(parts) == 1 and name.endswith(".md")) or path == "LICENSE":
            continue
        if path.startswith("docs/"):
            # The website enumerates only Markdown directly inside docs/.
            if len(parts) == 2 and name.endswith(".md"):
                jobs.add("website")
        elif path.startswith(".github/workflows/"):
            jobs.update(WORKFLOWS.get(name, SCOPES))
        elif path in {".github/scripts/ci_scope.py", ".github/scripts/test_ci_scope.py"}:
            jobs.update(SCOPES)
        elif path in {".github/scripts/check_release.py", ".github/scripts/test_check_release.py"}:
            # The changes job runs the release helper tests itself.
            continue
        elif path.startswith(".github/scripts/"):
            jobs.update(SCOPES)
        elif path == "package.json":
            jobs.update(DESKTOP)
        elif path in SHARED:
            jobs.update(FRONTEND | {"website"})
        elif name.endswith("_test.go") and path.startswith("backend/"):
            jobs.add("backend")
            if name.endswith("_bench_test.go") or name in {"perf_budget_test.go", "list_folder_test.go"}:
                jobs.add("backend_perf")
        elif path == "backend/internal/service/perf-budget.json":
            jobs.update({"backend", "backend_perf"})
        elif path.startswith("backend/"):
            jobs.update(BACKEND)
        elif name.endswith((".test.ts", ".test.svelte.ts", ".spec.ts", ".spec.svelte.ts")) or path.startswith("frontend/test/"):
            # Test files are not inputs to the shipped artifacts.
            if parts[0] in {"frontend", "desktop", "website"}:
                jobs.add(parts[0])
            else:
                jobs.update(SCOPES)
        elif path.startswith("frontend/scripts/perf/"):
            if name != "backend.ts":
                jobs.update(BROWSER)
            if name in {"backend.ts", "common.ts", "gates.ts"}:
                jobs.update({"backend", "backend_perf"})
        elif path.startswith("frontend/scripts/"):
            jobs.add("frontend")
            if name == "auth-session-browser.ts":
                jobs.update(BROWSER)
            elif name in {"bundle-budget.ts", "chunk-cycles.ts", "shell-scripts.ts"}:
                jobs.add("frontend_build")
        elif path.startswith("frontend/"):
            jobs.update(FRONTEND)
        elif path.startswith("website/"):
            jobs.add("website")
        elif path.startswith(".github/actions/setup-desktop/"):
            jobs.update(DESKTOP)
        elif path in {"desktop/scripts/tar-fixture.ts", "desktop/scripts/thumbnail-fixtures.ts"}:
            jobs.add("desktop")
        elif path.startswith("desktop/"):
            jobs.update(DESKTOP)
        elif path in {"Dockerfile", ".dockerignore"} or path.startswith("docker-compose"):
            jobs.add("docker")
        elif path.startswith("scripts/auth-smoke/") or path == "scripts/check-auth-sessions.ts":
            jobs.update(BROWSER)
        elif path == "scripts/build-single.sh":
            jobs.update(BROWSER | DESKTOP)
        elif path in {"scripts/package-release.ts", "scripts/check-release-package.ts", "scripts/package-appimage.ts", "scripts/prepare-aur.sh"}:
            jobs.update(DESKTOP)
        elif path.startswith("scripts/"):
            jobs.update(FRONTEND)
        elif path.startswith(".github/") or path.startswith(".oss-scanner/"):
            continue
        else:
            # Unknown inputs and unavailable diffs retain full validation.
            return set(SCOPES)
    return jobs


def changed_paths(base, head, pull_request=False):
    if not base or not head or set(base) == {"0"}:
        return None
    try:
        comparison = [f"{base}...{head}"] if pull_request else [base, head]
        result = subprocess.run(
            ["git", "diff", "--name-only", "--no-renames", "-z", *comparison, "--"],
            check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
    except subprocess.CalledProcessError:
        return None
    return result.stdout.decode("utf-8", errors="surrogateescape").split("\0")[:-1]


def main():
    paths = changed_paths(
        os.environ.get("BASE_SHA"), os.environ.get("HEAD_SHA"),
        os.environ.get("GITHUB_EVENT_NAME") == "pull_request",
    )
    jobs = set(SCOPES) if paths is None else affected_jobs(paths)
    outputs = "".join(f"{job}={str(job in jobs).lower()}\n" for job in SCOPES)
    print(outputs, end="")
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write(outputs)


if __name__ == "__main__":
    main()
