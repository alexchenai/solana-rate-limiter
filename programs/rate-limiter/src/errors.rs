use anchor_lang::prelude::*;

#[error_code]
pub enum RateLimiterError {
    #[msg("Rate limit exceeded for this time window")]
    RateLimitExceeded,
    
    #[msg("API key is inactive or has been revoked")]
    ApiKeyInactive,
    
    #[msg("Invalid tier: must be 0 (Free), 1 (Basic), or 2 (Pro)")]
    InvalidTier,
    
    #[msg("Unauthorized: only the key owner can perform this action")]
    Unauthorized,
    
    #[msg("Window account is still active: cannot close a current or future window")]
    WindowStillActive,
}
