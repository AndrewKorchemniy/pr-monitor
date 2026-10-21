//! PR Monitor: a tiny, fast tray app that tracks your team's Azure DevOps pull requests.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod activation;
mod assets;
mod build_info;
mod cli;
mod copy;
mod logging;
mod net;
mod panic;
mod renderer;
mod report;
mod self_test;
mod shutdown;
mod startup;
mod ui_host;
mod ui_launch;
mod unregister;
mod wiring;

fn main() -> std::process::ExitCode {
    startup::run()
}

#[cfg(test)]
#[path = "../build/icon_gen.rs"]
mod icon_gen;

#[cfg(test)]
mod tests;
