use std::path::{Path, PathBuf};
use std::{env, fs, io, process};

const COMMANDS: [&str; 7] = [
    "krunvm",
    "krunvm-changevm",
    "krunvm-create",
    "krunvm-config",
    "krunvm-delete",
    "krunvm-list",
    "krunvm-start",
];
const LIBKRUN_LIB_DIR_ENV: &str = "LIBKRUN_LIB_DIR";

fn main() {
    let outdir = match env::var_os("OUT_DIR") {
        Some(outdir) => outdir,
        None => {
            panic!("OUT_DIR environment variable not defined.");
        }
    };
    fs::create_dir_all(&outdir).unwrap();

    for command in COMMANDS {
        if let Err(err) = generate_man_page(&outdir, command) {
            panic!("failed to generate man page: {}", err);
        }
    }

    configure_libkrun_linking();
}

/// Configure native linker search paths for libkrun.
fn configure_libkrun_linking() {
    println!("cargo:rerun-if-env-changed={}", LIBKRUN_LIB_DIR_ENV);

    if let Some(dir) = env::var_os(LIBKRUN_LIB_DIR_ENV) {
        println!(
            "cargo:rustc-link-search=native={}",
            PathBuf::from(dir).display()
        );
        return;
    }

    for dir in candidate_libkrun_dirs() {
        if dir.join(libkrun_library_name()).exists() {
            println!("cargo:rustc-link-search=native={}", dir.display());
            return;
        }
    }

    println!(
        "cargo:warning=unable to find libkrun in common library directories; set {} to the directory containing {}",
        LIBKRUN_LIB_DIR_ENV,
        libkrun_library_name()
    );
}

/// Return common system library directories where libkrun may be installed.
fn candidate_libkrun_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "linux")]
    {
        if let Some(multiarch) = linux_multiarch_dir() {
            push_unique(
                &mut dirs,
                PathBuf::from(format!("/usr/local/lib/{multiarch}")),
            );
            push_unique(&mut dirs, PathBuf::from(format!("/usr/lib/{multiarch}")));
        }

        push_unique(&mut dirs, PathBuf::from("/usr/local/lib64"));
        push_unique(&mut dirs, PathBuf::from("/usr/local/lib"));
        push_unique(&mut dirs, PathBuf::from("/usr/lib64"));
        push_unique(&mut dirs, PathBuf::from("/usr/lib"));
    }

    #[cfg(target_os = "macos")]
    {
        push_unique(&mut dirs, PathBuf::from("/opt/homebrew/lib"));
        push_unique(&mut dirs, PathBuf::from("/usr/local/lib"));
    }

    dirs
}

/// Return the Debian-style multiarch library directory for the target.
#[cfg(target_os = "linux")]
fn linux_multiarch_dir() -> Option<&'static str> {
    match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => Some("x86_64-linux-gnu"),
        Ok("aarch64") => Some("aarch64-linux-gnu"),
        _ => None,
    }
}

/// Return the platform-specific libkrun shared library filename.
fn libkrun_library_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "libkrun.dylib"
    }

    #[cfg(not(target_os = "macos"))]
    {
        "libkrun.so"
    }
}

/// Add a path once while preserving search order.
fn push_unique(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if !dirs.contains(&dir) {
        dirs.push(dir);
    }
}

fn generate_man_page<P: AsRef<Path>>(outdir: P, command: &str) -> io::Result<()> {
    // If asciidoctor isn't installed, fallback to asciidoc.
    if let Err(err) = process::Command::new("asciidoctor").output() {
        eprintln!("Error from running 'asciidoctor': {}", err);
        return Err(err);
    }

    let outdir = outdir.as_ref();
    let outfile = outdir.join(format!("{}.1", command));
    let cwd = env::current_dir()?;
    let txt_path = cwd.join("docs").join(format!("{}.1.txt", command));

    let result = process::Command::new("asciidoctor")
        .arg("--doctype")
        .arg("manpage")
        .arg("--backend")
        .arg("manpage")
        .arg("--out-file")
        .arg(&outfile)
        .arg(&txt_path)
        .spawn()?
        .wait()?;
    if !result.success() {
        let msg = format!("'asciidoctor' failed with exit code {:?}", result.code());
        return Err(io::Error::other(msg));
    }
    Ok(())
}
