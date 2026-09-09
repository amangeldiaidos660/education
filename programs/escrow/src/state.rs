use anchor_lang::prelude::*;

#[derive(
    AnchorSerialize,
    AnchorDeserialize,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
)]
pub enum DealStatus {
    Created,
    Funded,
    Released,
    Cancelled,
}

#[account]
pub struct EscrowState {
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub deal_id: u64,
    pub bump: u8,
    pub status: DealStatus,
}

impl EscrowState {
    pub const LEN: usize =
        8 +  // Anchor discriminator
        32 + // sender
        32 + // receiver
        32 + // mint
        8 +  // amount
        8 +  // deal_id
        1 +  // bump
        1;   // DealStatus enum
}