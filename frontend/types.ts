export interface Transaction { id: string; ownerId: string; amountMinor: string; currencyCode: string; kind: string; occurredAt: string; merchant: string; category: string; accountId: string; payerId: string; note: string; deleted: boolean; shared: boolean; originalTransactionId?: string; captureId?: string; transactionKey?: string }
export interface Summary { currencyCode: string; expenseMinor: string; incomeMinor: string; refundMinor: string; count: number }
export interface Pending { id: string; kind: string; transactionId: string; payload: string; requestedBy: string }
export interface Member { memberId:string; displayName?:string; active:boolean }
export interface Account { id: string; name: string }
export const kinds: Record<string,string> = { expense: '支出', income: '收入', transfer: '转账', refund: '退款' };
export function money(value: string, currency = 'CNY'): string {
  // 金额只用整数处理，避免大额账单在界面丢失精度。
  const n = BigInt(value || '0'), digits = currency === 'JPY' ? 0 : 2;
  const sign = n < 0n ? '-' : '', v = n < 0n ? -n : n;
  return `${currency} ${sign}${digits ? `${v / 100n}.${String(v % 100n).padStart(2,'0')}` : v}`;
}
export function minor(value: string, currency: string): string {
  const digits = currency === 'JPY' ? 0 : 2;
  if (!new RegExp(`^\\d+(?:\\.\\d{1,${digits || 1}})?$`).test(value) || (!digits && value.includes('.'))) throw Error('请输入有效金额，金额精度与币种一致');
  const [whole, fraction = ''] = value.split('.');
  return (BigInt(whole) * (digits ? 100n : 1n) + BigInt(fraction.padEnd(digits,'0') || '0')).toString();
}
export const escape = (s: unknown) => String(s ?? '').replace(/[&<>"']/g,c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));
