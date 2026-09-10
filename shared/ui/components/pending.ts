import { connected, ledger } from '../bridge';
import { escape, type Pending, type Transaction } from '../types';
import { empty, run, message, modal } from './ui';
import { editTransaction } from './editor';
const names:Record<string,string>={duplicate:'疑似重复',sharedDelete:'共同删除申请',coreModification:'账单修改申请',confirmation:'需要补充信息',conflict:'同步字段冲突',difference:'识别结果与人工修改不同'};
const fields:Record<string,string>={amountMinor:'金额（最小货币单位）',occurredAt:'发生时间',payerId:'付款人',accountId:'付款账户',merchant:'商户',category:'分类',note:'备注',kind:'收支类型',currencyCode:'币种'};
export async function pending(host:HTMLElement) {
 const items=connected?await ledger<Pending[]>('pending'):[];
 host.innerHTML=items.length?items.map(p=>`<section class="card" data-id="${escape(p.id)}"><h2>${escape(names[p.kind]||'需要核对的事项')}</h2><p>${p.kind==='confirmation'?'补充付款账户和缺失信息后入账。':p.kind==='conflict'||p.kind==='difference'?'逐项选择保留本机内容或采用新内容。':'请核对关联账单后处理。'}</p><details><summary>查看原始记录</summary><pre>${escape(typeof p.payload==='string'?p.payload:JSON.stringify(p.payload,null,2))}</pre></details><div class="actions">${p.kind==='duplicate'?'<button data-action="merge">合并为同一笔</button><button data-action="keep">保留为新交易</button>':['sharedDelete','coreModification'].includes(p.kind)?'<button data-action="approve">同意申请</button><button data-action="reject">拒绝申请</button>':'<button data-action="resolve">核对并处理</button>'}</div></section>`).join(''):empty('都处理好了','待确认账单、重复项和共同账本申请会出现在这里。');
 host.querySelectorAll<HTMLButtonElement>('[data-action]').forEach(b=>b.onclick=()=>void run(async()=>{
  const action=b.dataset.action!,id=b.closest<HTMLElement>('[data-id]')!.dataset.id!,item=items.find(p=>p.id===id)!;
  const payload=typeof item.payload==='string'?JSON.parse(item.payload):item.payload;
  if(action==='resolve'){
   if(item.kind==='confirmation'){await editTransaction(()=>pending(host),payload.draft as Partial<Transaction>,payload.missingFields, draft=>ledger('confirmCandidate',{id,draft}));return;}
   if(['conflict','difference'].includes(item.kind)){
    const entries=Object.entries(payload.fields) as [string,{local:unknown;remote:unknown}][];
    modal('选择每个字段要保留的内容',entries.map(([key,value],i)=>`<label>${escape(fields[key]||key)}<select name="choice-${i}"><option value="local">保留本机：${escape(value.local)}</option><option value="remote">采用新值：${escape(value.remote)}</option></select></label>`).join(''),'保存选择',async data=>{await ledger(item.kind==='conflict'?'resolveConflict':'resolveDifference',{id,choices:Object.fromEntries(entries.map(([key],i)=>[key,data.get(`choice-${i}`)]))});await pending(host);message('字段选择已保存');});return;
   }
   throw Error('当前版本暂不能处理此类事项，请保留记录并更新应用。');
  }
  b.disabled=true;try{await ledger(['merge','keep'].includes(action)?'resolveDuplicate':item.kind==='coreModification'?'approveModification':'approveSharedDelete',{id,merge:action==='merge',approve:action==='approve'});message('处理结果已保存');await pending(host);}finally{b.disabled=false;}
 }));
}
