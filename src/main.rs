//! Standalone host: run the demo plugin as a desktop app, no DAW needed.
//!
//! Built only with the `standalone` feature (see `[[bin]]` in Cargo.toml);
//! `cargo truce run` compiles and launches it for you.

use noda_audio::Plugin;

fn main() {
    truce_standalone::run::<Plugin>();
}
