# Installation Guide

## One-liner (no package manager)

Linux/macOS — downloads the release tarball for your OS/arch, checks it against
the release `SHA256SUMS` and installs to `~/.local/bin`:

```bash
curl -fsSL https://raw.githubusercontent.com/danil0ws/klean/main/scripts/install.sh | sh
# options: --version v1.1.0  --prefix /usr/local/bin  --dry-run
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/danil0ws/klean/main/scripts/install.ps1 | iex
# options: -Version v1.1.0  -Prefix C:\Tools\klean  -DryRun
```

## Package Manager Installation Methods

### macOS

#### Homebrew (Recommended)

```bash
brew tap danil0ws/klean
brew install klean
```

Update:
```bash
brew upgrade klean
```

#### Other macOS package managers

MacPorts does not carry klean. Build it from source (see below) or download the
binary directly from the releases page.

### Linux

#### Ubuntu/Debian

From Packagecloud (published by `linux-packages.yml`):

```bash
# Add repository
curl -s https://packagecloud.io/install/repositories/danil0ws/klean/script.deb.sh | sudo bash

# Install
sudo apt-get install klean
```

Or download the `.deb` from a release:

```bash
VERSION=v1.0.0
curl -LO "https://github.com/danil0ws/klean/releases/download/${VERSION}/klean_${VERSION#v}-1_amd64.deb"
sudo dpkg -i klean_*.deb
```

#### Fedora/CentOS/RHEL

From Packagecloud:

```bash
# Add repository
curl -s https://packagecloud.io/install/repositories/danil0ws/klean/script.rpm.sh | sudo bash

# Install
sudo dnf install klean
```

Or download the `.rpm` from a release:

```bash
VERSION=v1.0.0
curl -LO "https://github.com/danil0ws/klean/releases/download/${VERSION}/klean-${VERSION#v}-1.x86_64.rpm"
sudo rpm -ivh klean-*.rpm
```

#### Arch Linux

From AUR (Arch User Repository):

```bash
# Using yay
yay -S klean

# Using makepkg
git clone https://aur.archlinux.org/klean.git
cd klean
makepkg -si
```

#### NixOS

klean is not in nixpkgs yet; use `cargo install klean` or build from source.

### Windows

#### Scoop

```powershell
scoop bucket add klean https://github.com/danil0ws/scoop-bucket
scoop install klean
```

Update:
```powershell
scoop update klean
```

#### Chocolatey

```powershell
choco install klean
```

Update:
```powershell
choco upgrade klean
```

#### Windows Package Manager (winget)

```powershell
winget install klean.klean
```

Update:
```powershell
winget upgrade klean.klean
```

### Cross-Platform

#### Mise / asdf-vm

Install Mise plugin:

```bash
mise plugin add klean https://github.com/danil0ws/mise-klean
mise install klean@latest
```

With asdf:

```bash
asdf plugin add klean https://github.com/danil0ws/mise-klean
asdf install klean latest
asdf global klean latest
```

#### Cargo (from crates.io)

```bash
cargo install klean
```

Update:
```bash
cargo install klean --force
```

### From Source

#### Build and install

```bash
# Clone repository
git clone https://github.com/danil0ws/klean.git
cd klean

# Build release
cargo build --release

# Install to PATH
sudo cp target/release/klean /usr/local/bin/

# Or install via cargo
cargo install --path .
```

### Direct Download

Download pre-built binaries from [GitHub Releases](https://github.com/danil0ws/klean/releases):

#### macOS

```bash
# Intel/x86_64
wget https://github.com/danil0ws/klean/releases/download/v1.0.0/klean-v1.0.0-x86_64-apple-darwin.tar.gz
tar xzf klean-v1.0.0-x86_64-apple-darwin.tar.gz
sudo mv klean /usr/local/bin/

# Apple Silicon/ARM64
wget https://github.com/danil0ws/klean/releases/download/v1.0.0/klean-v1.0.0-aarch64-apple-darwin.tar.gz
tar xzf klean-v1.0.0-aarch64-apple-darwin.tar.gz
sudo mv klean /usr/local/bin/
```

#### Linux

```bash
# x86_64
wget https://github.com/danil0ws/klean/releases/download/v1.0.0/klean-v1.0.0-x86_64-unknown-linux-musl.tar.gz
tar xzf klean-v1.0.0-x86_64-unknown-linux-musl.tar.gz
sudo mv klean /usr/local/bin/

# ARM64
wget https://github.com/danil0ws/klean/releases/download/v1.0.0/klean-v1.0.0-aarch64-unknown-linux-musl.tar.gz
tar xzf klean-v1.0.0-aarch64-unknown-linux-musl.tar.gz
sudo mv klean /usr/local/bin/
```

#### Windows

```powershell
# Download
Invoke-WebRequest -Uri "https://github.com/danil0ws/klean/releases/download/v1.0.0/klean-v1.0.0-x86_64-pc-windows-msvc.zip" -OutFile "klean.zip"

# Extract
Expand-Archive -Path "klean.zip" -DestinationPath "."

# Add to PATH or move to a directory in PATH
Move-Item -Path "klean.exe" -Destination "C:\Users\<Username>\AppData\Local\Programs\klean\"
```

## Verification

Verify installation:

```bash
klean --version
klean --help
```

## Docker

Build and run with Docker:

```bash
docker run -v /path/to/project:/project ghcr.io/danil0ws/klean:latest klean --path /project --dry-run
```

Or build locally:

```bash
docker build -t klean .
docker run -v $(pwd):/project klean klean --path /project --dry-run
```

## Troubleshooting

### Command not found

If you installed manually, ensure the directory containing `klean` is in your `$PATH`:

```bash
# Add to PATH
export PATH=$PATH:/path/to/klean/directory

# Permanently add to ~/.bashrc or ~/.zshrc
echo 'export PATH=$PATH:/path/to/klean/directory' >> ~/.bashrc
```

### Permission denied

Ensure the binary has execute permissions:

```bash
chmod +x /usr/local/bin/klean
```

### Build from source fails

Install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Then try building again:

```bash
cargo build --release
```

## System Requirements

- **Memory**: 512 MB minimum (2 GB recommended)
- **Disk space**: 50 MB for installation
- **Rust version** (if building): 1.70+

## Supported Platforms

| Platform | Architecture | Status |
|----------|--------------|--------|
| Linux | x86_64 | ✅ Supported |
| Linux | ARM64 | ✅ Supported |
| macOS | x86_64 | ✅ Supported |
| macOS | ARM64 (Apple Silicon) | ✅ Supported |
| Windows | x86_64 | ✅ Supported |

## Next Steps

- Read the [README](README.md) for usage instructions
- Check [CONTRIBUTING](CONTRIBUTING.md) to contribute
- Report issues on [GitHub Issues](https://github.com/danil0ws/klean/issues)
