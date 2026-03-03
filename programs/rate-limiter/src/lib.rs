use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::RateLimiterError;

pub mod state;
pub mod errors;

declare_id!("RateLm1tXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX");

// Window duration in seconds (1 hour)
const WINDOW_SIZE: i64 = 3600;

// Rate limit tiers: max requests per window
const TIER_LIMITS: [u32; 3] = [10, 100, 1000];

#[program]
pub mod rate_limiter {
    use super::*;

    /// Initialize the global registry. Called once by program authority.
    pub fn initialize_registry(ctx: Context<InitializeRegistry>) -> Result<()> {
        let registry = &mut ctx.accounts.registry;
        registry.authority = ctx.accounts.authority.key();
        registry.total_keys = 0;
        registry.bump = ctx.bumps.registry;
        emit!(RegistryInitialized {
            authority: registry.authority,
        });
        Ok(())
    }

    /// Create an API key for the caller with a specified tier.
    /// Tier 0 = Free (10 req/hr), Tier 1 = Basic (100 req/hr), Tier 2 = Pro (1000 req/hr)
    pub fn create_api_key(ctx: Context<CreateApiKey>, tier: u8) -> Result<()> {
        require!(tier < 3, RateLimiterError::InvalidTier);
        
        let api_key = &mut ctx.accounts.api_key;
        let registry = &mut ctx.accounts.registry;
        
        api_key.owner = ctx.accounts.owner.key();
        api_key.tier = tier;
        api_key.is_active = true;
        api_key.total_requests = 0;
        api_key.total_denied = 0;
        api_key.created_at = Clock::get()?.unix_timestamp;
        api_key.bump = ctx.bumps.api_key;
        
        registry.total_keys += 1;
        
        emit!(ApiKeyCreated {
            owner: api_key.owner,
            tier,
            limit_per_window: TIER_LIMITS[tier as usize],
        });
        
        Ok(())
    }

    /// Core rate limit check and increment.
    /// This is called on every "request". Returns RateLimitExceeded if over limit.
    /// 
    /// Web2 equivalent:
    ///   function checkRateLimit(apiKey, tier) {
    ///     const window = Math.floor(Date.now() / WINDOW_MS);
    ///     const key = `ratelimit:${apiKey}:${window}`;
    ///     const count = await redis.incr(key);
    ///     await redis.expire(key, WINDOW_SECONDS);
    ///     if (count > TIER_LIMITS[tier]) throw new RateLimitError();
    ///   }
    pub fn check_and_increment(ctx: Context<CheckAndIncrement>) -> Result<()> {
        let api_key = &mut ctx.accounts.api_key;
        require!(api_key.is_active, RateLimiterError::ApiKeyInactive);
        
        let clock = Clock::get()?;
        let current_window = clock.unix_timestamp / WINDOW_SIZE;
        let limit = TIER_LIMITS[api_key.tier as usize];
        
        let rate_state = &mut ctx.accounts.rate_limit_state;
        
        // Initialize if new window
        if rate_state.window_start == 0 {
            rate_state.api_key = api_key.key();
            rate_state.window_start = current_window * WINDOW_SIZE;
            rate_state.request_count = 0;
            rate_state.bump = ctx.bumps.rate_limit_state;
        }
        
        // Check limit before incrementing
        if rate_state.request_count >= limit {
            api_key.total_denied += 1;
            emit!(RateLimitExceeded {
                api_key: api_key.key(),
                window_start: rate_state.window_start,
                count: rate_state.request_count,
                limit,
            });
            return err!(RateLimiterError::RateLimitExceeded);
        }
        
        // Increment
        rate_state.request_count += 1;
        api_key.total_requests += 1;
        
        emit!(RequestAllowed {
            api_key: api_key.key(),
            window_start: rate_state.window_start,
            count: rate_state.request_count,
            limit,
        });
        
        Ok(())
    }

    /// Revoke an API key (admin function).
    pub fn revoke_api_key(ctx: Context<RevokeApiKey>) -> Result<()> {
        let api_key = &mut ctx.accounts.api_key;
        api_key.is_active = false;
        emit!(ApiKeyRevoked { owner: api_key.owner });
        Ok(())
    }

