import { connected, desktop, ledger } from '../bridge';
import { escape, money, kinds, type Transaction } from '../types';
import { empty, run, modal, message } from './ui';
import { editTransaction } from './editor';
export async function transactionList(host: HTMLElement, shared: boolean, deleted = false, compact = false) {
  let page = 0, search = '', from = '', to = '', current:Transaction[] = [];
  host.innerHTML = `${compact ? '' : `<form class="searchbar"><label class="sr-only" for="search">搜索账单</label><input id="search" name="search" placeholder="搜索商户、分类或备注"><label class="sr-only" for="list-from">开始日期</label><input id="list-from" name="from" type="date" aria-label="开始日期"><label class="sr-only" for="list-to">结束日期</label><input id="list-to" name="to" type="date" aria-label="结束日期"><button>搜索</button><button type="button" id="clear-search">清除</button></form>`}<div class="rows"></div>${compact?'':'<div class="pager"><button id="previous">上一页</button><span id="page-label"></span><button id="next">下一页</button></div>'}`;
  const load = async () => {
    current = connected ? await ledger<Transaction[]>('list',{search,page,pageSize:compact?5:30,shared,deleted,...(from?{from}:{}),...(to?{to}:{})}) : [];
    host.querySelector('.rows')!.innerHTML = current.length ? current.map(t=>`<button class="transaction" data-id="${escape(t.id)}"><span class="transaction-icon">${t.kind==='income'?'↙':t.kind==='transfer'?'⇄':'↗'}</span><span class="transaction-info"><strong>${escape(t.merchant || kinds[t.kind])}</strong><small>${escape(t.category)} · ${escape(new Date(t.occurredAt).toLocaleString('zh-CN'))}${t.shared?' · 共同':''}</small></span><span class="amount ${t.kind==='income'?'income':''}">${escape(money(t.amountMinor,t.currencyCode))}<small>${escape(kinds[t.kind])}</small></span></button>`).join('') : empty(search?'没有找到匹配账单':deleted?'回收站是空的':'从第一笔开始','每一笔认真记录的小事，都会让生活更清楚。');
    if(!compact){ host.querySelector('#page-label')!.textContent=`第 ${page+1} 页`; (host.querySelector('#previous') as HTMLButtonElement).disabled = page===0; (host.querySelector('#next') as HTMLButtonElement).disabled=current.length<30; }
    host.querySelectorAll<HTMLButtonElement>('[data-id]').forEach(b=>b.onclick=()=>void run(()=>detail(current.find(t=>t.id===b.dataset.id)!)));
  };
  const detail = async (item:Transaction) => {
    const history = await ledger<unknown[]>('history',{id:item.id});
    modal(item.merchant || '账单详情',`<div class="detail-amount">${escape(money(item.amountMinor,item.currencyCode))}</div><p>${escape(kinds[item.kind])} · ${escape(item.category)} · ${escape(item.payerId)}</p><p>${escape(item.note || '暂无备注')}</p><p class="muted">账单编号：${escape(item.id)}</p><details><summary>修改历史（${history.length}）</summary><pre>${escape(JSON.stringify(history,null,2))}</pre></details><div class="detail-actions"><button type="button" id="edit-detail" ${deleted?'disabled':''}>编辑</button><button type="button" id="share-detail" ${deleted||item.shared?'disabled':''}>加入共同账本</button><button type="button" id="delete-detail">${deleted?'恢复账单':shared?'申请共同删除':'移到回收站'}</button></div>`, '完成',async()=>{});
    document.querySelector<HTMLButtonElement>('#edit-detail')!.onclick=()=> { document.querySelector('dialog')!.close(); void run(()=>editTransaction(load,item)); };
    document.querySelector<HTMLButtonElement>('#share-detail')!.onclick=()=>void run(async()=> { const button=document.querySelector<HTMLButtonElement>('#share-detail')!;button.disabled=true;try{await ledger('share',{id:item.id}); document.querySelector('dialog')!.close(); message('已加入共同账本'); await load();}finally{button.disabled=false;} });
    document.querySelector<HTMLButtonElement>('#delete-detail')!.onclick=()=>void run(async()=> { const button=document.querySelector<HTMLButtonElement>('#delete-detail')!;button.disabled=true;try{await ledger(deleted?'restore':shared?'requestSharedDelete':'delete',{id:item.id}); document.querySelector('dialog')!.close(); message(deleted?'账单已恢复':shared?'共同删除申请已提交':'账单已移入回收站'); await load();}finally{button.disabled=false;} });
  };
  host.querySelector('form')?.addEventListener('submit',e=>{e.preventDefault();const data=new FormData(e.currentTarget as HTMLFormElement);search=String(data.get('search'));from=String(data.get('from')||'');to=String(data.get('to')||'');page=0;void run(load);});
  host.querySelector('#clear-search')?.addEventListener('click',()=>{search='';from='';to='';page=0;host.querySelector('form')!.reset();void run(load);});
  host.querySelector('#previous')?.addEventListener('click',()=>{page--;void run(load);});host.querySelector('#next')?.addEventListener('click',()=>{page++;void run(load);});
  await load();
}
