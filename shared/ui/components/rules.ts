import { icon } from './icons';
import { connected, ledger } from '../bridge';
import { escape } from '../types';
import { field, modal, run, message } from './ui';
interface Rule{id:string;merchant:string;category:string;enabled:boolean}
export async function rules(host:HTMLElement){
 const values=connected?await ledger<Rule[]>('rules'):[];
 host.innerHTML=`<div class="section-heading"><div><h2>分类规则</h2><p>仅在你确认后，用商户名称补充分类。</p></div><button id="add-rule">添加规则</button></div>${values.map(r=>`<div class="setting-row"><span>${escape(r.merchant)} ${icon('arrowRight')}<span class="sr-only">归类为</span> ${escape(r.category)} · ${r.enabled?'已启用':'已暂停'}</span><button data-rule="${escape(r.id)}">撤销规则</button></div>`).join('')||'<p class="muted">还没有分类规则。</p>'}`;
 host.querySelector<HTMLButtonElement>('#add-rule')!.onclick=()=>modal('添加分类规则',field('商户名称','merchant')+field('分类','category'),'确认添加',async data=>{await ledger('learnRule',Object.fromEntries(data));await rules(host);message('分类规则已保存');});
 host.querySelectorAll<HTMLButtonElement>('[data-rule]').forEach(b=>b.onclick=()=>void run(async()=>{await ledger('revokeRule',{id:b.dataset.rule});await rules(host);message('分类规则已撤销');}));
}
