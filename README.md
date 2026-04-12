# Self-Evolving Language Model

A production-ready, self-evolving language model that continuously learns from any files you add to `training_data/`.

## Features

- 🔍 **Automatic file watching** - Drop any file (PDF, DOCX, image, code, text) and it learns instantly
- 🧠 **RAG-powered chat** - Retrieves relevant context from learned knowledge
- 📦 **Vector storage** - Efficient embedding storage and retrieval
- 🔗 **Blockchain memory** - Immutable record of everything learned
- 🚀 **GitHub Codespaces** - Run in the cloud, always on
- 📊 **GitHub Pages dashboard** - Monitor learning progress
- ⚡ **GitHub Actions** - Automatic learning on push

## Quick Start

### Local Development

```bash
git clone https://github.com/your-username/self_evolving_lm
cd self_evolving_lm
cargo build --release

# Start the learning process
cargo run --release

# In another terminal, chat with the model
cargo run --bin chat --release