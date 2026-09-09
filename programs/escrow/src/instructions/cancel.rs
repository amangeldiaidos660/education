use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self,
    CloseAccount,
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
pub struct Cancel<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,

    #[account(
        mut,
        close = sender,
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

pub fn handle_cancel(ctx: Context<Cancel>) -> Result<()> {
    let status = ctx.accounts.escrow_state.status;

    require!(
        status == DealStatus::Created || status == DealStatus::Funded,
        EscrowError::InvalidState
    );

    let sender_key = ctx.accounts.escrow_state.sender;
    let deal_id_bytes = ctx.accounts.escrow_state.deal_id.to_le_bytes();
    let bump = [ctx.accounts.escrow_state.bump];

    let signer_seeds: &[&[u8]] = &[
        ESCROW_SEED,
        sender_key.as_ref(),
        &deal_id_bytes,
        &bump,
    ];

    let signer = &[signer_seeds];

    if status == DealStatus::Funded {
        // let amount = ctx.accounts.escrow_state.amount;
        let amount = ctx.accounts.vault.amount;
        let decimals = ctx.accounts.mint.decimals;

        let transfer_accounts = TransferChecked {
            from: ctx.accounts.vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.sender_token_account.to_account_info(),
            authority: ctx.accounts.escrow_state.to_account_info(),
        };

        let transfer_context = CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            transfer_accounts,
            signer,
        );

        token_interface::transfer_checked(
            transfer_context,
            amount,
            decimals,
        )?;
    }

    ctx.accounts.escrow_state.status = DealStatus::Cancelled;

    let close_accounts = CloseAccount {
        account: ctx.accounts.vault.to_account_info(),
        destination: ctx.accounts.sender.to_account_info(),
        authority: ctx.accounts.escrow_state.to_account_info(),
    };

    let close_context = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        close_accounts,
        signer,
    );

    token_interface::close_account(close_context)?;

    Ok(())
}