pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("3FQKshuMwXfxxqjE2nF8CXgcfBCU8HKDPGg8JJgNJtKd");

#[program]
pub mod escrow {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        deal_id: u64,
        receiver: Pubkey,
        amount: u64,
    ) -> Result<()> {
        crate::instructions::initialize::handle_initialize(
            ctx,
            deal_id,
            receiver,
            amount,
        )
    }

    pub fn deposit(ctx: Context<Deposit>) -> Result<()> {
        crate::instructions::deposit::handle_deposit(ctx)
    }

    pub fn release(ctx: Context<Release>) -> Result<()> {
        crate::instructions::release::handle_release(ctx)
    }

    pub fn cancel(ctx: Context<Cancel>) -> Result<()> {
        crate::instructions::cancel::handle_cancel(ctx)
    }
}