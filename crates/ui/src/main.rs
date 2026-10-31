#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod assets;
mod chrome;
mod shell;
mod state;
mod theme;

fn main() -> std::process::ExitCode {
    let (store, state, notice) = state::Store::load();
    shell::run(store, state, notice)
}
