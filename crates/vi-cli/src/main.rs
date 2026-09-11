//! The `vi` binary. Filled in at step 5 of M0.
fn main() -> anyhow::Result<()> {
    println!("vi {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
