import subprocess
from contextlib import chdir
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from ci_scope import JOBS, affected_jobs, changed_paths


class CIScopeTest(unittest.TestCase):
    def test_documentation_only_skips_builds(self):
        self.assertEqual(affected_jobs(["README.md", "AGENTS.md", "frontend/README.md"]), set())

    def test_dependencies_select_the_consumers(self):
        cases = {
            "backend/internal/service/file.go": {"backend", "frontend", "desktop", "docker"},
            "backend/go.sum": {"backend", "frontend", "desktop", "docker"},
            "frontend/src/lib/components/FileList.svelte": {"frontend", "desktop", "docker"},
            "frontend/scripts/perf/backend.ts": {"backend", "frontend", "desktop", "docker"},
            "website/src/pages/index.astro": {"website"},
            "docs/index.md": {"website"},
            "docs/api.md": {"website"},
            "desktop/src/app.ts": {"desktop"},
            "bun.lock": {"frontend", "website", "desktop", "docker"},
            "Dockerfile": {"docker"},
            ".github/workflows/ci.yml": set(JOBS),
            ".github/workflows/docker-publish.yml": set(JOBS),
            "VERSION": set(JOBS),
            ".github/actions/setup-desktop/action.yml": {"desktop"},
            ".github/scripts/ci_scope.py": set(JOBS),
            "new-build-config": set(JOBS),
            "frontend/static/help.md": {"frontend", "desktop", "docker"},
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                self.assertEqual(affected_jobs([path]), expected)

    def test_mixed_changes_union_dependencies(self):
        self.assertEqual(affected_jobs(["README.md", "website/src/a.astro", "desktop/src/app.ts"]), {"website", "desktop"})

    def test_unavailable_diff_fails_open(self):
        self.assertIsNone(changed_paths("0" * 40, "head"))
        with patch("subprocess.run", side_effect=subprocess.CalledProcessError(1, "git")):
            self.assertIsNone(changed_paths("base", "head"))

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
            self.assertEqual(affected_jobs(paths), set(JOBS))


if __name__ == "__main__":
    unittest.main()
