use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    Mint,
    TokenAccount,
    TokenInterface,
};

use crate::{
    constants::{ESCROW_SEED, VAULT_SEED},
    error::EscrowError,
    state::{DealStatus, EscrowState},
};

#[derive(Accounts)]
#[instruction(deal_id: u64)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,

    #[account(
        init,
        payer = sender,
        space = EscrowState::LEN,
        seeds = [
            ESCROW_SEED,
            sender.key().as_ref(),
            &deal_id.to_le_bytes(),
        ],
        bump
    )]
    pub escrow_state: Account<'info, EscrowState>,

    #[account(
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = sender,
        seeds = [
            VAULT_SEED,
            escrow_state.key().as_ref(),
        ],
        bump,
        token::mint = mint,
        token::authority = escrow_state,
        token::token_program = token_program,
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    #[account(address = anchor_spl::token_2022::ID)]
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handle_initialize(
    ctx: Context<Initialize>,
    deal_id: u64,
    receiver: Pubkey,
    amount: u64,
) -> Result<()> {
    require!(
        amount > 0,
        EscrowError::AmountMustBePositive
    );

    require!(
        receiver != ctx.accounts.sender.key(),
        EscrowError::SenderEqualsReceiver
    );

    let escrow_state = &mut ctx.accounts.escrow_state;

    escrow_state.sender = ctx.accounts.sender.key();
    escrow_state.receiver = receiver;
    escrow_state.mint = ctx.accounts.mint.key();
    escrow_state.amount = amount;
    escrow_state.deal_id = deal_id;
    escrow_state.bump = ctx.bumps.escrow_state;
    escrow_state.status = DealStatus::Created;

    Ok(())
}