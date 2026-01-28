//! Lightweight syslog codec facade used by sinks.
//!
//! This module provides the syslog encoder for formatting outgoing syslog messages.
//! Syslog parsing (for incoming messages) is handled by the preprocessing hook
//! in `sources::syslog::udp_source::build_preproc_hook`.

mod encoder;

pub use encoder::{EmitMessage, SyslogEncoder};
