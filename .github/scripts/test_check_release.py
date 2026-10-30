import unittest
from check_release import validate_version

class ReleaseTests(unittest.TestCase):
    def test_version_and_tag_match(self):
        validate_version("v0.3.2", "0.3.2")
        validate_version("v0.3.2-rc.1", "0.3.2-rc.1")

    def test_wrong_or_unsafe_tag_is_rejected(self):
        for tag in ["v0.3.1", "master", "v0.3.2/path", "v0.3.2;echo bad"]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                validate_version(tag, "0.3.2")
