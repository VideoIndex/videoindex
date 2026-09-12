"""Python operators and policies: a frame-level operator run by an inline
policy, an operator that fails, and a policy steering the agent against a
scripted OpenAI-compatible server."""
import json
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer

import pytest

import videoindex as vi

from test_smoke import make_clip, worker_env


def make_index(tmp_path, extra_toml=""):
    worker = worker_env()
    clip = str(tmp_path / "clip.mp4")
    make_clip(clip)
    cfg = vi.Config.from_toml(
        f"""
        [media]
        cache_dir = "{tmp_path / 'videos'}"
        [media.worker]
        path = "{worker}"
        {extra_toml}
        """
    )
    return vi.Index.create(str(tmp_path / "t.vidx"), config=cfg), clip


def test_function_operator_writes_descriptions(tmp_path):
    idx, clip = make_index(tmp_path)
    seen = {"frames": 0, "media": 0, "finish": 0}

    def done(ctx):
        seen["finish"] += 1
        return {"kind": "scene", "t0": 0.0, "t1": 20.0, "title": "whole clip"}

    @vi.operator(
        id="brightness",
        inputs=["hashed", "media"],
        optional_inputs=["shot"],
        outputs=["description", "scene"],
        finish=done,
    )
    def brightness(ctx, item):
        assert ctx["stage"] == "brightness" and ctx["video_id"]
        if item["kind"] == "media":
            seen["media"] += 1
            assert item["video"]["id"] == ctx["video_id"] and item["duration_secs"] > 0
            return None
        assert item["kind"] == "hashed"
        frame = item["frame"]
        assert frame.ndim == 3 and frame.shape[2] == 3 and frame.dtype.name == "uint8"
        seen["frames"] += 1
        return {
            "kind": "description",
            "target_kind": "frame",
            "target_id": item["sample_id"],
            "text": f"testsrc frame at {item['t']:.0f}s mean brightness {frame.mean():.0f}",
        }

    idx.register_operator(brightness)
    assert idx.operators() == ["brightness"]
    job = idx.add(clip, policy={"name": "py", "coarse": ["sample", "phash", "thumbnail", "brightness"], "fine": []})
    report = job.wait()
    assert report["error"] is None, report
    r = report["reports"][0]
    assert r["ok"], r
    assert seen["media"] == 1 and seen["frames"] > 0 and seen["finish"] == 1
    stage = r["stages"]["brightness"]
    assert stage["status"] == "complete" and stage["items_done"] == seen["frames"] + 1

    hits = idx.search("brightness", kinds=["description"], text_only=True)
    assert hits and hits[0]["evidence"][0]["kind"] == "description", hits[:1]
    video_id = idx.videos()[0]["id"]
    scenes = idx.timeline(video_id, level="scene")
    assert len(scenes) == 1 and scenes[0]["title"] == "whole clip"

    # Second run: the stage is cached and the Python function is not called.
    before = seen["frames"]
    report = idx.add(clip, policy={"coarse": ["sample", "phash", "thumbnail", "brightness"]}).wait()
    assert report["error"] is None
    assert seen["frames"] == before


def test_operator_errors_fail_the_job(tmp_path):
    idx, clip = make_index(tmp_path)

    class Broken(vi.Operator):
        id = "broken"
        inputs = ["hashed"]
        outputs = ["ocr_span"]

        def run(self, ctx, item):
            if item["kind"] == "hashed":
                raise RuntimeError("boom at %.1f" % item["t"])

    idx.register_operator(Broken())
    report = idx.add(clip, policy={"coarse": ["sample", "phash", "broken"]}).wait()
    assert report["error"] is None, report  # per-job errors are in the report
    r = report["reports"][0]
    assert not r["ok"]
    err = r["stages"]["broken"]["error"]
    assert "boom at" in err and "RuntimeError" in err, r["stages"]


def test_bad_declarations_are_rejected(tmp_path):
    idx, _ = make_index(tmp_path)
    with pytest.raises(ValueError, match="unknown item kind"):
        idx.register_operator(vi.operator(id="x", inputs=["Frame"], outputs=[])(lambda c, i: None))
    with pytest.raises(ValueError, match="id"):
        idx.register_operator(vi.operator(id="bad name", inputs=["hashed"], outputs=[])(lambda c, i: None))


class ScriptedOpenAI(BaseHTTPRequestHandler):
    """Answers every chat completion with a fixed streamed answer. The
    policy, not the model, picks the tools, so the server never needs to
    return tool calls."""

    def do_POST(self):  # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        self.rfile.read(n)
        body = (
            'data: {"choices":[{"delta":{"content":"The answer."},"finish_reason":"stop"}]}\n\n'
            'data: {"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":2}}\n\n'
            "data: [DONE]\n\n"
        )
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        self.wfile.write(body.encode())

    def log_message(self, *a):  # silence
        pass


@pytest.fixture
def fake_llm():
    srv = HTTPServer(("127.0.0.1", 0), ScriptedOpenAI)
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{srv.server_port}/v1"
    srv.shutdown()


def test_python_policy_drives_the_agent(tmp_path, fake_llm):
    idx, clip = make_index(
        tmp_path,
        f"""
        [providers.fake]
        adapter = "openai_compat"
        base_url = "{fake_llm}"
        model = "fake"
        [roles]
        agent_llm = {{ provider = "fake" }}
        """,
    )
    assert idx.add(clip, policy="m0").wait()["reports"][0]["ok"]

    class TwoSearches(vi.Policy):
        name = "two_searches"

        def __init__(self):
            self.states = []

        def next_step(self, state):
            self.states.append(state)
            if len(state["steps"]) < 2:
                return {"tool": "search", "args": {"query": f"probe {len(state['steps'])}", "k": 3}}
            return None

    policy = TwoSearches()
    events = list(idx.ask("what is shown?", policy=policy))
    kinds = [e["type"] for e in events]
    assert kinds.count("tool_call") == 2, kinds
    assert all(e["tool"] == "search" for e in events if e["type"] == "tool_call")
    assert kinds[-1] == "done"
    assert "".join(e["text"] for e in events if e["type"] == "token") == "The answer."
    assert len(policy.states) == 3
    assert policy.states[-1]["steps"][0]["tool"] == "search"
    assert policy.states[-1]["tool_calls_left"] < policy.states[0]["tool_calls_left"]

    with pytest.raises(TypeError, match="next_step"):
        idx.ask("q", policy=object())
