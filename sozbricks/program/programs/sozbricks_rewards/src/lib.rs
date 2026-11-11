use anchor_lang::{
    prelude::*,
    system_program::{self, Transfer},
};

declare_id!("3qbR1eZRqXUWroWKKYhbDmR3FfqTHfqSU8zZSxtANzYh");

pub const AUTHORITY: Pubkey = pubkey!("615vW8VZFB8fLGrPTcxQ2FWBWkevahkjX5kCbtoosyQw");
pub const MAX_REWARD: u64 = 500_000_000;

#[program]
pub mod sozbricks_rewards {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.config.authority = ctx.accounts.authority.key();
        ctx.accounts.config.paused = false;
        ctx.accounts.config.max_reward = MAX_REWARD;
        Ok(())
    }

    pub fn set_paused(ctx: Context<Configure>, paused: bool) -> Result<()> {
        ctx.accounts.config.paused = paused;
        Ok(())
    }

    pub fn fund(ctx: Context<Fund>, amount: u64) -> Result<()> {
        require!(amount > 0, RewardsError::InvalidAmount);
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.funder.to_account_info(),
                    to: ctx.accounts.reserve.to_account_info(),
                },
            ),
            amount,
        )
    }

    pub fn payout(ctx: Context<Payout>, reward_id: [u8; 16], amount: u64) -> Result<()> {
        require!(!ctx.accounts.config.paused, RewardsError::Paused);
        require!(reward_id != [0; 16], RewardsError::InvalidReward);
        require!(
            amount > 0 && amount <= ctx.accounts.config.max_reward,
            RewardsError::InvalidAmount
        );
        require_keys_neq!(
            ctx.accounts.recipient.key(),
            ctx.accounts.reserve.key(),
            RewardsError::InvalidRecipient
        );
        require_keys_neq!(
            ctx.accounts.recipient.key(),
            ctx.accounts.authority.key(),
            RewardsError::InvalidRecipient
        );
        require!(
            ctx.accounts.reserve.lamports()
                >= amount.saturating_add(Rent::get()?.minimum_balance(0)),
            RewardsError::InsufficientReserve
        );
        let bump = [ctx.bumps.reserve];
        let seeds: &[&[u8]] = &[b"reserve", &bump];
        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.reserve.to_account_info(),
                    to: ctx.accounts.recipient.to_account_info(),
                },
                &[seeds],
            ),
            amount,
        )?;
        ctx.accounts.receipt.reward_id = reward_id;
        ctx.accounts.receipt.recipient = ctx.accounts.recipient.key();
        ctx.accounts.receipt.amount = amount;
        ctx.accounts.receipt.paid_slot = Clock::get()?.slot;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut, address = AUTHORITY)]
    pub authority: Signer<'info>,
    #[account(init, payer = authority, space = 8 + 32 + 1 + 8, seeds = [b"config"], bump)]
    pub config: Account<'info, Settings>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Configure<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [b"config"], bump, has_one = authority)]
    pub config: Account<'info, Settings>,
}

#[derive(Accounts)]
pub struct Fund<'info> {
    #[account(mut)]
    pub funder: Signer<'info>,
    #[account(seeds = [b"config"], bump)]
    pub config: Account<'info, Settings>,
    #[account(mut, seeds = [b"reserve"], bump)]
    pub reserve: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(reward_id: [u8; 16])]
pub struct Payout<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds = [b"config"], bump, has_one = authority)]
    pub config: Account<'info, Settings>,
    #[account(mut, seeds = [b"reserve"], bump)]
    pub reserve: SystemAccount<'info>,
    #[account(mut)]
    pub recipient: SystemAccount<'info>,
    #[account(init, payer = authority, space = 8 + 16 + 32 + 8 + 8, seeds = [b"receipt", reward_id.as_ref()], bump)]
    pub receipt: Account<'info, Receipt>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Settings {
    pub authority: Pubkey,
    pub paused: bool,
    pub max_reward: u64,
}

#[account]
pub struct Receipt {
    pub reward_id: [u8; 16],
    pub recipient: Pubkey,
    pub amount: u64,
    pub paid_slot: u64,
}

#[error_code]
pub enum RewardsError {
    #[msg("Rewards are paused")]
    Paused,
    #[msg("Invalid reward amount")]
    InvalidAmount,
    #[msg("Invalid reward identifier")]
    InvalidReward,
    #[msg("Invalid recipient")]
    InvalidRecipient,
    #[msg("Reserve has insufficient funds")]
    InsufficientReserve,
}
