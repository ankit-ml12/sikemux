use sha2::{Digest, Sha256};

/// The helper is published beside each release instead of shipping in the app, which
/// downloads it when the simulator is first used. Only the exact helper built with the app
/// is accepted, so its size and hash are recorded here, for every app that links this crate.
fn main() {
    let binaries = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../binaries");
    println!("cargo:rerun-if-changed={}", binaries.display());
    let target = std::env::var("TARGET").unwrap_or_default();
    let candidates = [
        format!("sikemux-sim-{target}"),
        "sikemux-sim-universal-apple-darwin".to_owned(),
    ];
    for asset in candidates {
        let Ok(bytes) = std::fs::read(binaries.join(&asset)) else {
            continue;
        };
        println!("cargo:rustc-env=SIKEMUX_SIM_HELPER_ASSET={asset}");
        println!("cargo:rustc-env=SIKEMUX_SIM_HELPER_SIZE={}", bytes.len());
        println!(
            "cargo:rustc-env=SIKEMUX_SIM_HELPER_SHA256={}",
            hex::encode(Sha256::digest(&bytes))
        );
        return;
    }
}
