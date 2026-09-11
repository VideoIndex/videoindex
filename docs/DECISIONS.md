# Decisions

One dated bullet per decision made where the design documents were silent or had to change. Newest at the bottom.

- 2026-09-11: `/data` is a plain directory on the root filesystem (164 G free), not a separate volume. Neither existing disk has the 500 G the deployment doc plans for; a dedicated data disk is deferred until benchmark media (M4) needs it. The ephemeral `/mnt` disk is never used for media or indexes.
- 2026-09-11: The M0 "under 5 minutes for a 1-hour MP4" target is measured on this machine's 4 vCPUs, not the 8 cores the docs assume. The number is recorded either way; if it misses on 4 cores the doc target is annotated rather than the machine upgraded.
- 2026-09-11: The M0 timing test uses a synthetic 1-hour 720p H.264 file (ffmpeg `testsrc2`, 30 fps, GOP 150) because no real hour-long video exists on the machine yet and YouTube is blocked. Real dataset videos re-run the measurement once transferred.
- 2026-09-11: yt-dlp is installed as the upstream standalone binary in `/usr/local/bin`, not the Ubuntu package (which is a year old and breaks against YouTube). It is only for `vi doctor` and the `YtDlp` acquirer on machines where YouTube is reachable.
- 2026-09-11: nginx already runs on this machine for other sites and port 8080 is taken by another app. The deployment doc's Caddy-on-80/443 and `vi serve` on 8080 will be adjusted at M3 (nginx site + a free localhost port); `11-deployment.md` is updated when that work happens, not now.
