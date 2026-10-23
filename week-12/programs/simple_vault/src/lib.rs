use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, TransferChecked};
use anchor_spl::{memo::Memo, token_2022::Token2022};
use raydium_clmm_cpi::{cpi as clmm_cpi, program::RaydiumClmm, states::{AmmConfig, ObservationState, PoolState}};

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
        ensure_base_only(&ctx.accounts.strategy_state, &ctx.accounts.secondary_vault, ctx.accounts.vault_state.key())?;
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
        ensure_base_only(&ctx.accounts.strategy_state, &ctx.accounts.secondary_vault, ctx.accounts.vault_state.key())?;
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

    pub fn initialize_strategy(ctx: Context<InitializeStrategy>) -> Result<()> {
        let strategy = &mut ctx.accounts.strategy_state;
        strategy.vault = ctx.accounts.vault_state.key();
        strategy.pool = ctx.accounts.pool_state.key();
        strategy.token_b = ctx.accounts.token_b.key();
        strategy.secondary_vault = ctx.accounts.secondary_vault.key();
        strategy.bump = ctx.bumps.strategy_state;
        Ok(())
    }

    pub fn swap<'info>(
        ctx: Context<'_, '_, '_, 'info, Swap<'info>>,
        amount: u64,
        minimum_out: u64,
        a_to_b: bool,
    ) -> Result<()> {
        require!(amount > 0, VaultError::ZeroSwap);
        require!(minimum_out > 0, VaultError::ZeroMinimumOut);
        let (input, output, input_vault, output_vault, input_mint, output_mint) = if a_to_b {
            (&ctx.accounts.vault_token_account, &ctx.accounts.secondary_vault,
             &ctx.accounts.pool_vault_a, &ctx.accounts.pool_vault_b,
             &ctx.accounts.token_mint, &ctx.accounts.token_b)
        } else {
            (&ctx.accounts.secondary_vault, &ctx.accounts.vault_token_account,
             &ctx.accounts.pool_vault_b, &ctx.accounts.pool_vault_a,
             &ctx.accounts.token_b, &ctx.accounts.token_mint)
        };
        require!(input.amount >= amount, VaultError::InsufficientTokens);
        let input_before = input.amount;
        let output_before = output.amount;
        let token_mint = ctx.accounts.token_mint.key();
        let bump = [ctx.accounts.vault_state.bump];
        let seeds: &[&[u8]] = &[b"vault", token_mint.as_ref(), &bump];
        let signers = &[seeds];
        let accounts = clmm_cpi::accounts::SwapSingleV2 {
            payer: ctx.accounts.vault_state.to_account_info(),
            amm_config: ctx.accounts.amm_config.to_account_info(),
            pool_state: ctx.accounts.pool_state.to_account_info(),
            input_token_account: input.to_account_info(),
            output_token_account: output.to_account_info(),
            input_vault: input_vault.to_account_info(),
            output_vault: output_vault.to_account_info(),
            observation_state: ctx.accounts.observation_state.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            token_program_2022: ctx.accounts.token_program_2022.to_account_info(),
            memo_program: ctx.accounts.memo_program.to_account_info(),
            input_vault_mint: input_mint.to_account_info(),
            output_vault_mint: output_mint.to_account_info(),
        };
        clmm_cpi::swap_v2(
            CpiContext::new_with_signer(ctx.accounts.clmm_program.to_account_info(), accounts, signers)
                .with_remaining_accounts(ctx.remaining_accounts.to_vec()),
            amount, minimum_out, 0, true,
        )?;
        ctx.accounts.vault_token_account.reload()?;
        ctx.accounts.secondary_vault.reload()?;
        let (input_after, output_after) = if a_to_b {
            (ctx.accounts.vault_token_account.amount, ctx.accounts.secondary_vault.amount)
        } else {
            (ctx.accounts.secondary_vault.amount, ctx.accounts.vault_token_account.amount)
        };
        let spent = input_before.checked_sub(input_after).ok_or(VaultError::ArithmeticOverflow)?;
        let received = output_after.checked_sub(output_before).ok_or(VaultError::ArithmeticOverflow)?;
        require!(spent == amount, VaultError::PartialSwap);
        require!(received >= minimum_out, VaultError::SlippageExceeded);
        ctx.accounts.vault_state.total_assets = ctx.accounts.vault_token_account.amount;
        emit!(Swapped { vault: ctx.accounts.vault_state.key(), pool: ctx.accounts.pool_state.key(),
            amount_in: spent, amount_out: received, minimum_out, a_to_b });
        Ok(())
    }
}

