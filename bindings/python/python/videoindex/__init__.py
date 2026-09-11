"""VideoIndex: index long videos and query them.

The heavy lifting is in the Rust core (``videoindex._core``); this package
re-exports it and adds small conveniences.
"""

from ._core import (  # noqa: F401
    AskStream,
    Budget,
    Config,
    Index,
    Job,
    override_prompt,
    version,
)

__all__ = ["AskStream", "Budget", "Config", "Index", "Job", "override_prompt", "version", "prompts"]
__version__ = version()


class prompts:  # noqa: N801 - namespace, matches the design doc's `vi.prompts.override`
    """Prompt overrides for this process."""

    @staticmethod
    def override(name: str, text: str) -> None:
        override_prompt(name, text)
