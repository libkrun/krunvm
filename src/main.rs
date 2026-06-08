// Copyright 2021 Red Hat, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::env;
#[cfg(target_os = "macos")]
use std::ffi::CString;
#[cfg(target_os = "macos")]
use std::fs::File;
#[cfg(target_os = "macos")]
use std::io::{self, Error, ErrorKind, Read, Write};
#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use crate::commands::{
    ChangeVmCmd, ConfigCmd, CreateCmd, DeleteCmd, InspectCmd, ListCmd, StartCmd,
};
use clap::{Parser, Subcommand, ValueEnum};
#[cfg(target_os = "macos")]
use nix::unistd::execve;
use serde_derive::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use text_io::read;

#[allow(unused)]
mod bindings;
mod commands;
mod utils;

const APP_NAME: &str = "krunvm";

/// Network mode used when starting a microVM.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum NetworkMode {
    /// Use libkrun's default TSI networking.
    #[default]
    Default,
    /// Disable all implicit networking.
    None,
}

impl std::fmt::Display for NetworkMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match self {
            NetworkMode::Default => "default",
            NetworkMode::None => "none",
        };
        write!(f, "{}", mode)
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct VmConfig {
    name: String,
    cpus: u32,
    mem: u32,
    container: String,
    workdir: String,
    dns: String,
    #[serde(default)]
    network: NetworkMode,
    mapped_volumes: HashMap<String, String>,
    mapped_ports: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KrunvmConfig {
    #[serde(default = "default_config_version")]
    version: u8,
    default_cpus: u32,
    default_mem: u32,
    default_dns: String,
    storage_volume: String,
    vmconfig_map: HashMap<String, VmConfig>,
}

impl Default for KrunvmConfig {
    fn default() -> KrunvmConfig {
        KrunvmConfig {
            version: 1,
            default_cpus: 2,
            default_mem: 1024,
            default_dns: "1.1.1.1".to_string(),
            storage_volume: String::new(),
            vmconfig_map: HashMap::new(),
        }
    }
}

/// Return the current config file format version.
fn default_config_version() -> u8 {
    1
}

/// Load the krunvm configuration or exit with a useful diagnostic.
fn load_config() -> KrunvmConfig {
    let config_path = config_file_path();

    match confy::load(APP_NAME) {
        Ok(cfg) => {
            warn_if_config_version_missing(config_path.as_deref());
            cfg
        }
        Err(err) => {
            print_config_load_error(config_path.as_deref(), &err);
            std::process::exit(1);
        }
    }
}

/// Store the krunvm configuration or exit with a useful diagnostic.
pub fn store_config(cfg: &KrunvmConfig) {
    if let Err(err) = confy::store(APP_NAME, cfg) {
        print_config_store_error(config_file_path().as_deref(), &err);
        std::process::exit(1);
    }
}

/// Return the config path used by confy for supported krunvm platforms.
fn config_file_path() -> Option<PathBuf> {
    config_dir().map(|dir| {
        dir.join(config_project_path())
            .join(format!("{}.toml", APP_NAME))
    })
}

/// Return the project path used by confy for supported krunvm platforms.
fn config_project_path() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "rs.krunvm"
    }

    #[cfg(not(target_os = "macos"))]
    {
        APP_NAME
    }
}

