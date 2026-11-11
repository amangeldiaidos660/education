#[cfg(test)]
mod tests {
    use litesvm::LiteSVM;
    use sha2::{Digest, Sha256};
    use solana_account::Account;
    use solana_address::Address;
    use solana_instruction::{AccountMeta, Instruction};
    use solana_message::Message;
    use solana_transaction::Transaction;
    use std::str::FromStr;

    fn ix(program: Address, name: &str, args: &[u8], accounts: Vec<AccountMeta>) -> Instruction {
        let hash = Sha256::digest(format!("global:{name}").as_bytes());
        Instruction {
            program_id: program,
            accounts,
            data: [&hash[..8], args].concat(),
        }
    }
    fn send(vm: &mut LiteSVM, payer: Address, instruction: Instruction) -> bool {
        vm.expire_blockhash();
        let tx = Transaction::new_unsigned(Message::new_with_blockhash(
            &[instruction],
            Some(&payer),
            &vm.latest_blockhash(),
        ));
        vm.send_transaction(tx).is_ok()
    }
    #[test]
    fn rewards_reserve_authority_pause_and_duplicate_receipts() {
        let source = include_str!("../../programs/sozbricks_rewards/src/lib.rs");
        let program = Address::from_str(
            source
                .lines()
                .find(|s| s.starts_with("declare_id!"))
                .unwrap()
                .split('"')
                .nth(1)
                .unwrap(),
        )
        .unwrap();
        let authority = Address::from_str("615vW8VZFB8fLGrPTcxQ2FWBWkevahkjX5kCbtoosyQw").unwrap();
        let intruder = Address::new_from_array([9; 32]);
        let recipient = Address::new_from_array([8; 32]);
        let settings = Address::find_program_address(&[b"config"], &program).0;
        let reserve = Address::find_program_address(&[b"reserve"], &program).0;
        let system = AccountMeta::new_readonly(Address::default(), false);
        let mut vm = LiteSVM::new().with_sigverify(false);
        vm.add_program(
            program,
            &std::fs::read("../target/deploy/sozbricks_rewards.so").expect("Build SBF first"),
        )
        .unwrap();
        for address in [authority, intruder] {
            vm.set_account(
                address,
                Account {
                    lamports: 10_000_000_000,
                    data: vec![],
                    owner: Address::default(),
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .unwrap();
        }
        let initialize = |payer| {
            ix(
                program,
                "initialize",
                &[],
                vec![
                    AccountMeta::new(payer, true),
                    AccountMeta::new(settings, false),
                    system.clone(),
                ],
            )
        };
        assert!(!send(&mut vm, intruder, initialize(intruder)));
        assert!(send(&mut vm, authority, initialize(authority)));
        assert!(!send(&mut vm, authority, initialize(authority)));
        let fund = ix(
            program,
            "fund",
            &1_001_000_000_u64.to_le_bytes(),
            vec![
                AccountMeta::new(authority, true),
                AccountMeta::new_readonly(settings, false),
                AccountMeta::new(reserve, false),
                system.clone(),
            ],
        );
        assert!(send(&mut vm, authority, fund));
        let payout = |payer, id: [u8; 16], to, amount: u64| {
            ix(
                program,
                "payout",
                &[id.as_slice(), &amount.to_le_bytes()].concat(),
                vec![
                    AccountMeta::new(payer, true),
                    AccountMeta::new_readonly(settings, false),
                    AccountMeta::new(reserve, false),
                    AccountMeta::new(to, false),
                    AccountMeta::new(
                        Address::find_program_address(&[b"receipt", &id], &program).0,
                        false,
                    ),
                    system.clone(),
                ],
            )
        };
        assert!(!send(
            &mut vm,
            intruder,
            payout(intruder, [1; 16], recipient, 100_000_000)
        ));
        assert!(send(
            &mut vm,
            authority,
            payout(authority, [1; 16], recipient, 100_000_000)
        ));
        assert_eq!(vm.get_balance(&recipient), Some(100_000_000));
        assert!(!send(
            &mut vm,
            authority,
            payout(authority, [1; 16], recipient, 100_000_000)
        ));
        assert!(!send(
            &mut vm,
            authority,
            payout(authority, [1; 16], intruder, 200_000_000)
        ));
        assert_eq!(vm.get_balance(&recipient), Some(100_000_000));
        let pause = |paused| {
            ix(
                program,
                "set_paused",
                &[u8::from(paused)],
                vec![
                    AccountMeta::new(authority, true),
                    AccountMeta::new(settings, false),
                ],
            )
        };
        assert!(send(&mut vm, authority, pause(true)));
        assert!(!send(
            &mut vm,
            authority,
            payout(authority, [2; 16], recipient, 100_000_000)
        ));
        assert!(
            vm.get_account(&Address::find_program_address(&[b"receipt", &[2; 16]], &program).0)
                .is_none()
        );
        assert!(send(&mut vm, authority, pause(false)));
        assert!(!send(
            &mut vm,
            authority,
            payout(authority, [2; 16], recipient, 500_000_001)
        ));
        assert!(send(
            &mut vm,
            authority,
            payout(authority, [2; 16], recipient, 500_000_000)
        ));
        assert!(!send(
            &mut vm,
            authority,
            payout(authority, [3; 16], recipient, 500_000_000)
        ));
        assert!(
            vm.get_account(&Address::find_program_address(&[b"receipt", &[3; 16]], &program).0)
                .is_none()
        );
        let receipt = vm
            .get_account(&Address::find_program_address(&[b"receipt", &[1; 16]], &program).0)
            .unwrap();
        assert_eq!(receipt.owner, program);
        assert_eq!(&receipt.data[24..56], recipient.as_ref());
        assert_eq!(
            u64::from_le_bytes(receipt.data[56..64].try_into().unwrap()),
            100_000_000
        );
    }
}
