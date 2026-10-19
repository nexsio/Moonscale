#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tailscale_stream_lib::run();
}