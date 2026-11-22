import type { Rewards } from "./api";

export function devnetMessage(account: boolean, rewards?: Rewards): string {
  if (!account) return "Гостевой режим: демо-награды сохраняются на устройстве. Для выплат войдите через Google и привяжите личный кошелёк.";
  if (!rewards) return "Загружаем состояние наград и кошелька с сервера.";
  if (rewards.wallet_reconfirmation_required) return "Прежний кошелёк сохранён. После восстановления аккаунта подтвердите владение новой подписью, чтобы возобновить выплаты.";
  if (!rewards.wallet_address) return "Привяжите личный кошелёк в кабинете. Начисленные награды будут ожидать привязки.";
  if (!rewards.solana_payments_enabled) return "Кошелёк привязан. Автоматические выплаты пока не включены на сервере.";
  return "Кошелёк привязан. Награды отправляются автоматически через программу Solana Devnet. Статус каждой выплаты доступен в истории.";
}
