"""Select CI jobs from changed paths without suppressing required check results."""

import os
import subprocess

JOBS = ("backend", "frontend", "website", "desktop", "docker")
DOCS = {
    "README.md", "AGENTS.md", "SECURITY.md", "CONTRIBUTING.md", "LICENSE",
    "backend/README.md", "frontend/README.md", "website/README.md", "desktop/README.md",
}
SHARED = {"package.json", "bun.lock", "bun.lockb", "bunfig.toml", ".npmrc", ".bun-version"}


def affected_jobs(paths):
    jobs = set()
    for path in paths:
        if path in DOCS:
            continue
        if path.startswith("docs/"):
            jobs.add("website")
            continue
        if path.startswith(".github/workflows/") or path.startswith(".github/scripts/"):
            return set(JOBS)
        if path in SHARED:
            jobs.update(("frontend", "website", "desktop", "docker"))
        elif path.startswith("backend/"):
            # Browser checks build and exercise the embedded Go server.
            jobs.update(("backend", "frontend", "desktop", "docker"))
        elif path.startswith("frontend/"):
            jobs.update(("frontend", "desktop", "docker"))
            if path.startswith("frontend/scripts/perf/"):
                jobs.add("backend")
        elif path.startswith("website/"):
            jobs.add("website")
        elif path.startswith(".github/actions/setup-desktop/"):
            jobs.add("desktop")
        elif path.startswith("desktop/"):
            jobs.add("desktop")
        elif path in {"Dockerfile", ".dockerignore"} or path.startswith("docker-compose"):
            jobs.add("docker")
        elif path.startswith("scripts/"):
            jobs.update(("frontend", "desktop", "docker"))
        elif path.startswith(".github/") or path.startswith(".oss-scanner/"):
            continue
        else:
            # An unclassified build input must not silently bypass validation.
            return set(JOBS)
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
    jobs = set(JOBS) if paths is None else affected_jobs(paths)
    outputs = "".join(f"{job}={str(job in jobs).lower()}\n" for job in JOBS)
    print(outputs, end="")
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write(outputs)


if __name__ == "__main__":
    main()
