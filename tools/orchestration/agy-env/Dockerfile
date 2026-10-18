FROM debian:bookworm-slim

# Base tools + system libraries to build Tauri 2 (webkit, tray, keyring/dbus) and xcap (pipewire, gbm)
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl wget git jq less procps ripgrep openssh-client xz-utils file bc \
      python3 python3-pip python3-venv build-essential pkg-config \
      libssl-dev libdbus-1-dev libwebkit2gtk-4.1-dev libgtk-3-dev \
      libayatana-appindicator3-dev librsvg2-dev libxdo-dev \
      libpipewire-0.3-dev libgbm-dev libdrm-dev libwayland-dev libegl1-mesa-dev \
      libxcb1-dev libxrandr-dev libclang-dev clang \
    && rm -rf /var/lib/apt/lists/*

# Node 24 LTS (Vite 8 does not run on Debian's Node 18)
RUN set -eux; case "$(dpkg --print-architecture)" in amd64) a=x64;; arm64) a=arm64;; *) exit 1;; esac; \
    base=https://nodejs.org/dist/latest-v24.x; \
    f=$(curl -fsSL $base/SHASUMS256.txt | grep -o "node-v[0-9.]*-linux-$a.tar.xz" | head -1); \
    curl -fsSL $base/$f | tar -xJ -C /usr/local --strip-components=1; node --version

# Rust in /opt (outside HOME, which is a volume and would hide it). Root-owned: agy cannot change the
# toolchain (AGENTS.md §8). The cargo registry goes to $HOME/.cargo.
ENV RUSTUP_HOME=/opt/rust/rustup PATH=/opt/rust/cargo/bin:$PATH
RUN curl -fsSL https://sh.rustup.rs | CARGO_HOME=/opt/rust/cargo sh -s -- -y --no-modify-path \
      --profile minimal --component clippy,rustfmt \
    && chmod -R a+rX /opt/rust

# Compile lock shared between containers (volume "locks")
RUN mkdir -p /locks && chmod 1777 /locks

# Same UID as the host user so files written in the mounted workspace keep the right owner
ARG UID=1000
RUN useradd -m -u ${UID} -s /bin/bash agy
USER agy
ENV HOME=/home/agy CARGO_HOME=/home/agy/.cargo PATH=/home/agy/.local/bin:/opt/rust/cargo/bin:$PATH

# The agy binary lives in HOME (volume) so its self-update persists
RUN curl -fsSL https://antigravity.google/cli/install.sh | bash

CMD ["sleep", "infinity"]
