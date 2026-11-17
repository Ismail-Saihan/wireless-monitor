// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Wireless Virtual Monitor Entry Point
//! Supports bidirectional orchestration: Linux Wayland Host & Windows Direct3D11 Receiver.

fn main() {
    wireless_monitor_lib::run();
}
