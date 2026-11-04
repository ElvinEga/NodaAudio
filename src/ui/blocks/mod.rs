//! High-level composite blocks assembling NodaAudio widgets into complete layouts.
//!
//! Directly inspired by audiocn's composite block architecture (`components/blocks/`).
//! These blocks package real-world production configurations (microphones, music stations,
//! soundboards, mixing consoles, and popovers) into ready-to-use egui components.

pub mod mic_setup;
pub mod music_player;
pub mod quick_audio_popover;
pub mod soundboard;
pub mod system_audio_mixer;

pub use mic_setup::{MicCheckResult, MicSetupBlock};
pub use music_player::MusicPlayerBlock;
pub use quick_audio_popover::QuickAudioPopover;
pub use soundboard::SoundboardBlock;
pub use system_audio_mixer::SystemAudioMixerBlock;
