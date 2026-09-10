import { desktop, system } from '../bridge';
import { escape } from '../types';
import { message } from './ui';
interface SyncResult { at?:number;pausedSpaces?:string[];results?:Array<{spaceId:string;error?:string;result?:{errors?:Array<{error:string}>}}> }
interface SyncStatus {configured:boolean;paused:boolean;lastSync?:SyncResult}
function outcome(result:SyncResult){
 const spaces=result.results||[];
 const errors=spaces.flatMap(s=>s.error?[s.error]:(s.result?.errors||[]).map(e=>e.error));
 const success=spaces.filter(s=>!s.error&&!s.result?.errors?.length).length;
 return {errors,text:spaces.length?`已完成 ${success} 个账本空间${errors.length?`，${spaces.length-success} 个空间需要处理`:'，全部同步成功'}`:'尚无同步结果'};
}
export async function syncStatus(host:HTMLElement){
 if(!desktop){host.innerHTML='<p class="muted">请在轻账应用中配置和查看同步。</p>';return;}
 try{
  const state=await system<SyncStatus>('syncStatus');const last=state.lastSync;
  const paused=state.paused||Boolean(last?.pausedSpaces?.length);
  host.innerHTML=`<div class="setting-row"><strong>${!state.configured?'尚未配置同步':paused?'需要处理：部分或全部同步已暂停':'自动同步已开启'}</strong><span>${last?.at?`上次同步：${escape(new Date(last.at*1000).toLocaleString('zh-CN'))}`:'尚未同步'}</span></div>${paused?'<p class="notice">请查看错误并处理后，点击“立即同步”重新尝试。</p>':''}${last?`<p class="muted">${escape(outcome(last).text)}</p>${outcome(last).errors.map(e=>`<p class="notice">${escape(e)}</p>`).join('')}<details><summary>同步详情</summary><pre>${escape(JSON.stringify(last,null,2))}</pre></details>`:''}`;
 }catch(error){host.innerHTML=`<p class="notice">无法读取同步状态：${escape(error)}</p>`;}
}
export async function syncNow(host:HTMLElement){
 const result=await system<SyncResult>('syncRun');const value=outcome(result);message(value.text,value.errors.length>0);await syncStatus(host);
}
