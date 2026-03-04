// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `--waybar` mode: probe all providers and output waybar-compatible JSON to stdout.
    // Intended for use with the waybar `custom/script` module on Linux/Hyprland.
    //
    // Example waybar config:
    //   "custom/openusage": {
    //       "exec": "openusage --waybar",
    //       "interval": 300,
    //       "return-type": "json"
    //   }
    if std::env::args().any(|a| a == "--waybar") {
        openusage_lib::waybar::run();
        return;
    }

    openusage_lib::run();
}
