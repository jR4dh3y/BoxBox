"""Keep stable releases on tested master commits and match their version metadata."""
import os
import re
import subprocess
from pathlib import Path


def validate_version(tag, version):
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?", tag):
        raise ValueError("Invalid release tag")
    if tag != f"v{version}":
        raise ValueError("Release tag does not match VERSION")


def main():
    tag = os.environ["GITHUB_REF_NAME"]
    validate_version(tag, Path("VERSION").read_text().strip())
    if "-" not in tag:
        subprocess.run(["git", "fetch", "origin", "master:refs/remotes/origin/master"], check=True)
        subprocess.run(["git", "merge-base", "--is-ancestor", "HEAD", "origin/master"], check=True)


if __name__ == "__main__":
    main()
