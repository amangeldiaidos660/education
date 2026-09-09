use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token_interface::{
        self,
        CloseAccount,
        Mint,
        TokenAccount,
        TokenInterface,
        TransferChecked,
    },
};

use crate::{
    constants::{ESCROW_SEED, VAULT_SEED},
    error::EscrowError,
    state::{DealStatus, EscrowState},
};

#[derive(Accounts)]
pub struct Release<'info> {
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
        constraint = escrow_state.sender == sender.key()
            @ EscrowError::Unauthorized,
        constraint = escrow_state.mint == mint.key()
            @ EscrowError::InvalidMint,
        constraint = escrow_state.receiver == receiver.key()
            @ EscrowError::InvalidReceiver,
    )]
    pub escrow_state: Account<'info, EscrowState>,

    /// CHECK:
    /// Receiver pubkey is strictly matched against escrow_state.receiver.
    pub receiver: UncheckedAccount<'info>,

    #[account(
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

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

    #[account(
        mut,
        token::mint = mint,
        token::authority = receiver,
        token::token_program = token_program,
        constraint =
            receiver_token_account.key()
                == get_associated_token_address_with_program_id(
                    &receiver.key(),
                    &mint.key(),
                    &token_program.key(),
                )
            @ EscrowError::InvalidReceiver,
    )]
    pub receiver_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(address = anchor_spl::token_2022::ID)]
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handle_release(ctx: Context<Release>) -> Result<()> {
    require!(
        ctx.accounts.escrow_state.status == DealStatus::Funded,
        EscrowError::InvalidState
    );

    // let amount = ctx.accounts.escrow_state.amount;
    let amount = ctx.accounts.vault.amount;
    let decimals = ctx.accounts.mint.decimals;

    let sender_key = ctx.accounts.escrow_state.sender;
    let deal_id_bytes =
        ctx.accounts.escrow_state.deal_id.to_le_bytes();
    let bump = [ctx.accounts.escrow_state.bump];

    let signer_seeds: &[&[u8]] = &[
        ESCROW_SEED,
        sender_key.as_ref(),
        &deal_id_bytes,
        &bump,
    ];

    let signer = &[signer_seeds];

    let transfer_accounts = TransferChecked {
        from: ctx.accounts.vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.receiver_token_account.to_account_info(),
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

    ctx.accounts.escrow_state.status = DealStatus::Released;

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