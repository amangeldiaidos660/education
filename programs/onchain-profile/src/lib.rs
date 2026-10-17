use anchor_lang::prelude::*;

declare_id!("FqxcxoUDob741NNaN2ttbpNwKm13y58nM3kuuQuv8eZY");

#[program]
mod onchain_profile {
    use super::*;

    pub fn create_profile(
        ctx: Context<CreateProfile>,
        username: String,
        bio: String,
    ) -> Result<()> {
        require!(
            username.len() <= Profile::MAX_USERNAME_LEN,
            ProfileError::UsernameTooLong
        );

        require!(
            bio.len() <= Profile::MAX_BIO_LEN,
            ProfileError::BioTooLong
        );

        let profile = &mut ctx.accounts.profile;

        profile.owner = ctx.accounts.signer.key();
        profile.username = username;
        profile.bio = bio;
        profile.bump = ctx.bumps.profile;

        msg!("Profile created for: {}", profile.owner);

        Ok(())
    }

    pub fn update_profile(
        ctx: Context<UpdateProfile>,
        username: String,
        bio: String,
    ) -> Result<()> {
        require!(
            username.len() <= Profile::MAX_USERNAME_LEN,
            ProfileError::UsernameTooLong
        );

        require!(
            bio.len() <= Profile::MAX_BIO_LEN,
            ProfileError::BioTooLong
        );

        let profile = &mut ctx.accounts.profile;

        profile.username = username;
        profile.bio = bio;

        msg!("Profile updated for: {}", profile.owner);

        Ok(())
    }

    pub fn delete_profile(_ctx: Context<DeleteProfile>) -> Result<()> {
        msg!("Profile deleted");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateProfile<'info> {
    #[account(
        init,
        payer = signer,
        space = Profile::SPACE,
        seeds = [b"profile", signer.key().as_ref()],
        bump
    )]
    pub profile: Account<'info, Profile>,

    #[account(mut)]
    pub signer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateProfile<'info> {
    #[account(
        mut,
        seeds = [b"profile", signer.key().as_ref()],
        bump = profile.bump,
        constraint = profile.owner == signer.key() @ ProfileError::Unauthorized
    )]
    pub profile: Account<'info, Profile>,

    #[account(mut)]
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct DeleteProfile<'info> {
    #[account(
        mut,
        close = signer,
        seeds = [b"profile", signer.key().as_ref()],
        bump = profile.bump,
        constraint = profile.owner == signer.key() @ ProfileError::Unauthorized
    )]
    pub profile: Account<'info, Profile>,

    #[account(mut)]
    pub signer: Signer<'info>,
}

#[account]
pub struct Profile {
    pub owner: Pubkey,
    pub username: String,
    pub bio: String,
    pub bump: u8,
}

impl Profile {
    pub const MAX_USERNAME_LEN: usize = 32;
    pub const MAX_BIO_LEN: usize = 128;

    pub const SPACE: usize =
        8 +                 // discriminator
        32 +                // owner
        4 + Self::MAX_USERNAME_LEN +
        4 + Self::MAX_BIO_LEN +
        1;                  // bump
}

#[error_code]
pub enum ProfileError {
    #[msg("Username is too long.")]
    UsernameTooLong,

    #[msg("Bio is too long.")]
    BioTooLong,

    #[msg("Only the profile owner can modify this profile.")]
    Unauthorized,
}