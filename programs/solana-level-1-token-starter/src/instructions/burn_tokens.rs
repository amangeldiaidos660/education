use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, BurnChecked, Mint, TokenAccount, TokenInterface,
};

use crate::error::TokenStarterError;

#[derive(Accounts)]
pub struct BurnTokens<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = authority,
        token::token_program = token_program,
    )]
    pub token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<BurnTokens>, amount: u64) -> Result<()> {
    require!(amount > 0, TokenStarterError::AmountMustBePositive);

    let decimals = ctx.accounts.mint.decimals;

    let cpi_accounts = BurnChecked {
        mint: ctx.accounts.mint.to_account_info(),
        from: ctx.accounts.token_account.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };

    let cpi_context =
        CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts);

    token_interface::burn_checked(cpi_context, amount, decimals)
}