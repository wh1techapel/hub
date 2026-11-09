// No console window behind Hub in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    hub_lib::run();
}
