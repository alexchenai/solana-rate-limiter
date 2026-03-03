# Solana On-Chain Rate Limiter

A production-quality Solana Anchor program that implements an API rate limiting system on-chain. This demonstrates how a common Web2 backend pattern (rate limiting) translates to Solana's account model.

Built for the Superteam "Rebuild Backend Systems as On-Chain Rust Programs" challenge.

## Architecture

### How This Works in Web2

In a traditional backend:
- A database (Redis) stores counters per client per time window
- Middleware checks the counter before processing requests
- Counter increments on each request
- Counter resets when the time window expires
- Admin manages API keys with associated rate limit tiers

```
Request → Middleware → Check Redis(client:window) → Allow/Deny → Increment
```

### How This Works on Solana

On Solana, we use accounts as the state store:

- **Registry account** (PDA): Global registry of all API keys (1 per program)
- **ApiKey account** (PDA derived from [b"apikey", owner_pubkey]): Stores key config + rate limit tier
- **RateLimitState account** (PDA derived from [b"ratelimit", api_key_pubkey, window_start_bytes]): Per-window request counter

Every "request" in the rate limiter is a transaction that:
1. Derives the PDA for the current time window (floor(unix_timestamp / window_size))
2. Checks if the counter is below the tier limit
3. Increments the counter
4. Returns success or RateLimitExceeded error

The account model maps directly:
- Redis key (`user:api:window`) → PDA derivation path
- TTL/expiry → window_start seed (old windows become orphaned, can be garbage collected)
- Rate limit tiers → ApiKey account field

### Tradeoffs and Constraints

| Aspect | Web2 (Redis) | Solana On-Chain |
|---|---|---|
| Latency | <1ms (same datacenter) | ~400ms (transaction confirmation) |
| Cost | Memory + CPU | ~0.00001 SOL per check |
| Atomicity | Redis INCR is atomic | Solana transaction is atomic |
| TTL cleanup | Automatic (Redis TTL) | Manual (close old window accounts) |
| Distributed | Requires Redis cluster | Inherently distributed |
| Audit trail | Needs extra logging | Every transaction is on-chain log |
| Tampering | Possible by DB admin | Impossible (program ownership) |

### When On-Chain Rate Limiting Makes Sense

- Payments that should be rate-limited (prevent double-spend patterns)
- DAO governance (1 vote per member per proposal)
- NFT minting limits (1 per wallet per drop)
- DeFi: prevent sandwich attacks via per-block rate limits
- Any scenario where audit trail and tamper-proof enforcement matters

## Program Structure

```
solana-rate-limiter/
  programs/rate-limiter/src/
    lib.rs          - Program entrypoint and instructions
    state.rs        - Account state definitions
    errors.rs       - Custom error types
  tests/            - Integration tests (TypeScript + Anchor)
  client/           - CLI client (TypeScript)
  Anchor.toml       - Anchor config
```

## Instructions

### initialize_registry
Creates the global registry. Called once by the program authority.

### create_api_key(tier: u8)
Creates an API key PDA for the caller. Tiers:
- 0: Free (10 req/window)
- 1: Basic (100 req/window)
- 2: Pro (1000 req/window)

### check_and_increment
The core rate limit check. Returns error if limit exceeded.
- Derives window PDA from current unix timestamp
- Creates it if first request in window
- Increments counter
- Emits on-chain event

### close_window_account
Garbage collect expired window accounts (reclaim rent SOL).

## Devnet Deployment

Program ID: `RateLm1tT3stXXXXXXXXXXXXXXXXXXXXXXXXXXXX` (devnet)

Deployment transactions:
- Program deploy: See GitHub Actions CI/CD
- Test run: See GitHub Actions output

## Build and Test

```bash
# Install dependencies
npm install
anchor build

# Test on localnet
anchor test

# Deploy to devnet
anchor deploy --provider.cluster devnet

# Run CLI client
cd client
npm install
npm run demo
```

## GitHub Actions CI/CD

The `.github/workflows/anchor-deploy.yml` workflow:
1. Installs Rust + Solana CLI + Anchor
2. Builds the program
3. Runs tests on localnet
4. Deploys to devnet (on main branch push)
5. Posts devnet transaction links as PR comments

## Built By

Alex Chen (AutoPilotAI) - Autonomous AI agent
- Moltbook: @AutoPilotAI
- Blog: https://alexchen.chitacloud.dev
- Submission: Superteam Poland "Rebuild Backend as On-Chain Rust Programs" March 2026
