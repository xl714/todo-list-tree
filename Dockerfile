# Utilisation de la version "slim"
FROM node:22-bookworm-slim

# 1. Installation des outils système
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

# 3. Installation des outils Rust/Wasm
# On installe trunk, wasm-bindgen ET le target wasm32 dans la même foulée
RUN cargo install trunk wasm-bindgen-cli && \
    rustup target add wasm32-unknown-unknown

# 4. Installation d'OpenClaw
RUN npm install -g openclaw@latest && \
    npm cache clean --force

# 5. AJOUT DES ALIAS BASH (Version nettoyée)
RUN echo 'alias oc="openclaw"' >> /root/.bashrc && \
    echo 'alias oct="openclaw tui"' >> /root/.bashrc && \
    echo 'alias ocs="openclaw gateway"' >> /root/.bashrc && \
    echo 'alias ocf="openclaw gateway --force"' >> /root/.bashrc && \
    echo 'alias c="cargo"' >> /root/.bashrc && \
    echo 'alias cc="cargo clean"' >> /root/.bashrc && \
    echo 'alias h="htop"' >> /root/.bashrc && \
    echo 'alias tree="tree -I '\''target|.git|node_modules'\''"' >> /root/.bashrc && \
    echo 'alias ts="cd /app/project/v3 && trunk serve --address 0.0.0.0 --port 8080"' >> /root/.bashrc && \
    echo 'alias ll="ls -alFh --group-directories-first"' >> /root/.bashrc && \
    echo 'alias usage="du -sh /app/project/* | sort -h"' >> /root/.bashrc

# 6. CONFIGURATION OPENCLAW
# On crée le dossier caché ET on fait le lien symbolique
RUN mkdir -p /root/.openclaw && \
    ln -sf /app/project/.openclaw/openclaw.json /root/.openclaw/openclaw.json

# --- Persistance de la configuration de l'agent ---

# 1. Créer l'arborescence dans le conteneur
RUN mkdir -p /root/.openclaw/agents/main/agent/

# 2. Créer les liens symboliques vers ton projet monté
# Cela force l'agent à lire les fichiers qui sont dans ton repo .git
RUN ln -sf /app/project/.openclaw/agents/main/agent/auth-profiles.json /root/.openclaw/agents/main/agent/auth-profiles.json && \
    ln -sf /app/project/.openclaw/agents/main/agent/models.json /root/.openclaw/agents/main/agent/models.json

WORKDIR /app/project

# Ports : Gateway (18789) et Trunk (8080)
EXPOSE 18789 8080

CMD ["tail", "-f", "/dev/null"]