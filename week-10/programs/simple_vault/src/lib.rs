use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, TransferChecked};

declare_id!("HnAz6jnZQ2iwBUWutPEs8HTWJMHey6DUG1dCR5DvHk9f");

#[program]
pub mod simple_vault {
    use super::*;

    pub fn initialize_vault(ctx: Context<InitializeVault>) -> Result<()> {
        let vault = &mut ctx.accounts.vault_state;
        vault.admin = ctx.accounts.admin.key();
        vault.token_mint = ctx.accounts.token_mint.key();
        vault.vault_token_account = ctx.accounts.vault_token_account.key();
        vault.share_mint = ctx.accounts.share_mint.key();
        vault.bump = ctx.bumps.vault_state;
        vault.total_assets = 0;
        Ok(())
    }

    pub fn create_user_card(ctx: Context<CreateUserCard>) -> Result<()> {
        let card = &mut ctx.accounts.user_card;
        card.vault = ctx.accounts.vault_state.key();
        card.owner = ctx.accounts.user.key();
        card.bump = ctx.bumps.user_card;
        card.deposited_amount = 0;
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        require!(amount > 0, VaultError::ZeroDeposit);
        require!(
            ctx.accounts.user_token_account.amount >= amount,
            VaultError::InsufficientTokens
        );

        let total_assets = ctx.accounts.vault_token_account.amount;
        let total_shares = ctx.accounts.share_mint.supply;
        let shares = if total_shares == 0 {
            amount
        } else {
            require!(total_assets > 0, VaultError::InvalidVaultBalance);
            let calculated = (amount as u128)
                .checked_mul(total_shares as u128)
                .ok_or(VaultError::ArithmeticOverflow)?
                .checked_div(total_assets as u128)
                .ok_or(VaultError::ArithmeticOverflow)?;
            u64::try_from(calculated).map_err(|_| error!(VaultError::ArithmeticOverflow))?
        };
        require!(shares > 0, VaultError::ZeroShares);

        let updated_total_assets = total_assets
            .checked_add(amount)
            .ok_or(VaultError::ArithmeticOverflow)?;
        let updated_user_deposit = ctx.accounts.user_card.deposited_amount
            .checked_add(amount)
            .ok_or(VaultError::ArithmeticOverflow)?;

        token::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.token_mint.decimals,
        )?;

