//! `sikemux update`. Root puts a host's core in place, through the update
//! service the installer sets up.

#[cfg(target_os = "linux")]
pub mod root;
