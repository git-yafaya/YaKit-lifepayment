import { connected, ledger } from '../bridge';
import { escape, money, type Summary, type Member } from '../types';
import { empty, run } from './ui';
interface Point { label:string;amountMinor:string;currencyCode?:string }
interface Analysis { monthly:Point[];categories:Point[];payers:Point[] }
export async function analysis(host:HTMLElement,shared:boolean){
 host.innerHTML='<section class="card"><form class="analysis-filter"><label>开始日期<input name="from" type="date"></label><label>结束日期<input name="to" type="date"></label><button type="submit">查看分析</button></form></section><div id="filtered-summary"></div><div id="charts"></div>';
 const members=connected?await ledger<Member[]>('members'):[];
 const memberNames=new Map(members.map((m,i)=>[m.memberId,m.displayName||`共同成员 ${i+1}`]));
 const load=async(payload:object={})=>{
  const summaries=connected?await ledger<Summary[]>('summary',{shared,...payload}):[];host.querySelector('#filtered-summary')!.innerHTML=summaries.map(s=>`<section class="card"><h2>${escape(s.currencyCode)} · 当前范围</h2><div class="setting-row"><span>净支出</span><strong>${escape(money((BigInt(s.expenseMinor)-BigInt(s.refundMinor)).toString(),s.currencyCode))}</strong></div><div class="setting-row"><span>收入</span><strong>${escape(money(s.incomeMinor,s.currencyCode))}</strong></div><p>${s.count} 笔账单</p></section>`).join('');
  const data=connected?await ledger<Analysis>('analysis',{shared,...payload}):{monthly:[],categories:[],payers:[]};
  host.querySelector('#charts')!.innerHTML=([['monthly','每月净支出'],['categories','分类支出'],['payers','付款人支出']] as const).map(([key,title])=>`<section class="card"><h2>${title}</h2>${data[key].length?data[key].map(p=>`<div class="setting-row"><span>${escape(key==='payers'?(memberNames.get(p.label)||'历史付款人'):p.label)}</span><strong>${escape(money(p.amountMinor,p.currencyCode || 'CNY'))}</strong></div>`).join(''):empty('还没有可分析的账单','改变日期范围，或先记下第一笔。')}</section>`).join('');
 };
 host.querySelector('form')!.onsubmit=e=>{e.preventDefault();const values=Object.fromEntries(new FormData(e.currentTarget as HTMLFormElement));void run(()=>load(Object.fromEntries(Object.entries(values).filter(([,v])=>v))));};await load();
}
