// Windows would otherwise show a console window behind the app. Harmless on
// macOS and Linux, which is where djmanzo actually targets.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    dj_app::run();
}
