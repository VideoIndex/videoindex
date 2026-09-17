// Smoke test: create an index, add a generated clip with the M0 policy, list
// videos, search text-only, read a timeline. Needs ffmpeg and a built native
// module (`npm run build:debug`) plus the decode worker from the Rust build.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const root = path.resolve(__dirname, "..", "..", "..");
const worker = ["target/release/vi-media-worker", "target/debug/vi-media-worker", "target/release/vidx", "target/debug/vidx"]
  .map((p) => path.join(root, p))
  .find((p) => fs.existsSync(p));

test("index, add, videos, search, timeline, ask", { skip: !worker && "no vidx binary built" }, async () => {
  const { Index, version } = require("..");
  assert.match(version(), /^\d+\.\d+/);
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "vi-node-"));
  const clip = path.join(tmp, "clip.mp4");
  execFileSync("ffmpeg", ["-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=10:duration=12",
    "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=16000:duration=12", "-c:v", "libx264", "-preset", "ultrafast", "-g", "20",
    "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest", clip]);
  const configToml = `[media]\ncache_dir = "${tmp}/videos"\n[media.worker]\npath = "${worker}"\n`;
  const idx = Index.create(path.join(tmp, "t.vidx"), { configToml });
  const reports = await idx.add(clip, "m0");
  assert.equal(reports.length, 1);
  assert.equal(reports[0].ok, true);
  const videos = await idx.videos();
  assert.equal(videos.length, 1);
  assert.equal(typeof videos[0].duration, "number");
  const status = await idx.status();
  assert.equal(status.videos.length, 1);
  const res = await idx.search("anything", { textOnly: true });
  assert.ok(Array.isArray(res.hits));
  const tl = await idx.timeline(videos[0].id, "shot");
  assert.ok(Array.isArray(tl));
  // Reopen and ask with no LLM bound: the stream ends with an error/done, never hangs.
  const again = Index.open(path.join(tmp, "t.vidx"), { configToml });
  let sawDone = false;
  try {
    for await (const ev of again.ask("what?", { policy: "retrieval-only", maxToolCalls: 1 })) {
      if (ev.type === "done") sawDone = true;
    }
  } catch (e) {
    assert.match(String(e.message), /role|provider|agent_llm/i);
    sawDone = true;
  }
  assert.ok(sawDone);
});
