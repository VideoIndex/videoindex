"""Benchmark loaders. Every loader yields `Question` objects with the same shape."""
from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class Question:
    """One multiple-choice question over one video."""

    id: str
    benchmark: str
    video_key: str  # YouTube id
    question: str  # stem without options
    options: list[str]  # texts in letter order
    answer: str  # letter
    task_types: list[str] = field(default_factory=list)
    time_reference: str | None = None  # e.g. "00:15-00:19" when the benchmark gives one
    video_type: str | None = None
    extra: dict = field(default_factory=dict)

    @property
    def letters(self) -> list[str]:
        return [chr(ord("A") + i) for i in range(len(self.options))]

    def prompt(self, tools: bool = True) -> str:
        """The multiple-choice prompt. `tools=False` for configurations where
        the model cannot call tools (retrieval-only, uniform baseline): telling
        it to use tools makes it write pseudo tool calls instead of answering."""
        opts = "\n".join(f"({l}) {o}" for l, o in zip(self.letters, self.options))
        how = (
            "Use the tools to find the evidence in this video, reason briefly, then end"
            if tools
            else "Reason briefly from the evidence you have been given about this video (do not ask for more), then end"
        )
        return (
            f"{self.question}\n\nOptions:\n{opts}\n\n"
            f"{how} with a line of the form 'Answer: X' where X is one of {', '.join(self.letters)}. "
            "If the evidence does not settle it, pick the most likely option; always give an answer line."
        )


def load(benchmark: str, root: str):
    if benchmark == "lvbench":
        from .lvbench import load as f
    elif benchmark == "minerva":
        from .minerva import load as f
    elif benchmark in ("onehour", "1h-videoqa", "onehour_videoqa"):
        from .onehour_videoqa import load as f
    else:
        raise SystemExit(f"unknown benchmark {benchmark}")
    return f(root)
