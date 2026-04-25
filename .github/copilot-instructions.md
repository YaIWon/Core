# Marisselle LM - Rust Project

## Key Patterns
- Use anyhow::Result for error handling
- Use tracing for logging
- All blockchain code connects to REAL RPC endpoints
- CPU mining uses SHA256 with difficulty adjustment

## Important Crates
- candle-core 0.4.1 (tensor operations)
- bitcoin 0.30 (OP_RETURN, addresses)
- tokio (async runtime)
