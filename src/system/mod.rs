// ======================================================================
// SYSTEM ACCESS MODULE - COMPLETE DEVICE CONTROL
// File: src/system/mod.rs
// Description: Full system access for Marisselle LM
//              Files, devices, USB, network, cameras, microphones,
//              Bluetooth, serial ports, and all connected peripherals
// ======================================================================

pub mod access;
pub mod network;
pub mod permission;
pub mod devices;
pub mod commands;

pub use access::SystemAccess;
pub use network::NetworkAccess;
pub use permission::{PermissionManager, Permission, PermissionLevel};
pub use devices::DeviceManager;
pub use commands::CommandExecutor;
