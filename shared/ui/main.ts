import './style.css';
import { connected, desktop, ledger } from './bridge';
import { money, escape, type Summary, type Pending } from './types';
import { run, empty, message } from './components/ui';
import { textEntry, editTransaction } from './components/editor';
import { transactionList } from './components/transactions';
import { settings } from './components/settings';
import { pending } from './components/pending';
import { analysis } from './components/analysis';
import { initialLock, imageImport, imageImportAvailability } from './components/platform';
import { importFile } from './components/files';

const routes=[['home','⌂','首页'],['transactions','☷','账单'],['analysis','◴','分析'],['pending','◷','待处理'],['recycle','↶','回收站'],['settings','⚙','设置']];
let route='home',shared=false,revision=0;
document.querySelector('#app')!.innerHTML=`<aside class="sidebar"><a href="#home" class="brand"><span class="brand-icon">轻</span><span>轻账<small>LIGHT LEDGER</small></span></a><div class="sidebar-caption">我的日常</div><nav aria-label="主导航">${routes.map(([id,icon,label])=>`<button data-route="${id}"><span aria-hidden="true">${icon}</span><span class="nav-label">${label}</span>${id==='pending'?'<span id="pending-count" class="count" hidden></span>':''}</button>`).join('')}</nav><div class="sidebar-bottom"><span class="local-dot"></span> 本地优先 · 安心记录<small>让生活有数，让日子轻一点。</small></div></aside><div class="workspace"><header class="topbar"><div class="breadcrumb">我的账本 <span>/</span> <strong id="route-name">首页</strong></div><div class="scope" role="group" aria-label="账本范围"><button id="mine" aria-pressed="true">我的</button><button id="ours" aria-pressed="false">我们</button></div><button class="primary" id="quick-add"><span>＋</span> 记一笔</button></header>${desktop?'':`<div class="preview-banner">${connected?'开发验收 · 已连接临时账本，系统能力需要在本机应用中使用。':'界面预览 · 未连接本地账本，请在轻账应用中操作。'}</div>`}<main id="main"></main><div id="status" role="status" aria-live="polite" hidden></div><footer class="app-footer"><span>轻账 · 为每一份小日常</span><span>${desktop?'本机账本':'浏览器界面预览'}</span></footer></div>`;
const main=document.querySelector<HTMLElement>('#main')!;
async function refresh(){await navigate(route);}
async function navigate(next:string){route=next;const revisionNow=++revision;document.querySelector('#route-name')!.textContent=routes.find(r=>r[0]===route)![2];document.querySelectorAll<HTMLElement>('[data-route]').forEach(b=>{b.classList.toggle('active',b.dataset.route===route);b.setAttribute('aria-current',b.dataset.route===route?'page':'false');});
 main.innerHTML='<div class="loading" role="status">正在打开账本…</div>';
 const heading=(title:string,subtitle:string)=>`<div class="page-heading"><div><p class="eyebrow">${shared?'OUR LITTLE LIFE':'YOUR EVERYDAY, CLEARER'}</p><h1>${title}</h1><p>${subtitle}</p></div></div>`;
 try{
  if(route==='home'||route==='analysis'){
   const summaries=connected?await ledger<Summary[]>('summary',{shared}):[];if(revisionNow!==revision)return;
   const pendingItems=connected?await ledger<Pending[]>('pending'):[];const badge=document.querySelector<HTMLElement>('#pending-count')!;badge.textContent=String(pendingItems.length);badge.hidden=!pendingItems.length;
   if(route==='home'){
    main.innerHTML=heading(shared?'一起，把日子过清楚。':'把日子，记得轻一点。','每一笔小小的记录，都是认真生活的证据。')+`<section class="assistant-card"><div class="assistant-icon" aria-hidden="true">✳</div><div><div class="assistant-label">你的记账小助手 <span>本地账本</span></div><h2>${pendingItems.length?`有 ${pendingItems.length} 件事，等你确认。`:'今天，也从容一点。'}</h2><p>${summaries.length?`账本已经记录 ${summaries.reduce((n,s)=>n+s.count,0)} 笔日常，随时回来看看。`:'还没有账单。记下第一笔，或导入已有的账单文件。'}</p><div class="actions"><button class="primary" id="natural">说一句，记一笔 <span>↗</span></button><button id="import">导入账单</button><button id="import-image">导入截图</button></div></div></section><div class="stats">${summaryCards(summaries)}</div><section class="card recent"><div class="section-heading"><div><h2>最近的日常</h2><p>小事值得被记住。</p></div><button class="text-button" id="all-transactions">查看全部 →</button></div><div id="list"></div></section>`;
    void run(()=>imageImportAvailability(main.querySelector<HTMLButtonElement>('#import-image')!));
    main.querySelector<HTMLButtonElement>('#natural')!.onclick=()=>textEntry(refresh);main.querySelector<HTMLButtonElement>('#import')!.onclick=()=>void run(()=>importFile(refresh));main.querySelector<HTMLButtonElement>('#import-image')!.onclick=()=>void run(()=>imageImport(refresh));main.querySelector<HTMLButtonElement>('#all-transactions')!.onclick=()=>void run(()=>navigate('transactions'));await transactionList(main.querySelector('#list')!,shared,false,true);
   }else{main.innerHTML=heading('看看钱，去了哪里。','按时间、分类和付款人查看净支出，转账不计入收支。')+'<div id="analysis"></div>';await analysis(main.querySelector('#analysis')!,shared);}
  }else if(route==='transactions'||route==='recycle'){main.innerHTML=heading(route==='recycle'?'给误删，留一次反悔。':'每一笔，都有迹可循。',route==='recycle'?'恢复需要保留的账单。':'搜索商户、分类或备注，打开账单查看和修改。')+'<section class="card" id="list"></section>';await transactionList(main.querySelector('#list')!,shared,route==='recycle');}
  else if(route==='pending'){main.innerHTML=heading('一点确认，多一份清楚。','核对疑似重复与共同账本申请。')+'<div id="pending-list"></div>';await pending(main.querySelector('#pending-list')!);}
  else{main.innerHTML=heading('按照你的习惯。','管理账户、设备、同步与自己的数据。')+'<div class="settings-grid" id="settings"></div>';await settings(main.querySelector('#settings')!,refresh);}
  if(shared && ['home','transactions','analysis'].includes(route)){const p=document.createElement('p');p.className='scope-note';p.textContent='这里展示明确加入共同账本的记录；在账单详情中可以逐笔共享。';main.prepend(p);}
 }catch(error){main.innerHTML=heading('账本暂时无法打开','请检查本地数据服务后重试。')+'<button id="retry">重新加载</button>';main.querySelector<HTMLButtonElement>('#retry')!.onclick=()=>void run(refresh);message(error instanceof Error?error.message:String(error),true);}
}
function summaryCards(summaries:Summary[]){const s=summaries[0];return `<div class="stat"><p>净支出 <span>累计</span></p><strong>${s?escape(money((BigInt(s.expenseMinor)-BigInt(s.refundMinor)).toString(),s.currencyCode)):'—'}</strong><small>原始支出减去关联退款</small></div><div class="stat"><p>收入 <span>累计</span></p><strong class="income">${s?escape(money(s.incomeMinor,s.currencyCode)):'—'}</strong><small>每一份收获都算数</small></div><div class="stat"><p>记录下的日常</p><strong>${summaries.reduce((n,s)=>n+s.count,0)} <em>笔</em></strong><small>${summaries.length>1?'存在多个币种，请在分析中分别查看':'转账独立保留，不混入收支'}</small></div>`;}
document.querySelectorAll<HTMLButtonElement>('[data-route]').forEach(b=>b.onclick=()=>void run(()=>navigate(b.dataset.route!)));
document.querySelector<HTMLButtonElement>('#quick-add')!.onclick=()=>void run(()=>editTransaction(refresh));
for(const id of ['mine','ours'])document.querySelector<HTMLButtonElement>(`#${id}`)!.onclick=()=>{shared=id==='ours';document.querySelector('#mine')!.setAttribute('aria-pressed',String(!shared));document.querySelector('#ours')!.setAttribute('aria-pressed',String(shared));void run(refresh);};
document.addEventListener('keydown',e=>{if((e.ctrlKey||e.metaKey)&&e.key==='n'){e.preventDefault();if(!document.querySelector('dialog')&&!document.querySelector('#lock-screen'))void run(()=>editTransaction(refresh));}});
void run(async()=>{await initialLock();await refresh();});
