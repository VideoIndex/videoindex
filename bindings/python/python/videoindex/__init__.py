"""VideoIndex: index long videos and query them.

The heavy lifting is in the Rust core (``videoindex._core``); this package
re-exports it and adds the extension points: ``Operator`` / ``@operator``
for indexing stages written in Python and ``Policy`` for agent strategies.
"""

from __future__ import annotations

from typing import Any, Callable, Iterable, Optional, Sequence

from ._core import (  # noqa: F401
    AskStream,
    Budget,
    Config,
    Index,
    Job,
    override_prompt,
    version,
)

__all__ = [
    "AskStream",
    "Budget",
    "Config",
    "Index",
    "Job",
    "Operator",
    "Policy",
    "operator",
    "override_prompt",
    "version",
    "prompts",
]
__version__ = version()

#: Item kinds an operator can consume or produce, as the core names them.
ITEM_KINDS = (
    "media", "frame", "hashed", "thumbnail", "audio_chunk", "speech_range",
    "transcript_span", "shot", "ocr_span", "image_embedding", "text_embedding",
    "scene", "chapter", "description", "extraction",
)

#: Row kinds a Python operator may return from ``run``/``finish``.
ROW_KINDS = ("transcript_span", "ocr_span", "shot", "scene", "chapter", "description")


class Operator:
    """An indexing stage implemented in Python.

    Subclass and set ``id``, ``inputs`` and ``outputs``; implement
    ``run(ctx, item)`` and optionally ``finish(ctx)``. Register with
    ``Index.register_operator`` and name the ``id`` in a policy's ``coarse``
    or ``fine`` list like a built-in operator.

    Every kind in ``inputs`` must be produced by another operator in the
    policy, except ``media`` (the root item every job starts with; list it
    to receive the video's metadata first) and kinds also named in
    ``optional_inputs``, which are delivered when some operator produces
    them and skipped otherwise.

    ``item`` is a dict with ``kind`` and the item's fields: frames and
    hashed frames carry ``frame`` (HxWx3 uint8 NumPy array), ``sample`` or
    ``sample_id`` and ``t``; speech ranges carry ``samples`` (int16 at
    ``sample_rate``); spans, segments and descriptions carry their row.
    ``ctx`` has ``job``, ``video_id``, ``stage``, ``sample_fps``, ``cost_usd``.

    ``run`` returns ``None``, one row dict or a list of row dicts. Rows have
    ``kind`` in ``ROW_KINDS`` plus the row's fields with times in seconds::

        {"kind": "transcript_span", "t0": 1.0, "t1": 2.5, "text": "..."}
        {"kind": "ocr_span", "frame_sample_id": item["sample_id"], "t": item["t"], "text": "..."}
        {"kind": "scene", "t0": 0.0, "t1": 30.0, "title": "Intro"}
        {"kind": "description", "target_kind": "frame", "target_id": item["sample_id"], "text": "..."}

    The core assigns ids and provenance, writes the rows and hands them to
    downstream operators. Each operator's outputs are cached under its
    ``id``, ``version`` and ``params`` like any other stage; bump ``version``
    when the output changes.
    """

    id: str = ""
    version: int = 1
    inputs: Sequence[str] = ()
    outputs: Sequence[str] = ()
    optional_inputs: Sequence[str] = ()
    params: Optional[dict[str, Any]] = None

    def run(self, ctx: dict[str, Any], item: dict[str, Any]) -> Any:  # pragma: no cover - interface
        raise NotImplementedError

    def __repr__(self) -> str:
        return f"<Operator {self.id} v{self.version} {list(self.inputs)} -> {list(self.outputs)}>"


class _FunctionOperator(Operator):
    def __init__(self, fn: Callable[[dict[str, Any], dict[str, Any]], Any], finish=None):
        self._fn = fn
        self._finish = finish
        if finish is not None:
            self.finish = finish  # type: ignore[assignment]

    def run(self, ctx: dict[str, Any], item: dict[str, Any]) -> Any:
        return self._fn(ctx, item)

    def __call__(self, ctx: dict[str, Any], item: dict[str, Any]) -> Any:
        return self._fn(ctx, item)


def operator(
    id: str,
    *,
    version: int = 1,
    inputs: Iterable[str],
    outputs: Iterable[str],
    optional_inputs: Iterable[str] = (),
    params: Optional[dict[str, Any]] = None,
    finish: Optional[Callable[[dict[str, Any]], Any]] = None,
) -> Callable[[Callable[[dict[str, Any], dict[str, Any]], Any]], Operator]:
    """Turn ``fn(ctx, item)`` into an ``Operator``::

        @vi.operator(id="brightness", inputs=["hashed"], outputs=["description"])
        def brightness(ctx, item):
            return {"kind": "description", "target_kind": "frame",
                    "target_id": item["sample_id"], "text": f"mean {item['frame'].mean():.0f}"}

        idx.register_operator(brightness)
        idx.add(path, policy={"coarse": ["sample", "phash", "thumbnail", "brightness"]})
    """

    def wrap(fn):
        op = _FunctionOperator(fn, finish)
        op.id = id
        op.version = version
        op.inputs = tuple(inputs)
        op.outputs = tuple(outputs)
        op.optional_inputs = tuple(optional_inputs)
        op.params = dict(params) if params else None
        op.__doc__ = fn.__doc__
        op.__name__ = getattr(fn, "__name__", id)  # type: ignore[attr-defined]
        return op

    return wrap


class Policy:
    """Decides the agent's next step; pass an instance as ``policy=`` to
    ``Index.ask``. ``next_step(state)`` sees ``question``, ``steps`` (each
    ``{"tool", "args", "result"}``) and ``tool_calls_left``. Return
    ``{"tool": name, "args": {...}}`` to call a tool (``search``,
    ``list_videos``, ``timeline``, ``get_transcript``, ``get_ocr``,
    ``get_descriptions``, ``view``, ``describe``) or ``None`` to have the
    LLM write the answer from the observations so far.
    """

    name: str = "python"

    def next_step(self, state: dict[str, Any]) -> Optional[dict[str, Any]]:  # pragma: no cover - interface
        raise NotImplementedError


class prompts:  # noqa: N801 - namespace, matches the design doc's `vi.prompts.override`
    """Prompt overrides for this process."""

    @staticmethod
    def override(name: str, text: str) -> None:
        override_prompt(name, text)
