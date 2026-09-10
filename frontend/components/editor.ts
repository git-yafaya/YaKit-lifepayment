import { ledger } from '../bridge';
import { escape, kinds, minor, type Transaction, type Account, type Member } from '../types';
import { modal, field, message } from './ui';
export async function editTransaction(refresh: () => Promise<void>, item?: Partial<Transaction>, missing: string[] = [], saveDraft?: (draft:object)=>Promise<unknown>) {
  const [accounts,members]=await Promise.all([ledger<Account[]>('accounts'),ledger<Member[]>('members')]);
  const payer=item?.payerId&&item.payerId!=='local'?item.payerId:members.find(m=>m.displayName==='我')?.memberId;
  const available=members.filter(m=>m.active||m.memberId===payer);
  const payerOptions=available.map((m,i)=>`<option value="${escape(m.memberId)}" ${m.memberId===payer?'selected':''}>${escape(m.displayName||`共同成员 ${i+1}`)}${m.active?'':'（历史成员）'}</option>`).join('')+(payer&&!available.some(m=>m.memberId===payer)?`<option value="${escape(payer)}" selected>原付款人（历史记录）</option>`:'');
  const currency = item?.currencyCode || 'CNY';
  const amount = item?.amountMinor ? (currency === 'JPY' ? item.amountMinor : `${BigInt(item.amountMinor) / 100n}.${String(BigInt(item.amountMinor) % 100n).padStart(2,'0')}`) : '';
  const date = new Date(item?.occurredAt || Date.now()); date.setMinutes(date.getMinutes()-date.getTimezoneOffset());
  modal(item?.id ? '账单详情' : '记下一笔', `${missing.length ? `<p class="notice">还需要确认：${escape(missing.map(k=>({amountMinor:'金额',kind:'收支类型',payerId:'付款人',accountId:'付款账户',occurredAt:'发生时间'}[k]||k)).join('、'))}</p>` : ''}<div class="form-grid"><label>类型<select name="kind">${Object.entries(kinds).map(([v,l])=>`<option value="${v}" ${v === item?.kind ? 'selected' : ''}>${l}</option>`).join('')}</select></label><label>币种<select name="currencyCode">${['CNY','JPY','USD','EUR'].map(c=>`<option ${c===currency?'selected':''}>${c}</option>`).join('')}</select></label>${field('金额','amount',amount)}${field('发生时间','occurredAt',date.toISOString().slice(0,16),'datetime-local')}${field('商户 / 用途','merchant',item?.merchant || '')}${field('分类','category',item?.category || '其他')}<label>付款账户<select name="accountId" required><option value="">请选择</option>${accounts.map(a=>`<option value="${escape(a.id)}" ${a.id===item?.accountId?'selected':''}>${escape(a.name)}</option>`).join('')}</select></label><label>付款人<select name="payerId" required><option value="">请选择付款人</option>${payerOptions}</select></label>${field('关联原账单（退款必填）','originalTransactionId',item?.originalTransactionId || '', 'text',false)}</div><label>备注<textarea name="note" rows="3">${escape(item?.note || '')}</textarea></label>`, '保存账单',async data=>{
    const values = Object.fromEntries(data); const draft={...values,id:item?.id,captureId:item?.captureId,transactionKey:item?.transactionKey,amountMinor:minor(String(data.get('amount')),String(data.get('currencyCode'))),occurredAt:new Date(String(data.get('occurredAt'))).toISOString()};const result = (saveDraft?await saveDraft(draft):await ledger(item?.id ? 'update' : 'create',draft)) as {message?:string;pendingId?:string};
    message(result?.message || (result?.pendingId?'修改申请已提交，等待所有者确认':'账单已保存')); await refresh();
  });
}
export function textEntry(refresh: () => Promise<void>) {
  modal('告诉轻账，发生了什么', `<p class="muted">例如：午饭支出 28 元。解析后由你确认金额、时间和付款账户。</p><label>记账内容<textarea name="text" rows="4" required placeholder="今天午饭支出 28 元"></textarea></label>`, '识别并补充',async data=> {
    const result = await ledger<{draft:Partial<Transaction>;missingFields:string[]}>('parseText',{text:data.get('text')});
    // 等当前对话框关闭后再打开确认表单，保持键盘焦点在唯一弹窗中。
    setTimeout(()=>void editTransaction(refresh,result.draft,result.missingFields).catch(e=>message(String(e),true)),0);
  });
}