fn ensure_base_only(strategy: &AccountInfo, secondary: &AccountInfo, vault: Pubkey) -> Result<()> {
    if strategy.data_is_empty() {
        return Ok(());
    }
    require_keys_eq!(*strategy.owner, crate::ID, VaultError::InvalidStrategy);
    let state = StrategyState::try_deserialize(&mut &strategy.try_borrow_data()?[..])?;
    require_keys_eq!(state.vault, vault, VaultError::InvalidStrategy);
    require_keys_eq!(state.secondary_vault, secondary.key(), VaultError::InvalidStrategy);
    require_keys_eq!(*secondary.owner, token::ID, VaultError::InvalidStrategy);
    let account = TokenAccount::try_deserialize(&mut &secondary.try_borrow_data()?[..])?;
    require_keys_eq!(account.owner, vault, VaultError::InvalidStrategy);
    require_keys_eq!(account.mint, state.token_b, VaultError::InvalidStrategy);
    require!(account.amount == 0, VaultError::UnconvertedAssets);
    Ok(())
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
    /// CHECK: PDA seeds are checked here; ensure_base_only validates data when initialized.
    #[account(seeds = [b"strategy", vault_state.key().as_ref()], bump)]
    pub strategy_state: UncheckedAccount<'info>,
    /// CHECK: PDA seeds are checked here; ensure_base_only validates token data.
    #[account(seeds = [b"strategy_tokens", vault_state.key().as_ref()], bump)]
    pub secondary_vault: UncheckedAccount<'info>,
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
    /// CHECK: PDA seeds are checked here; ensure_base_only validates data when initialized.
    #[account(seeds = [b"strategy", vault_state.key().as_ref()], bump)]
    pub strategy_state: UncheckedAccount<'info>,
    /// CHECK: PDA seeds are checked here; ensure_base_only validates token data.
    #[account(seeds = [b"strategy_tokens", vault_state.key().as_ref()], bump)]
    pub secondary_vault: UncheckedAccount<'info>,
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

#[derive(Accounts)]
pub struct InitializeStrategy<'info> {
    #[account(seeds = [b"vault", token_mint.key().as_ref()], bump = vault_state.bump,
        has_one = admin, has_one = token_mint)]
    pub vault_state: Box<Account<'info, VaultState>>,
    #[account(init, payer = admin, space = 8 + 32 * 4 + 1,
        seeds = [b"strategy", vault_state.key().as_ref()], bump)]
    pub strategy_state: Box<Account<'info, StrategyState>>,
    #[account(init, payer = admin, seeds = [b"strategy_tokens", vault_state.key().as_ref()], bump,
        token::mint = token_b, token::authority = vault_state)]
    pub secondary_vault: Box<Account<'info, TokenAccount>>,
    pub token_mint: Box<Account<'info, Mint>>,
    pub token_b: Box<Account<'info, Mint>>,
    #[account(constraint = pool_state.load()?.token_mint_0 == token_mint.key() @ VaultError::InvalidPool,
        constraint = pool_state.load()?.token_mint_1 == token_b.key() @ VaultError::InvalidPool)]
    pub pool_state: AccountLoader<'info, PoolState>,
    pub clmm_program: Program<'info, RaydiumClmm>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut, seeds = [b"vault", token_mint.key().as_ref()], bump = vault_state.bump,
        has_one = admin, has_one = token_mint, has_one = vault_token_account)]
    pub vault_state: Box<Account<'info, VaultState>>,
    #[account(seeds = [b"strategy", vault_state.key().as_ref()], bump = strategy_state.bump,
        constraint = strategy_state.vault == vault_state.key() @ VaultError::InvalidStrategy,
        constraint = strategy_state.pool == pool_state.key() @ VaultError::InvalidPool,
        constraint = strategy_state.token_b == token_b.key() @ VaultError::InvalidStrategy,
        has_one = secondary_vault)]
    pub strategy_state: Box<Account<'info, StrategyState>>,
    #[account(mut, seeds = [b"vault_tokens", token_mint.key().as_ref()], bump,
        token::mint = token_mint, token::authority = vault_state)]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [b"strategy_tokens", vault_state.key().as_ref()], bump,
        token::mint = token_b, token::authority = vault_state)]
    pub secondary_vault: Box<Account<'info, TokenAccount>>,
    pub token_mint: Box<Account<'info, Mint>>,
    pub token_b: Box<Account<'info, Mint>>,
    #[account(mut,
        constraint = pool_state.load()?.token_mint_0 == token_mint.key() @ VaultError::InvalidPool,
        constraint = pool_state.load()?.token_mint_1 == token_b.key() @ VaultError::InvalidPool)]
    pub pool_state: AccountLoader<'info, PoolState>,
    #[account(address = pool_state.load()?.amm_config)]
    pub amm_config: Box<Account<'info, AmmConfig>>,
    #[account(mut, address = pool_state.load()?.token_vault_0,
        token::mint = token_mint, token::authority = pool_state)]
    pub pool_vault_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = pool_state.load()?.token_vault_1,
        token::mint = token_b, token::authority = pool_state)]
    pub pool_vault_b: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = pool_state.load()?.observation_key)]
    pub observation_state: AccountLoader<'info, ObservationState>,
    pub clmm_program: Program<'info, RaydiumClmm>,
    pub admin: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_program_2022: Program<'info, Token2022>,
    pub memo_program: Program<'info, Memo>,
}

#[account]
pub struct StrategyState {
    pub vault: Pubkey,
    pub pool: Pubkey,
    pub token_b: Pubkey,
    pub secondary_vault: Pubkey,
    pub bump: u8,
}

#[event]
pub struct Swapped {
    pub vault: Pubkey,
    pub pool: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub minimum_out: u64,
    pub a_to_b: bool,
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
    #[msg("Swap amount must be greater than zero")]
    ZeroSwap,
    #[msg("Minimum output must be greater than zero")]
    ZeroMinimumOut,
    #[msg("Invalid vault strategy")]
    InvalidStrategy,
    #[msg("Pool does not match the configured token pair")]
    InvalidPool,
    #[msg("Convert token B back to A before deposits or withdrawals")]
    UnconvertedAssets,
    #[msg("The entire input amount must be swapped")]
    PartialSwap,
    #[msg("Received amount is below the minimum")]
    SlippageExceeded,
}
