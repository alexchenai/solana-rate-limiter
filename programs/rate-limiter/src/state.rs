use anchor_lang::prelude::*;

/// Global registry account (PDA: [b"registry"])
/// Singleton - one per program deployment
#[account]
pub struct Registry {
    /// Program authority (can revoke keys, upgrade tiers)
    pub authority: Pubkey,
    /// Total API keys created
    pub total_keys: u64,
    /// PDA bump
    pub bump: u8,
}

impl Registry {
    pub const SIZE: usize = 8   // discriminator
        + 32    // authority
        + 8     // total_keys
        + 1;    // bump
}

/// API Key account (PDA: [b"apikey", owner_pubkey])
/// One per wallet address
#[account]
pub struct ApiKey {
    /// Owner wallet
    pub owner: Pubkey,
    /// Rate limit tier (0=Free, 1=Basic, 2=Pro)
    pub tier: u8,
    /// Whether this key is active
    pub is_active: bool,
    /// Lifetime total requests allowed
    pub total_requests: u64,
    /// Lifetime total requests denied
    pub total_denied: u64,
    /// Unix timestamp when created
    pub created_at: i64,
    /// PDA bump
    pub bump: u8,
}

impl ApiKey {
    pub const SIZE: usize = 8   // discriminator
        + 32    // owner
        + 1     // tier
        + 1     // is_active
        + 8     // total_requests
        + 8     // total_denied
        + 8     // created_at
        + 1;    // bump
}

/// Per-window rate limit state (PDA: [b"ratelimit", api_key_pubkey, window_start_bytes])
/// Created lazily on first request in a window.
/// Equivalent to a Redis key with TTL.
///
/// Web2: redis.incr(`ratelimit:${apiKey}:${windowId}`)
/// Solana: Account PDA derived from (api_key, window_id)
#[account]
pub struct RateLimitState {
    /// Parent API key
    pub api_key: Pubkey,
    /// Unix timestamp of window start (floor(unix_ts / WINDOW_SIZE) * WINDOW_SIZE)
    pub window_start: i64,
    /// Number of requests in this window
    pub request_count: u32,
    /// PDA bump
    pub bump: u8,
}

impl RateLimitState {
    pub const SIZE: usize = 8   // discriminator
        + 32    // api_key
        + 8     // window_start
        + 4     // request_count
        + 1;    // bump
}
