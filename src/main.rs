use std::io;

struct Account {
    address: String,
    balance: u64,
}

impl Account {
    fn new(address: &str, balance: u64) -> Self {
        Self {
            address: address.to_string(),
            balance,
        }
    }
}

enum TransferError {
    ZeroAmount,
    SameAccount,
    InsufficientFunds,
    BalanceOverflow,
}

impl TransferError {
    fn message(&self) -> &str {
        match self {
            Self::ZeroAmount => "Сумма перевода должна быть больше нуля.",
            Self::SameAccount => "Нельзя переводить средства на тот же аккаунт.",
            Self::InsufficientFunds => "Недостаточно средств на балансе отправителя.",
            Self::BalanceOverflow => "Баланс получателя превысит допустимое значение.",
        }
    }
}

fn transfer(
    sender: &mut Account,
    recipient: &mut Account,
    amount: u64,
) -> Result<(), TransferError> {
    if amount == 0 {
        return Err(TransferError::ZeroAmount);
    }
    if sender.address == recipient.address {
        return Err(TransferError::SameAccount);
    }
    if sender.balance < amount {
        return Err(TransferError::InsufficientFunds);
    }

    let recipient_balance = recipient
        .balance
        .checked_add(amount)
        .ok_or(TransferError::BalanceOverflow)?;

    sender.balance -= amount;
    recipient.balance = recipient_balance;
    Ok(())
}

fn show_transfer(sender: &mut Account, recipient: &mut Account, amount: u64) {
    let sender_before = sender.balance;
    let recipient_before = recipient.balance;

    println!("Перевод: {} -> {}", sender.address, recipient.address);
    println!("Сумма перевода: {} SOL", amount);

    match transfer(sender, recipient, amount) {
        Ok(()) => {
            println!("Перевод выполнен успешно.");
            println!("Баланс отправителя: {} -> {} SOL", sender_before, sender.balance);
            println!("Баланс получателя: {} -> {} SOL", recipient_before, recipient.balance);
        }
        Err(error) => {
            println!("Ошибка: {}", error.message());
            println!("Балансы не изменились.");
        }
    }
    println!();
}

fn read_input() -> Option<String> {
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(0) => None,
        Ok(_) => Some(input.trim().to_string()),
        Err(error) => {
            eprintln!("Ошибка чтения ввода: {}", error);
            None
        }
    }
}

fn main() {
    let mut aidos = Account::new("Aidos", 10);
    let mut aliya = Account::new("Aliya", 2);

    loop {
        println!("Балансы: Aidos — {} SOL, Aliya — {} SOL", aidos.balance, aliya.balance);
        println!("1 — Aidos -> Aliya");
        println!("2 — Aliya -> Aidos");
        println!("0 — Выход");
        println!("Выберите действие:");

        let choice = match read_input() {
            Some(value) => value,
            None => break,
        };

        let (sender, recipient) = match choice.as_str() {
            "1" => (&mut aidos, &mut aliya),
            "2" => (&mut aliya, &mut aidos),
            "0" => break,
            _ => {
                println!("Ошибка: выберите 1, 2 или 0.\n");
                continue;
            }
        };

        println!("Введите сумму перевода в целых SOL:");
        let input = match read_input() {
            Some(value) => value,
            None => break,
        };

        let amount = match input.parse::<u64>() {
            Ok(value) => value,
            Err(_) => {
                println!("Ошибка: введите целое неотрицательное число в диапазоне u64.\n");
                continue;
            }
        };

        show_transfer(sender, recipient, amount);
    }
}
