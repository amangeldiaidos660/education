use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self,
    Mint,
    TokenAccount,
    TokenInterface,
    TransferChecked,
};

use crate::{
    constants::{ESCROW_SEED, VAULT_SEED},
    error::EscrowError,
    state::{DealStatus, EscrowState},
};

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,

    #[account(
        mut,
        seeds = [
            ESCROW_SEED,
            sender.key().as_ref(),
            &escrow_state.deal_id.to_le_bytes(),
        ],
        bump = escrow_state.bump,
        constraint = escrow_state.sender == sender.key() @ EscrowError::Unauthorized,
        constraint = escrow_state.mint == mint.key() @ EscrowError::InvalidMint,
    )]
    pub escrow_state: Account<'info, EscrowState>,

    #[account(
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = sender,
        token::token_program = token_program,
    )]
    pub sender_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
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
}

pub fn handle_deposit(ctx: Context<Deposit>) -> Result<()> {
    require!(
        ctx.accounts.escrow_state.status == DealStatus::Created,
        EscrowError::InvalidState
    );

    let amount = ctx.accounts.escrow_state.amount;
    let decimals = ctx.accounts.mint.decimals;

    require!(
        amount > 0,
        EscrowError::AmountMustBePositive
    );

    let cpi_accounts = TransferChecked {
        from: ctx.accounts.sender_token_account.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.vault.to_account_info(),
        authority: ctx.accounts.sender.to_account_info(),
    };

    let cpi_context = CpiContext::new(
        ctx.accounts.token_program.key(),
        cpi_accounts,
    );

    token_interface::transfer_checked(
        cpi_context,
        amount,
        decimals,
    )?;

    ctx.accounts.escrow_state.status = DealStatus::Funded;

    Ok(())
}