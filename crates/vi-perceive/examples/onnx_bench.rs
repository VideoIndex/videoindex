//! Time the local ONNX models on one device: `onnx_bench <models-dir> <cpu|cuda> <image.png> [iterations]`.
//! Reports load time and per-call latency for SigLIP image/text embedding,
//! bge-small text embedding and RapidOCR on the given image.
#![allow(clippy::unwrap_used)]
use std::path::Path;
use std::time::Instant;

use vi_perceive::bge::TextEmbedder;
use vi_perceive::ocr::{OcrConfig, RapidOcr};
use vi_perceive::onnx::resolve_device;
use vi_perceive::siglip::Siglip;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = Path::new(&args[1]);
    let device = resolve_device(&args[2]).unwrap();
    let img = image::open(&args[3]).unwrap().to_rgb8();
    let iters: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(20);
    let (w, h) = img.dimensions();
    let rgb = img.as_raw();
    let stride = w as usize * 3;
    println!("device {device:?}  image {w}x{h}  iterations {iters}");

    let t = Instant::now();
    let siglip = Siglip::load(&dir.join("siglip-base-patch16-224"), device, 0).unwrap();
    println!("siglip load {:.2}s", t.elapsed().as_secs_f64());
    for batch in [1usize, 8] {
        let imgs: Vec<(&[u8], u32, u32, usize)> =
            (0..batch).map(|_| (&rgb[..], w, h, stride)).collect();
        siglip.embed_images(&imgs).unwrap();
        let t = Instant::now();
        for _ in 0..iters {
            siglip.embed_images(&imgs).unwrap();
        }
        println!(
            "siglip image batch {batch}: {:.1} ms/call",
            t.elapsed().as_secs_f64() * 1000.0 / iters as f64
        );
    }
    let texts = vec!["a slide with a bar chart".to_string()];
    siglip.embed_text(&texts).unwrap();
    let t = Instant::now();
    for _ in 0..iters {
        siglip.embed_text(&texts).unwrap();
    }
    println!(
        "siglip text: {:.1} ms/call",
        t.elapsed().as_secs_f64() * 1000.0 / iters as f64
    );

    let t = Instant::now();
    let bge = TextEmbedder::load(&dir.join("bge-small-en-v1.5"), device, 0).unwrap();
    println!("bge load {:.2}s", t.elapsed().as_secs_f64());
    let spans: Vec<String> = (0..32)
        .map(|i| {
            format!("span {i}: the lecturer explains how gradient descent updates the weights")
        })
        .collect();
    bge.embed(&spans).unwrap();
    let t = Instant::now();
    for _ in 0..iters {
        bge.embed(&spans).unwrap();
    }
    println!(
        "bge batch 32: {:.1} ms/call",
        t.elapsed().as_secs_f64() * 1000.0 / iters as f64
    );

    let t = Instant::now();
    let ocr = RapidOcr::load(&dir.join("rapidocr"), device, 0, OcrConfig::default()).unwrap();
    println!("ocr load {:.2}s", t.elapsed().as_secs_f64());
    let lines = ocr.read(rgb, w, h, stride).unwrap();
    let t = Instant::now();
    for _ in 0..iters {
        ocr.read(rgb, w, h, stride).unwrap();
    }
    println!(
        "ocr read ({} lines): {:.1} ms/call",
        lines.len(),
        t.elapsed().as_secs_f64() * 1000.0 / iters as f64
    );
}
