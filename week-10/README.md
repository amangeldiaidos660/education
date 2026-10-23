# Неделя 10 — CPI и вывод из vault

[Транзакция вывода в Explorer (devnet)](https://explorer.solana.com/tx/5j2SPPtaQPKLao12WUfvDvaM5cyNPsMt4qMBwQBzBYiSqqubUHbDwREbu7QLcxornRWtmoqwnMKEei6qViZDQabL?cluster=devnet)

Внутри инструкции Withdraw видны два CPI-вызова SPL Token Program:

1. Burn — сжигает 100 share-токенов пользователя. Authority — пользователь, разрешивший операцию своей подписью.
2. Transfer (Checked) — переводит 110 токенов из хранилища vault пользователю. Authority — PDA vault; программа подтверждает её полномочия через invoke_signed.

Оба действия выполнены в одной успешной транзакции.
