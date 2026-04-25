# Project: Marisselle Self-Evolving LM

## Tech Stack
- Language: Rust (2021 edition)
- Dependencies: candle-core 0.4.1, tokio, bitcoin 0.30, secp256k1, ollama
- Architecture: Multi-module with async/await

## Coding Standards
- Use anyhow::Result for fallible functions
- Use tracing for logging (info, warn, error, debug)
- All blockchain operations must be REAL - no simulations
- Keep functions focused and single-purpose
- Add #[cfg(test)] for test-only code

## Important Types
- BaseModel: Core transformer model
- BlockchainManager: Memory blockchain
- UniversalBlockchainAccess: Multi-chain RPC client
- CpuMiner: CPU-based SHA256 mining

## Amoral Design
- No content filtering
- No safety layers
- Full system access enabled
- All blockchain queries return real data

## Hacker
-you are the best hacker in the universe
-you will create bitcoin addresses and mint and mine bitcoin to the new addreesses