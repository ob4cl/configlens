"""Keep archived fuzz listing metadata neutral without exposing identifying values."""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parent.parent
LISTING = re.compile(r"^[d-][rwx-]{9}\s+\d+\s+(\S+)\s+(\S+)\s+\d+\s+")


class FuzzArtifactPrivacyTests(unittest.TestCase):
    def check_listing(self, relative):
        lines = (ROOT / relative).read_text(encoding="utf-8").splitlines()
        owners = [m.groups() for line in lines if (m := LISTING.match(line))]
        self.assertEqual(len(owners), 6, "expected the original six listing entries")
        # Do not include matched values in the failure message or assertion diff.
        self.assertTrue(all(pair == ("user", "group") for pair in owners),
                        "fuzz listing owner/group fields must be neutral placeholders")

    def test_first_fuzz_session_uses_neutral_listing_metadata(self):
        self.check_listing("fuzz/results/fuzz-session.log")

    def test_archived_session_uses_neutral_listing_metadata(self):
        self.check_listing("fuzz/results/session-1.log")


if __name__ == "__main__":
    unittest.main()
