use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

entrypoint!(process_instruction);

enum DonationInstruction {
    Donate,
}

impl DonationInstruction {
    fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        match data {
            [0] => Ok(Self::Donate),
            _ => {
                msg!("Error: Donate requires exactly one byte: 0.");
                Err(ProgramError::InvalidInstructionData)
            }
        }
    }
}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match DonationInstruction::unpack(instruction_data)? {
        DonationInstruction::Donate => donate(program_id, accounts),
    }
}

fn donate(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let state = accounts.first().ok_or_else(|| {
        msg!("Error: counter account is missing.");
        ProgramError::NotEnoughAccountKeys
    })?;

    if state.owner != program_id {
        msg!("Error: counter account must be owned by this program.");
        return Err(ProgramError::IncorrectProgramId);
    }

    if !state.is_writable || state.executable {
        msg!("Error: counter account must be writable and non-executable.");
        return Err(ProgramError::InvalidAccountData);
    }

    let mut data = state.try_borrow_mut_data()?;

    let bytes: [u8; 8] = data.as_ref().try_into().map_err(|_| {
        msg!("Error: counter account data must contain exactly 8 bytes.");
        ProgramError::InvalidAccountData
    })?;

    let count = u64::from_le_bytes(bytes);

    let next_count = count.checked_add(1).ok_or_else(|| {
        msg!("Error: donation counter overflow.");
        ProgramError::Custom(1)
    })?;

    data.copy_from_slice(&next_count.to_le_bytes());

    msg!("Donate: {} -> {}", count, next_count);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROGRAM_ID: Pubkey = Pubkey::new_from_array([1; 32]);

    fn execute(
        data: &mut [u8],
        owner: &Pubkey,
        writable: bool,
        executable: bool,
        instruction: &[u8],
    ) -> ProgramResult {
        let key = Pubkey::new_from_array([2; 32]);
        let mut lamports = 1_000_000;

        let account = AccountInfo::new(
            &key,
            false,
            writable,
            &mut lamports,
            data,
            owner,
            executable,
            0,
        );

        process_instruction(&PROGRAM_ID, &[account], instruction)
    }

    #[test]
    fn donate_preserves_state_between_calls() {
        let mut data = [0; 8];

        for expected in 1_u64..=3 {
            assert_eq!(
                execute(&mut data, &PROGRAM_ID, true, false, &[0]),
                Ok(())
            );

            assert_eq!(u64::from_le_bytes(data), expected);
        }
    }

    #[test]
    fn rejects_missing_account() {
        assert_eq!(
            process_instruction(&PROGRAM_ID, &[], &[0]),
            Err(ProgramError::NotEnoughAccountKeys)
        );
    }

    #[test]
    fn rejects_invalid_instruction_without_changing_state() {
        for instruction in [&[][..], &[1][..], &[0, 0][..]] {
            let mut data = 7_u64.to_le_bytes();

            assert_eq!(
                execute(
                    &mut data,
                    &PROGRAM_ID,
                    true,
                    false,
                    instruction
                ),
                Err(ProgramError::InvalidInstructionData)
            );

            assert_eq!(u64::from_le_bytes(data), 7);
        }
    }

    #[test]
    fn rejects_foreign_owner_without_changing_state() {
        let mut data = 7_u64.to_le_bytes();
        let owner = Pubkey::new_from_array([3; 32]);

        assert_eq!(
            execute(
                &mut data,
                &owner,
                true,
                false,
                &[0]
            ),
            Err(ProgramError::IncorrectProgramId)
        );

        assert_eq!(u64::from_le_bytes(data), 7);
    }

    #[test]
    fn rejects_readonly_or_executable_account() {
        for (writable, executable) in [
            (false, false),
            (true, true),
        ] {
            let mut data = 7_u64.to_le_bytes();

            assert_eq!(
                execute(
                    &mut data,
                    &PROGRAM_ID,
                    writable,
                    executable,
                    &[0]
                ),
                Err(ProgramError::InvalidAccountData)
            );

            assert_eq!(u64::from_le_bytes(data), 7);
        }
    }

    #[test]
    fn rejects_invalid_data_length_without_changing_state() {
        for length in [0, 7, 9] {
            let mut data = vec![0; length];

            assert_eq!(
                execute(
                    &mut data,
                    &PROGRAM_ID,
                    true,
                    false,
                    &[0]
                ),
                Err(ProgramError::InvalidAccountData)
            );

            assert_eq!(data, vec![0; length]);
        }
    }

    #[test]
    fn rejects_overflow_without_changing_state() {
        let mut data = u64::MAX.to_le_bytes();

        assert_eq!(
            execute(
                &mut data,
                &PROGRAM_ID,
                true,
                false,
                &[0]
            ),
            Err(ProgramError::Custom(1))
        );

        assert_eq!(u64::from_le_bytes(data), u64::MAX);
    }
}