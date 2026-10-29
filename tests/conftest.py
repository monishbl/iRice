"""Shared test setup."""

import os
import tempfile

# Tests never write into the real session: the Quick Share debug log (and
# anything else keyed to $XDG_RUNTIME_DIR) goes to a throwaway directory, and
# a debug flag the user set for a real phone test doesn't turn tests' traffic
# into lines in their log.
_runtime = tempfile.mkdtemp(prefix="irice-tests-run-")
os.environ["XDG_RUNTIME_DIR"] = _runtime
os.environ.pop("IRICE_QS_DEBUG", None)
