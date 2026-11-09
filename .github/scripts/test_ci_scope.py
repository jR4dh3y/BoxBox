import os
import subprocess
from contextlib import chdir
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from ci_scope import BACKEND, BROWSER, DESKTOP, FRONTEND, SCOPES, affected_jobs, changed_paths, main


class CIScopeTest(unittest.TestCase):
    def test_documentation_only_skips_builds(self):
        self.assertEqual(affected_jobs(["README.md", "AGENTS.md", "frontend/README.md"]), set())

    def test_dependencies_select_the_consumers(self):
        cases = {
            "backend/internal/service/file.go": BACKEND,
            "backend/go.sum": BACKEND,
            "backend/internal/service/file_test.go": {"backend"},
            "backend/internal/service/file_bench_test.go": {"backend", "backend_perf"},
            "backend/internal/service/perf-budget.json": {"backend", "backend_perf"},
            "frontend/src/lib/components/FileList.svelte": FRONTEND,
            "frontend/src/lib/stores/jobs.test.ts": {"frontend"},
            "frontend/src/lib/stores/websocket.test.svelte.ts": {"frontend"},
            "frontend/test/preload.ts": {"frontend"},
            "frontend/scripts/perf/backend.ts": {"backend", "backend_perf"},
            "frontend/scripts/perf/common.ts": BROWSER | {"backend", "backend_perf"},
            "frontend/scripts/perf/desktop-connectivity.ts": BROWSER,
            "frontend/scripts/perf/fixtures/slow-jobs.go": BROWSER,
            "frontend/scripts/perf/gates.test.ts": {"frontend"},
            "frontend/scripts/bundle-budget.ts": {"frontend", "frontend_build"},
            "website/src/pages/index.astro": {"website"},
            "docs/index.md": {"website"},
            "docs/api.md": {"website"},
            "docs/evidence/test-pruning/comparison.md": set(),
            "docs/releases/v0.3.2.md": set(),
            "desktop/src/app.ts": DESKTOP,
            "desktop/src/launch.test.ts": {"desktop"},
            "desktop/scripts/tar-fixture.ts": {"desktop"},
            "package.json": DESKTOP,
            "frontend/static/README.md": FRONTEND,
            "frontend/static/example.spec.json": FRONTEND,
            "bun.lock": FRONTEND | {"website"},
            "Dockerfile": {"docker"},
            ".github/workflows/ci.yml": set(SCOPES),
            ".github/workflows/docker-publish.yml": DESKTOP | {"website", "docker"},
            ".github/workflows/docker-nightly.yml": {"docker"},
            ".github/workflows/appimages.yml": DESKTOP,
            ".github/workflows/aur.yml": DESKTOP,
            "VERSION": set(SCOPES),
            ".github/actions/setup-desktop/action.yml": DESKTOP,
            ".github/scripts/ci_scope.py": set(SCOPES),
            ".github/scripts/check_release.py": set(),
            ".github/scripts/new-builder.py": set(SCOPES),
            ".github/scripts/test_check_release.py": set(),
            "scripts/package-release.ts": DESKTOP,
            "scripts/build-single.sh": BROWSER | DESKTOP,
            "scripts/auth-smoke/process.ts": BROWSER,
            "new-build-config": set(SCOPES),
            "frontend/static/help.md": FRONTEND,
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                self.assertEqual(affected_jobs([path]), expected)

    def test_mixed_changes_union_dependencies(self):
        self.assertEqual(affected_jobs(["README.md", "website/src/a.astro", "desktop/src/app.ts"]), {"website", *DESKTOP})

    def test_unavailable_diff_fails_open(self):
        self.assertIsNone(changed_paths("0" * 40, "head"))
        with patch("subprocess.run", side_effect=subprocess.CalledProcessError(1, "git")):
            self.assertIsNone(changed_paths("base", "head"))

    def test_output_includes_false_build_flags_for_test_only_change(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            with patch.dict(os.environ, {"GITHUB_OUTPUT": str(output)}), patch(
                "ci_scope.changed_paths", return_value=["backend/internal/service/file_test.go", "desktop/src/launch.test.ts"]
            ):
                main()
            values = dict(line.split("=") for line in output.read_text().splitlines())
            self.assertEqual(set(values), set(SCOPES))
            self.assertEqual({key for key, value in values.items() if value == "true"}, {"backend", "desktop"})
            self.assertEqual(set(values.values()), {"true", "false"})

    def test_unavailable_diff_outputs_every_check(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            with patch.dict(os.environ, {"GITHUB_OUTPUT": str(output)}), patch("ci_scope.changed_paths", return_value=None):
                main()
            self.assertEqual(set(output.read_text().splitlines()), {f"{key}=true" for key in SCOPES})

    def test_real_git_rename_across_projects_checks_both(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def git(*args):
                return subprocess.check_output(["git", "-C", directory, *args], stderr=subprocess.DEVNULL).decode().strip()
            git("init")
            git("config", "user.email", "ci-test@example.invalid")
            git("config", "user.name", "CI test")
            (root / "backend").mkdir()
            (root / "backend/file.go").write_text("package fixture\n")
            git("add", ".")
            git("commit", "-m", "base")
            base = git("rev-parse", "HEAD")
            (root / "website").mkdir()
            git("mv", "backend/file.go", "website/file.go")
            git("commit", "-m", "move")
            head = git("rev-parse", "HEAD")
            with chdir(directory):
                paths = changed_paths(base, head)
            self.assertEqual(set(paths), {"backend/file.go", "website/file.go"})
            self.assertEqual(affected_jobs(paths), set(SCOPES))

            # Advancing the target branch must not add its unrelated edits to a PR's scope.
            git("checkout", "-b", "target", base)
            (root / "docs").mkdir()
            (root / "docs/api.md").write_text("target-only documentation\n")
            git("add", ".")
            git("commit", "-m", "advance target")
            target = git("rev-parse", "HEAD")
            with chdir(directory):
                pr_paths = changed_paths(target, head, pull_request=True)
                push_paths = changed_paths(target, head)
            self.assertEqual(set(pr_paths), {"backend/file.go", "website/file.go"})
            self.assertIn("docs/api.md", push_paths)


if __name__ == "__main__":
    unittest.main()
