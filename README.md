# krunvm

```krunvm``` is a CLI-based utility for creating microVMs from OCI images, using [libkrun](https://github.com/containers/libkrun) and [buildah](https://github.com/containers/buildah).

## Features

* Minimal footprint
* Fast boot time
* Zero disk image maintenance
* Zero network configuration
* Support for mapping host volumes into the guest
* Support for exposing guest ports to the host

## Demo

[![asciicast](https://asciinema.org/a/CGtTS93VsdzWwUfkY1kqVnaik.svg)](https://asciinema.org/a/CGtTS93VsdzWwUfkY1kqVnaik)

## Supported platforms

- Linux/KVM on x86_64.
- Linux/KVM on AArch64.
- macOS/Hypervisor.framework on ARM64.

## Installation

### macOS

```
brew tap slp/krun
brew install krunvm
```

### Fedora

```
dnf copr enable -y slp/libkrunfw
dnf copr enable -y slp/libkrun
dnf copr enable -y slp/krunvm
dnf install -y krunvm
```

### Building from sources

The build generates man pages from the files in `docs/`, so `asciidoctor` must be installed and available in `PATH`.

#### Build-time dependencies

* Rust stable toolchain, including `cargo`
* [libkrun](https://github.com/containers/libkrun) headers and libraries
* [asciidoctor](https://github.com/asciidoctor/asciidoctor)
* A C toolchain and the platform libraries required by `libkrun`

[buildah](https://github.com/containers/buildah) is required at runtime to create and manage VMs, but it is not linked into the `krunvm` binary.

On macOS, install the Homebrew build dependencies with:

```sh
brew tap slp/krun
brew install asciidoctor libkrun
```

On Debian or Ubuntu systems, the CI setup builds `libkrun` from source after installing its package dependencies:

```sh
sudo apt-get update
sudo apt-get install -y \
    asciidoctor \
    libvirglrenderer-dev \
    libepoxy-dev \
    libdrm-dev \
    libpipewire-0.3-dev \
    libclang-dev
curl -OL https://github.com/containers/libkrun/archive/refs/tags/v1.17.0.tar.gz
tar xf v1.17.0.tar.gz
make -C libkrun-1.17.0
sudo make -C libkrun-1.17.0 PREFIX=/usr install
```

#### Building

Use the Makefile for normal local builds:

```sh
make
```

This creates a release binary at `target/release/krunvm`.
On macOS, the Makefile also signs the release binary with `krunvm.entitlements`.

For a debug build:

```sh
make debug
```

You can also build directly with Cargo:

```sh
cargo build --release
```

If you build directly with Cargo on macOS, sign the resulting binary before running it.

#### Installing

Install the release binary with:

```sh
make install PREFIX=/usr/local
```

Use `DESTDIR` when staging an install for packaging:

```sh
make install PREFIX=/usr DESTDIR=/tmp/krunvm-root
```

#### Checking changes

Run the same Clippy check used by CI with:

```sh
cargo clippy --locked -- -D warnings
```
