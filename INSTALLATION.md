# Installation Guide

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

#### MacPorts

```bash
sudo port install klean
```

Update:
```bash
sudo port selfupdate
sudo port upgrade klean
```

### Linux

#### Ubuntu/Debian

From PPA or Packagecloud:

```bash
# Add repository
curl -s https://packagecloud.io/install/repositories/danil0ws/klean/script.deb.sh | sudo bash

# Install
sudo apt-get install klean
```

Or download .deb directly:
```bash
wget https://github.com/danil0ws/klean/releases/download/latest/klean-*.deb
sudo dpkg -i klean-*.deb
```

#### Fedora/CentOS/RHEL

From Copr or Packagecloud:

```bash
# Add repository
curl -s https://packagecloud.io/install/repositories/danil0ws/klean/script.rpm.sh | sudo bash

# Install
sudo dnf install klean
```

Or download .rpm directly:
```bash
wget https://github.com/danil0ws/klean/releases/download/latest/klean-*.rpm
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

Pacman (from official repos if packaged):
```bash
sudo pacman -S klean
```

#### NixOS

```bash
nix-shell -p klean
# or
nix profile install github:danil0ws/klean
```

### Windows

#### Scoop

```powershell
scoop bucket add klean https://github.com/klean-cli/scoop-bucket
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
mise plugin add klean https://github.com/klean-cli/mise-klean
mise install klean@latest
```

With asdf:

```bash
asdf plugin add klean https://github.com/danil0ws/asdf-klean
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
