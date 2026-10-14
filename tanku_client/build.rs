//! Refuses a release of the client that has its development features.
//!
//! Those features are on unless a build turns them off, and Cargo can't tie a feature to a
//! profile. With them, a release links the engine as a library of its own: it runs where it
//! was built and starts nowhere else. Cargo runs a build script early, so the mistake costs
//! little: seconds once a release was built before, and about half a minute of a build from
//! nothing, where most of the engine is still to be compiled.

fn main() {
    // Nothing here reads a file of the crate. A build with another profile or with other
    // features runs this script again by itself.
    println!("cargo::rerun-if-changed=build.rs");
    // Cargo tells the family of the profile: `release` for a release, for the page of a
    // browser and for a benchmark, and `debug` for everything else. The family is what
    // matters here, whatever the profile is set to.
    let release = std::env::var("PROFILE").is_ok_and(|profile| profile == "release");
    let development = std::env::var_os("CARGO_FEATURE_DEVELOPMENT").is_some();
    if release && development {
        // Cargo fails the build for these lines, and shows them as its own errors.
        println!(
            "cargo::error=a build in a release profile can't have the development features: \
             it would link the engine as a library of its own and start nowhere but here"
        );
        println!(
            "cargo::error=leave them out: \
             cargo build --release --no-default-features -p tanku_client"
        );
        println!("cargo::error=a release that still takes scripts adds: --features script");
    }
}
