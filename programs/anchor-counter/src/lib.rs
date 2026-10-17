use anchor_lang::prelude::*;

// Program ID обновится автоматически после Build в Solana Playground.
declare_id!("4rMKbwWFWV9K6wLqWkyLQyvNHD6BXFWpxs6H3D5ZF1gt");

#[program]
mod anchor_counter {
    use super::*;

    // Создает новый Counter account со значением 0.
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.count = 0;

        msg!("Counter initialized: {}", counter.count);

        Ok(())
    }

    // Увеличивает счетчик на 1.
    pub fn increment(ctx: Context<UpdateCounter>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;

        let old_value = counter.count;

        counter.count = counter
            .count
            .checked_add(1)
            .ok_or(CounterError::Overflow)?;

        msg!("Counter incremented: {} -> {}", old_value, counter.count);

        Ok(())
    }

    // Уменьшает счетчик на 1.
    pub fn decrement(ctx: Context<UpdateCounter>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;

        require!(counter.count > 0, CounterError::AlreadyZero);

        let old_value = counter.count;
        counter.count -= 1;

        msg!("Counter decremented: {} -> {}", old_value, counter.count);

        Ok(())
    }
}

// Используется только при создании Counter account.
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = signer,
        space = 8 + 8
    )]
    pub counter: Account<'info, Counter>,

    #[account(mut)]
    pub signer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// Используется при increment и decrement.
#[derive(Accounts)]
pub struct UpdateCounter<'info> {
    #[account(mut)]
    pub counter: Account<'info, Counter>,
}

// Данные, которые хранятся on-chain.
#[account]
pub struct Counter {
    pub count: u64,
}

// Пользовательские ошибки.
#[error_code]
pub enum CounterError {
    #[msg("Counter cannot be decreased below zero.")]
    AlreadyZero,

    #[msg("Counter overflow.")]
    Overflow,
}