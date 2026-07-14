//! Native Win32 presentation foundation for the PE client.
//!
//! P1 deliberately does not switch the running PE workflow away from its existing eframe entry.
//! The modules here establish the same Inno Setup 6.7 Modern Windows 11 colour, metric and native
//! control boundary used by the desktop client, so later PE parts can migrate one complete screen
//! at a time without changing disk or imaging behaviour.

pub mod controls;
pub mod details;
pub mod layout;
pub mod progress;
pub mod state;
pub mod theme;
pub mod window;
