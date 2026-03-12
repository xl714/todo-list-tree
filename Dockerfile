# Utilisation de la version "slim"
FROM node:22-bookworm-slim

# 1. Installation des outils de confort (psmisc pour fuser, procps pour ps/top)
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    git build-essential curl ca-certificates \
    psmisc procps tree htop nano \
    pkg-config libssl-dev && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# 2. Installation de Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# 3. Installation des outils Rust/Wasm (Trunk est indispensable pour Leptos)
RUN cargo install trunk wasm-bindgen-cli

# 4. Installation d'OpenClaw
RUN npm install -g openclaw@latest && \
    npm cache clean --force

# 5. AJOUT DES ALIAS BASH
# On écrit directement les alias dans le .bashrc du root
RUN echo 'alias oc="openclaw"' >> /root/.bashrc && \
    echo 'alias oct="openclaw tui"' >> /root/.bashrc && \
    echo 'alias ocs="openclaw gateway"' >> /root/.bashrc && \
    echo 'alias ocf="openclaw gateway --force"' >> /root/.bashrc && \
    echo 'alias c="cargo"' >> /root/.bashrc && \
    echo 'alias cc="cargo clean"' >> /root/.bashrc && \
    echo 'alias ts="trunk serve"' >> /root/.bashrc && \
    echo 'alias ll="ls -alF"' >> /root/.bashrc && \
    echo 'alias tree="tree -I '\''target|.git|node_modules'\''"' >> /root/.bashrc

RUN echo 'll() { ls -alFh --group-directories-first "${1:-.}"; }' >> /root/.bashrc
RUN echo 'alias usage="du -sh /app/project/* | sort -h"' >> /root/.bashrc

WORKDIR /app/project

EXPOSE 18789

CMD ["tail", "-f", "/dev/null"]