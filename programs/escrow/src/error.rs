use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Amount must be greater than zero")]
    AmountMustBePositive,

    #[msg("Sender and receiver must be different")]
    SenderEqualsReceiver,

    #[msg("Invalid escrow state")]
    InvalidState,

    #[msg("Unauthorized signer")]
    Unauthorized,

    #[msg("Invalid mint")]
    InvalidMint,

    #[msg("Invalid receiver")]
    InvalidReceiver,
}