/// Return the config directory used by confy for supported krunvm platforms.
fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        if let Some(dir) = env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
            if dir.is_absolute() {
                return Some(dir);
            }
        }

        home_dir().map(|home| home.join(".config"))
    }

    #[cfg(target_os = "macos")]
    {
        home_dir().map(|home| home.join("Library/Preferences"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Return the current user's home directory from the environment.
fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// Warn when a legacy config file is missing the top-level config version.
fn warn_if_config_version_missing(config_path: Option<&Path>) {
    let Some(config_path) = config_path else {
        return;
    };

    let Ok(config_data) = std::fs::read_to_string(config_path) else {
        return;
    };

    if top_level_key_exists(&config_data, "version") {
        return;
    }

    eprintln!(
        "Warning: {} is missing a top-level `version` field; assuming `version = 1`.",
        config_path.display()
    );
    eprintln!(
        "To remove this warning, add `version = 1` before any TOML table headers in that file."
    );
}

/// Return whether a top-level TOML key appears before any table header.
fn top_level_key_exists(config_data: &str, key: &str) -> bool {
    for line in config_data.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') {
            break;
        }

        if let Some(rest) = trimmed.strip_prefix(key) {
            if rest.trim_start().starts_with('=') {
                return true;
            }
        }
    }

    false
}

/// Print a config loading error with repair hints.
fn print_config_load_error(config_path: Option<&Path>, err: &confy::ConfyError) {
    eprintln!("Error: failed to load krunvm configuration.");

    if let Some(config_path) = config_path {
        eprintln!("Configuration file: {}", config_path.display());
    }

    eprintln!("Reason: {}", err);

    if err.to_string().contains("missing field `version`") {
        eprintln!("Hint: add `version = 1` as a top-level entry before any TOML table headers.");
    } else {
        eprintln!(
            "Hint: fix the TOML in the configuration file, or move it aside and run krunvm again to create a default config."
        );
    }
}

/// Print a config storing error with repair hints.
fn print_config_store_error(config_path: Option<&Path>, err: &confy::ConfyError) {
    eprintln!("Error: failed to write krunvm configuration.");

    if let Some(config_path) = config_path {
        eprintln!("Configuration file: {}", config_path.display());
    }

    let reason = match err {
        confy::ConfyError::SerializeTomlError(err) => {
            format!("Failed to serialize configuration data into TOML: {}", err)
        }
        _ => err.to_string(),
    };
    eprintln!("Reason: {}", reason);

    if reason.contains("values must be emitted before tables") {
        eprintln!(
            "Hint: krunvm could not serialize the current config layout; please report this as a bug."
        );
    } else {
        eprintln!("Hint: check that the configuration directory exists and is writable.");
    }
}

#[cfg(target_os = "macos")]
fn check_case_sensitivity(volume: &str) -> Result<bool, io::Error> {
    let first_path = format!("{}/krunvm_test", volume);
    let second_path = format!("{}/krunVM_test", volume);
    {
        let mut first = File::create(&first_path)?;
        first.write_all(b"first")?;
    }
    {
        let mut second = File::create(&second_path)?;
        second.write_all(b"second")?;
    }
    let mut data = String::new();
    {
        let mut test = File::open(&first_path)?;

        test.read_to_string(&mut data)?;
    }
    if data == "first" {
        let _ = std::fs::remove_file(first_path);
        let _ = std::fs::remove_file(second_path);
        Ok(true)
    } else {
        let _ = std::fs::remove_file(first_path);
        Ok(false)
    }
}

#[cfg(target_os = "macos")]
fn check_volume(cfg: &mut KrunvmConfig) {
    if !cfg.storage_volume.is_empty() {
        return;
    }

    println!(
        "
On macOS, krunvm requires a dedicated, case-sensitive volume.
You can easily create such volume by executing something like
this on another terminal:

diskutil apfs addVolume disk3 \"Case-sensitive APFS\" krunvm

NOTE: APFS volume creation is a non-destructive action that
doesn't require a dedicated disk nor \"sudo\" privileges. The
new volume will share the disk space with the main container
volume.
"
    );
    loop {
        print!("Please enter the mountpoint for this volume [/Volumes/krunvm]: ");
        io::stdout().flush().unwrap();
        let answer: String = read!("{}\n");

        let volume = if answer.is_empty() {
            "/Volumes/krunvm".to_string()
        } else {
            answer.to_string()
        };

        print!("Checking volume... ");
        match check_case_sensitivity(&volume) {
            Ok(res) => {
                if res {
                    println!("success.");
                    println!("The volume has been configured. Please execute krunvm again");
                    cfg.storage_volume = volume;
                    store_config(cfg);
                    std::process::exit(-1);
                } else {
                    println!("failed.");
                    println!("This volume failed the case sensitivity test.");
                }
            }
            Err(err) => {
                println!("error.");
                println!("There was an error running the test: {}", err);
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn check_unshare() {
    let uid = unsafe { libc::getuid() };
    if uid != 0 && !std::env::vars().any(|(key, _)| key == "BUILDAH_ISOLATION") {
        println!("Please re-run krunvm inside a \"buildah unshare\" session");
        std::process::exit(-1);
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Sets the level of verbosity
    #[arg(short)]
    verbosity: Option<u8>, //TODO: implement or remove this
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Start(StartCmd),
    Create(CreateCmd),
    Inspect(InspectCmd),
    List(ListCmd),
    Delete(DeleteCmd),
    #[command(name = "changevm")]
    ChangeVm(ChangeVmCmd),
    Config(ConfigCmd),
}

#[cfg(target_os = "macos")]
fn get_brew_prefix() -> Option<String> {
    let output = std::process::Command::new("brew")
        .arg("--prefix")
        .stderr(std::process::Stdio::inherit())
        .output()
        .ok()?;

    let exit_code = output.status.code().unwrap_or(-1);
    if exit_code != 0 {
        return None;
    }

    Some(std::str::from_utf8(&output.stdout).ok()?.trim().to_string())
}

#[cfg(target_os = "macos")]
fn reexec() -> Result<(), Error> {
    let exec_path = env::current_exe().map_err(|_| ErrorKind::NotFound)?;
    let exec_cstr = CString::new(exec_path.to_str().ok_or(ErrorKind::InvalidFilename)?)?;

    let args: Vec<CString> = env::args_os()
        .map(|arg| CString::new(arg.into_vec()).unwrap())
        .collect();

    let mut envs: Vec<CString> = env::vars_os()
        .map(|(key, value)| {
            CString::new(format!(
                "{}={}",
                key.into_string().unwrap(),
                value.into_string().unwrap()
            ))
            .unwrap()
        })
        .collect();
    let brew_prefix = get_brew_prefix().ok_or(ErrorKind::NotFound)?;
    envs.push(CString::new(format!(
        "DYLD_LIBRARY_PATH={brew_prefix}/lib"
    ))?);

    // Use execve to replace the current process. This function only returns
    // if an error occurs.
    match execve(&exec_cstr, &args, &envs) {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Error re-executing krunvm: {}", e);
            std::process::exit(-1);
        }
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    {
        if env::var("DYLD_LIBRARY_PATH").is_err() {
            _ = reexec();
        }
    }

    let mut cfg = load_config();
    let cli_args = Cli::parse();

    #[cfg(target_os = "macos")]
    check_volume(&mut cfg);
    #[cfg(target_os = "linux")]
    check_unshare();

    match cli_args.command {
        Command::Inspect(cmd) => cmd.run(&mut cfg),
        Command::Start(cmd) => cmd.run(&cfg),
        Command::Create(cmd) => cmd.run(&mut cfg),
        Command::List(cmd) => cmd.run(&cfg),
        Command::Delete(cmd) => cmd.run(&mut cfg),
        Command::ChangeVm(cmd) => cmd.run(&mut cfg),
        Command::Config(cmd) => cmd.run(&mut cfg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_vm_config_with_network_and_mapping_tables() {
        let mut cfg = KrunvmConfig::default();
        cfg.vmconfig_map.insert(
            "ubuntu".to_string(),
            VmConfig {
                name: "ubuntu".to_string(),
                cpus: 2,
                mem: 1024,
                container: "ubuntu-container".to_string(),
                workdir: String::new(),
                dns: "1.1.1.1".to_string(),
                network: NetworkMode::None,
                mapped_volumes: HashMap::new(),
                mapped_ports: HashMap::new(),
            },
        );

        let path =
            env::temp_dir().join(format!("krunvm-serialize-test-{}.toml", std::process::id()));

        confy::store_path(&path, &cfg).unwrap();
        let config_data = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(path);

        assert!(config_data.contains("network = 'none'"));
        assert!(config_data.contains("[vmconfig_map.ubuntu.mapped_volumes]"));
        assert!(config_data.contains("[vmconfig_map.ubuntu.mapped_ports]"));
    }
}
