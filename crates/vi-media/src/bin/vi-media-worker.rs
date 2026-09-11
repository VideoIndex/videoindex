//! Standalone decode worker. The `vi` binary embeds the same entry point
//! behind the hidden `__vi_media_worker` argument; this binary exists for
//! tests and for hosts that prefer a separate executable.
fn main() {
    std::process::exit(vi_media::worker::main());
}
