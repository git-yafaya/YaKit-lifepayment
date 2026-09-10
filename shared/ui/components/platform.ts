import { icon } from './icons';
import { open } from '@tauri-apps/plugin-dialog';
import { desktop, ledger, system } from '../bridge';
import { escape, type Transaction } from '../types';
import { editTransaction } from './editor';
import { message, modal, run } from './ui';
interface Preferences{appLockEnabled:boolean;notificationSources:string[]}
interface Capabilities{platform?:string;packaged:boolean;ocr:{available:boolean;reason:string};notifications:{available:boolean;access:string;reason?:string};hello:{available:boolean;reason?:string}}
// 只调整入口的可用状态，识别和保存仍由原有流程处理。
export async function imageImportAvailability(button:HTMLButtonElement){
 if(!desktop)return;
 const caps=await system<Capabilities>('capabilities');
 if(caps.ocr?.available)return;
 button.disabled=true;
 const note=document.createElement('p');note.id='image-import-note';note.textContent=caps.ocr?.reason||'此设备暂不支持截图识别，请使用文字记账或导入账单。';
 button.setAttribute('aria-describedby',note.id);button.parentElement!.after(note);
}
export async function imageImport(refresh:()=>Promise<void>){
 const path=await open({multiple:false,filters:[{name:'账单截图',extensions:['png','jpg','jpeg','bmp','tif','tiff']}]});if(!path)return;
 const result=await system<{text:string;captureId?:string}>('ocrImage',{path});
 modal('核对图片识别文字','<p>确认原文后，再补充或修正金额、时间和付款账户。</p><label>识别内容<textarea name="text" rows="8" required></textarea></label>','继续确认账单',async data=>{const parsed=await ledger<{draft:Partial<Transaction>;missingFields:string[]}>('parseText',{text:data.get('text')});setTimeout(()=>void run(()=>editTransaction(refresh,{...parsed.draft,captureId:result.captureId},parsed.missingFields)),0);});document.querySelector<HTMLTextAreaElement>('dialog textarea')!.value=result.text;
}
export async function privacy(host:HTMLElement,refresh:()=>Promise<void>){
 if(!desktop){host.innerHTML='<h2>本机能力</h2><p class="muted">通知、截图识别和应用锁需要在轻账应用中查看设备支持情况。</p>';return;}
 const caps=await system<Capabilities>('capabilities');const prefs=await system<Preferences>('deviceSettings');const android=caps.platform==='android';
 host.innerHTML=`<h2>隐私与自动录入</h2><div class="setting-row"><div><strong>截图识别</strong><p>${caps.ocr?.available?'使用本机中文识别组件，图片不会上传。':escape(caps.ocr?.reason||'此设备暂不可用')}</p></div><button id="ocr" ${caps.ocr?.available?'':'disabled'}>导入截图</button></div><div class="setting-row"><div><strong>通知录入</strong><p>${caps.notifications?.available?'仅主动读取你选择的来源，不清除系统通知。':escape(caps.notifications?.reason||'此设备暂不支持通知录入。')}</p></div><button id="notifications" ${caps.notifications?.available?'':'disabled'}>授权与选择来源</button></div><div class="setting-row"><div><strong>应用锁</strong><p>${caps.hello?.available?(android?'验证身份后打开账本。':'用 Windows Hello 验证后打开账本。'):escape(caps.hello?.reason||(android?'Android 应用锁尚未接入。':'请先在 Windows 设置中配置 PIN 或 Windows Hello。'))}</p></div><button id="lock-enable" ${caps.hello?.available?'':'disabled'}>${prefs.appLockEnabled?'关闭应用锁':'开启应用锁'}</button></div>${prefs.appLockEnabled?'<button id="lock-now">立即锁定</button>':''}${android?'':'<div class="setting-row"><div><strong>关闭窗口后退出</strong><p>后台保留与开机启动未开启。</p></div><span class="pill">默认</span></div>'}`;
 host.querySelector<HTMLButtonElement>('#ocr')!.onclick=()=>void run(()=>imageImport(refresh));host.querySelector<HTMLButtonElement>('#notifications')!.onclick=()=>void run(()=>notifications(refresh,prefs));
 host.querySelector<HTMLButtonElement>('#lock-enable')!.onclick=()=>void run(async()=>{await system('helloVerify');await system('saveDeviceSettings',{...prefs,appLockEnabled:!prefs.appLockEnabled});await privacy(host,refresh);message(prefs.appLockEnabled?'应用锁已关闭':'应用锁已开启，下次打开应用需要验证');});
 host.querySelector<HTMLButtonElement>('#lock-now')?.addEventListener('click',lock);
}
async function notifications(refresh:()=>Promise<void>,prefs:Preferences){
 const access=await system<{allowed:boolean}>('notificationRequestAccess');if(!access.allowed)throw Error('通知访问尚未授权。可以在 Windows 设置中允许后重试。');
 const sources=await system<Array<{id:string;name:string}>>('notificationSources');
 modal('选择允许读取的通知来源',`<p>来源来自当前通知中心。选择后只读取这些应用的现有通知。</p>${sources.length?sources.map(s=>`<label class="check-row"><input type="checkbox" name="sources" value="${escape(s.id)}" ${prefs.notificationSources.includes(s.id)?'checked':''}>${escape(s.name)}</label>`).join(''):'<p class="notice">当前没有可选来源。等待应用产生通知后再试。</p>'}`,'保存选择并读取',async data=>{
  const selected=data.getAll('sources').map(String);if(!selected.length)throw Error('请至少选择一个来源');await system('saveDeviceSettings',{...prefs,notificationSources:selected});
  const result=await system<{notifications:Array<{text:string;source:string;captureId:string}>;note:string}>('notificationRead',{sources:selected});
  setTimeout(()=>{modal('选择要录入的通知',`<p>${escape(result.note)}</p><label>通知<select name="index">${result.notifications.map((n,i)=>`<option value="${i}">${escape(n.text.slice(0,100))}</option>`).join('')}</select></label>`,'核对账单',async values=>{const item=result.notifications[Number(values.get('index'))];if(!item)throw Error('没有可录入的通知');const parsed=await ledger<{draft:Partial<Transaction>;missingFields:string[]}>('parseText',{text:item.text});setTimeout(()=>void run(()=>editTransaction(refresh,{...parsed.draft,captureId:item.captureId},parsed.missingFields)),0);});},0);
 });
}
export function lock(){if(document.querySelector('#lock-screen'))return;const app=document.querySelector<HTMLElement>('#app')!;app.inert=true;app.hidden=true;const overlay=document.createElement('section');overlay.id='lock-screen';overlay.innerHTML=`<span class="brand-icon">${icon('ledger')}</span><h1>你的账本，安心收好。</h1><p>使用 Windows Hello 验证身份后继续。</p><button class="primary">解锁账本</button><p role="alert"></p>`;document.body.append(overlay);overlay.querySelector('button')!.focus();overlay.querySelector('button')!.onclick=async()=>{const button=overlay.querySelector('button')!;button.disabled=true;try{await system('helloVerify');overlay.remove();app.hidden=false;app.inert=false;document.querySelector<HTMLButtonElement>('#quick-add')!.focus();}catch(e){overlay.querySelector('[role=alert]')!.textContent=String(e);}finally{button.disabled=false;}};}
export async function initialLock(){if(desktop){const prefs=await system<Preferences>('deviceSettings');if(prefs.appLockEnabled)lock();}}