    /// Upgrade an API key tier (owner only).
    pub fn upgrade_tier(ctx: Context<UpgradeTier>, new_tier: u8) -> Result<()> {
        require!(new_tier < 3, RateLimiterError::InvalidTier);
        let api_key = &mut ctx.accounts.api_key;
        let old_tier = api_key.tier;
        api_key.tier = new_tier;
        emit!(TierUpgraded {
            owner: api_key.owner,
            old_tier,
            new_tier,
        });
        Ok(())
    }

    /// Close an expired window account to reclaim rent.
    /// Web2 equivalent: Redis TTL cleanup happens automatically.
    /// On Solana we must explicitly close old accounts to reclaim SOL.
    pub fn close_window_account(_ctx: Context<CloseWindowAccount>) -> Result<()> {
        // Anchor handles account closing via `close` constraint
        Ok(())
    }
}

// ==================== Account Contexts ====================

#[derive(Accounts)]
pub struct InitializeRegistry<'info> {
    #[account(
        init,
        payer = authority,
        space = Registry::SIZE,
        seeds = [b"registry"],
        bump
    )]
    pub registry: Account<'info, Registry>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateApiKey<'info> {
    #[account(
        init,
        payer = owner,
        space = ApiKey::SIZE,
        seeds = [b"apikey", owner.key().as_ref()],
        bump
    )]
    pub api_key: Account<'info, ApiKey>,
    
    #[account(mut, seeds = [b"registry"], bump = registry.bump)]
    pub registry: Account<'info, Registry>,
    
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction()]
pub struct CheckAndIncrement<'info> {
    #[account(
        mut,
        seeds = [b"apikey", api_key.owner.as_ref()],
        bump = api_key.bump,
        has_one = owner
    )]
    pub api_key: Account<'info, ApiKey>,
    
    #[account(
        init_if_needed,
        payer = owner,
        space = RateLimitState::SIZE,
        seeds = [
            b"ratelimit",
            api_key.key().as_ref(),
            &(Clock::get().unwrap().unix_timestamp / WINDOW_SIZE).to_le_bytes()
        ],
        bump
    )]
    pub rate_limit_state: Account<'info, RateLimitState>,
    
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RevokeApiKey<'info> {
    #[account(
        mut,
        seeds = [b"apikey", api_key.owner.as_ref()],
        bump = api_key.bump
    )]
    pub api_key: Account<'info, ApiKey>,
    
    #[account(address = api_key.owner)]
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpgradeTier<'info> {
    #[account(
        mut,
        seeds = [b"apikey", api_key.owner.as_ref()],
        bump = api_key.bump,
        has_one = owner
    )]
    pub api_key: Account<'info, ApiKey>,
    
    #[account(mut)]
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct CloseWindowAccount<'info> {
    #[account(
        mut,
        close = owner,
        seeds = [
            b"ratelimit",
            rate_limit_state.api_key.as_ref(),
            &(rate_limit_state.window_start / WINDOW_SIZE).to_le_bytes()
        ],
        bump = rate_limit_state.bump
    )]
    pub rate_limit_state: Account<'info, RateLimitState>,
    
    #[account(mut)]
    pub owner: Signer<'info>,
}

// ==================== Events ====================

#[event]
pub struct RegistryInitialized {
    pub authority: Pubkey,
}

#[event]
pub struct ApiKeyCreated {
    pub owner: Pubkey,
    pub tier: u8,
    pub limit_per_window: u32,
}

#[event]
pub struct ApiKeyRevoked {
    pub owner: Pubkey,
}

#[event]
pub struct TierUpgraded {
    pub owner: Pubkey,
    pub old_tier: u8,
    pub new_tier: u8,
}

#[event]
pub struct RequestAllowed {
    pub api_key: Pubkey,
    pub window_start: i64,
    pub count: u32,
    pub limit: u32,
}

#[event]
pub struct RateLimitExceeded {
    pub api_key: Pubkey,
    pub window_start: i64,
    pub count: u32,
    pub limit: u32,
}