        let token_mint = ctx.accounts.token_mint.key();
        let bump = [ctx.accounts.vault_state.bump];
        let seeds: &[&[u8]] = &[b"vault", token_mint.as_ref(), &bump];
        let signer_seeds = &[seeds];
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.share_mint.to_account_info(),
                    to: ctx.accounts.user_share_account.to_account_info(),
                    authority: ctx.accounts.vault_state.to_account_info(),
                },
                signer_seeds,
            ),
            shares,
        )?;

        ctx.accounts.vault_state.total_assets = updated_total_assets;
        ctx.accounts.user_card.deposited_amount = updated_user_deposit;

        emit!(Deposited {
            vault: ctx.accounts.vault_state.key(),
            user: ctx.accounts.user.key(),
            amount,
            shares,
        });
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, shares: u64) -> Result<()> {
        require!(shares > 0, VaultError::ZeroWithdrawal);
        require!(
            ctx.accounts.user_share_account.amount >= shares,
            VaultError::InsufficientShares
        );

        let total_assets = ctx.accounts.vault_token_account.amount;
        let total_shares = ctx.accounts.share_mint.supply;
        require!(total_shares >= shares, VaultError::InsufficientShares);
        let calculated = (shares as u128)
            .checked_mul(total_assets as u128)
            .ok_or(VaultError::ArithmeticOverflow)?
            .checked_div(total_shares as u128)
            .ok_or(VaultError::ArithmeticOverflow)?;
        let amount =
            u64::try_from(calculated).map_err(|_| error!(VaultError::ArithmeticOverflow))?;
        require!(amount > 0, VaultError::ZeroWithdrawalAmount);
        let updated_total_assets = total_assets
            .checked_sub(amount)
            .ok_or(VaultError::ArithmeticOverflow)?;

        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.share_mint.to_account_info(),
                    from: ctx.accounts.user_share_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            shares,
        )?;

        let token_mint = ctx.accounts.token_mint.key();
        let bump = [ctx.accounts.vault_state.bump];
        let seeds: &[&[u8]] = &[b"vault", token_mint.as_ref(), &bump];
        let signer_seeds = &[seeds];
        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.vault_state.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
            ctx.accounts.token_mint.decimals,
        )?;

        ctx.accounts.vault_state.total_assets = updated_total_assets;
        emit!(Withdrawn {
            vault: ctx.accounts.vault_state.key(),
            user: ctx.accounts.user.key(),
            shares,
            amount,
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(
        init,
        payer = admin,
        space = VaultState::LEN,
        seeds = [b"vault", token_mint.key().as_ref()],
        bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        init,
        payer = admin,
        token::mint = token_mint,
        token::authority = vault_state,
        seeds = [b"vault_tokens", token_mint.key().as_ref()],
        bump,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = admin,
        mint::decimals = token_mint.decimals,
        mint::authority = vault_state,
        seeds = [b"share_mint", token_mint.key().as_ref()],
        bump,
    )]
    pub share_mint: Account<'info, Mint>,

    pub token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CreateUserCard<'info> {
    #[account(
        seeds = [b"vault", vault_state.token_mint.as_ref()],
        bump = vault_state.bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        init,
        payer = user,
        space = UserCard::LEN,
        seeds = [b"user_card", vault_state.key().as_ref(), user.key().as_ref()],
        bump,
    )]
    pub user_card: Account<'info, UserCard>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(
        mut,
        seeds = [b"vault", token_mint.key().as_ref()],
        bump = vault_state.bump,
        has_one = token_mint,
        has_one = vault_token_account,
        has_one = share_mint,
    )]
    pub vault_state: Box<Account<'info, VaultState>>,

    #[account(
        mut,
        seeds = [b"user_card", vault_state.key().as_ref(), user.key().as_ref()],
        bump = user_card.bump,
        constraint = user_card.owner == user.key() @ VaultError::InvalidUserCard,
        constraint = user_card.vault == vault_state.key() @ VaultError::InvalidUserCard,
    )]
    pub user_card: Box<Account<'info, UserCard>>,

    #[account(
        mut,
        seeds = [b"vault_tokens", token_mint.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = vault_state,
    )]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [b"share_mint", token_mint.key().as_ref()],
        bump,
        mint::authority = vault_state,
        constraint = share_mint.decimals == token_mint.decimals @ VaultError::InvalidDecimals,
    )]
    pub share_mint: Box<Account<'info, Mint>>,

    pub token_mint: Box<Account<'info, Mint>>,

    #[account(mut, token::mint = token_mint, token::authority = user)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut, token::mint = share_mint, token::authority = user)]
    pub user_share_account: Box<Account<'info, TokenAccount>>,

    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(
        mut,
        seeds = [b"vault", token_mint.key().as_ref()],
        bump = vault_state.bump,
        has_one = token_mint,
        has_one = vault_token_account,
        has_one = share_mint,
    )]
    pub vault_state: Box<Account<'info, VaultState>>,

    #[account(
        seeds = [b"user_card", vault_state.key().as_ref(), user.key().as_ref()],
        bump = user_card.bump,
        constraint = user_card.owner == user.key() @ VaultError::InvalidUserCard,
        constraint = user_card.vault == vault_state.key() @ VaultError::InvalidUserCard,
    )]
    pub user_card: Box<Account<'info, UserCard>>,

    #[account(
        mut,
        seeds = [b"vault_tokens", token_mint.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = vault_state,
    )]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [b"share_mint", token_mint.key().as_ref()],
        bump,
        mint::authority = vault_state,
        constraint = share_mint.decimals == token_mint.decimals @ VaultError::InvalidDecimals,
    )]
    pub share_mint: Box<Account<'info, Mint>>,

    pub token_mint: Box<Account<'info, Mint>>,

    #[account(mut, token::mint = token_mint, token::authority = user)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut, token::mint = share_mint, token::authority = user)]
    pub user_share_account: Box<Account<'info, TokenAccount>>,

    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(Default)]
pub struct VaultState {
    pub admin: Pubkey,
    pub token_mint: Pubkey,
    pub vault_token_account: Pubkey,
    pub share_mint: Pubkey,
    pub bump: u8,
    pub total_assets: u64,
}

impl VaultState {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 32 + 1 + 8;
}

#[account]
#[derive(Default)]
pub struct UserCard {
    pub vault: Pubkey,
    pub owner: Pubkey,
    pub bump: u8,
    pub deposited_amount: u64,
}

impl UserCard {
    pub const LEN: usize = 8 + 32 + 32 + 1 + 8;
}

#[event]
pub struct Deposited {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
    pub shares: u64,
}

#[event]
pub struct Withdrawn {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub shares: u64,
    pub amount: u64,
}

#[error_code]
pub enum VaultError {
    #[msg("Deposit amount must be greater than zero")]
    ZeroDeposit,
    #[msg("Deposit is too small to receive any shares")]
    ZeroShares,
    #[msg("Vault has shares but no assets")]
    InvalidVaultBalance,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("User card does not match the depositor and vault")]
    InvalidUserCard,
    #[msg("Token and share decimals must match")]
    InvalidDecimals,
    #[msg("User has insufficient deposit tokens")]
    InsufficientTokens,
    #[msg("Withdrawal shares must be greater than zero")]
    ZeroWithdrawal,
    #[msg("User has insufficient shares")]
    InsufficientShares,
    #[msg("Withdrawal is too small to receive any tokens")]
    ZeroWithdrawalAmount,
}
