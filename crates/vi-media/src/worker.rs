//! The decode worker process. Reads [`Request`]s on stdin, writes
//! [`Response`]s on stdout, puts pixels and PCM into shared-memory slots.

use std::collections::VecDeque;
use std::io::{BufReader, BufWriter, Write};
use std::sync::mpsc;

use crate::decode;
use crate::error::{MediaError, Result};
use crate::protocol::{read_msg, write_msg, Request, Response, PROTOCOL_VERSION};
use crate::shm::{SharedRegionMut, ShmSpec};

/// Run the worker over the process's stdin/stdout until `Shutdown` or EOF.
/// Returns the process exit code.
pub fn main() -> i32 {
    match run() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("vi-media-worker: {e}");
            1
        }
    }
}

fn run() -> Result<()> {
    decode::init()?;
    let (tx, rx) = mpsc::channel::<Request>();
    std::thread::Builder::new()
        .name("vi-media-worker-stdin".into())
        .spawn(move || {
            let stdin = std::io::stdin();
            let mut reader = BufReader::new(stdin.lock());
            loop {
                match read_msg::<_, Request>(&mut reader) {
                    Ok(Some(req)) => {
                        if tx.send(req).is_err() {
                            break;
                        }
                    }
                    Ok(None) | Err(_) => {
                        let _ = tx.send(Request::Shutdown);
                        break;
                    }
                }
            }
        })?;

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    while let Ok(req) = rx.recv() {
        match req {
            Request::Hello { version } => {
                if version != PROTOCOL_VERSION {
                    write_msg(
                        &mut out,
                        &Response::Error {
                            message: format!(
                                "protocol version mismatch: parent {version}, worker {PROTOCOL_VERSION}"
                            ),
                        },
                    )?;
                    break;
                }
                write_msg(
                    &mut out,
                    &Response::Hello {
                        version: PROTOCOL_VERSION,
                        libav: decode::libav_version(),
                        pid: std::process::id(),
                    },
                )?;
            }
            Request::Probe { path } => {
                let resp = match crate::probe::probe_file(&path) {
                    Ok(p) => Response::Probe(p),
                    Err(e) => Response::Error {
                        message: e.to_string(),
                    },
                };
                write_msg(&mut out, &resp)?;
            }
            Request::DecodeVideo { req, shm } => {
                let hint = None;
                let result = with_session(&mut out, &rx, &shm, |shm, sink| {
                    decode::decode_video(&req, hint, shm, sink)
                });
                finish_session(&mut out, result)?;
            }
            Request::DecodeAudio { req, shm } => {
                let result = with_session(&mut out, &rx, &shm, |shm, sink| {
                    decode::decode_audio(&req, shm, sink)
                });
                finish_session(&mut out, result)?;
            }
            Request::SlotFree { .. } | Request::Cancel => {
                // Outside a session these are stale; ignore.
            }
            Request::Shutdown => break,
        }
        out.flush()?;
    }
    Ok(())
}

fn with_session<W: Write>(
    out: &mut W,
    rx: &mpsc::Receiver<Request>,
    shm: &ShmSpec,
    f: impl FnOnce(&mut SharedRegionMut, &mut dyn decode::Sink) -> Result<(u64, u64)>,
) -> Result<(u64, u64)> {
    let mut region = SharedRegionMut::open(shm)?;
    let mut sink = StdSink {
        out,
        rx,
        free: (0..shm.slots).collect(),
    };
    f(&mut region, &mut sink)
}

fn finish_session<W: Write>(out: &mut W, result: Result<(u64, u64)>) -> Result<()> {
    match result {
        Ok((items, decoded)) => write_msg(out, &Response::End { items, decoded }),
        Err(MediaError::Cancelled) => write_msg(
            out,
            &Response::Error {
                message: "cancelled".into(),
            },
        ),
        Err(e) => write_msg(
            out,
            &Response::Error {
                message: e.to_string(),
            },
        ),
    }
}

struct StdSink<'a, W: Write> {
    out: &'a mut W,
    rx: &'a mpsc::Receiver<Request>,
    free: VecDeque<u32>,
}

impl<W: Write> StdSink<'_, W> {
    fn handle(&mut self, req: Request) -> Result<()> {
        match req {
            Request::SlotFree { slot } => {
                self.free.push_back(slot);
                Ok(())
            }
            Request::Cancel | Request::Shutdown => Err(MediaError::Cancelled),
            other => Err(MediaError::Protocol(format!(
                "unexpected request during decode session: {other:?}"
            ))),
        }
    }
}

impl<W: Write> decode::Sink for StdSink<'_, W> {
    fn send(&mut self, resp: &Response) -> Result<()> {
        write_msg(self.out, resp)
    }

    fn acquire_slot(&mut self) -> Result<u32> {
        loop {
            self.poll_cancel()?;
            if let Some(s) = self.free.pop_front() {
                return Ok(s);
            }
            match self.rx.recv() {
                Ok(req) => self.handle(req)?,
                Err(_) => return Err(MediaError::Cancelled),
            }
        }
    }

    fn poll_cancel(&mut self) -> Result<()> {
        loop {
            match self.rx.try_recv() {
                Ok(req) => self.handle(req)?,
                Err(mpsc::TryRecvError::Empty) => return Ok(()),
                Err(mpsc::TryRecvError::Disconnected) => return Err(MediaError::Cancelled),
            }
        }
    }
}
