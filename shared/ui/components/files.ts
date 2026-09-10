import { open, save } from '@tauri-apps/plugin-dialog';
import { ledger, system } from '../bridge';
import { escape } from '../types';
import { modal, field, message } from './ui';
export async function importFile(refresh:()=>Promise<void>) {
  const path = await open({multiple:false,filters:[{name:'账单文件',extensions:['csv','json']}]}); if(!path) return;
  const {text,name} = await system<{text:string;name?:string}>('readFile',{path});
  modal('确认导入文件', `<p>将使用标准账单模板导入。相同捕获来源会去重；不能识别的行保留错误。</p><p>文件长度：${text.length.toLocaleString()} 字符</p><label>内容预览<textarea readonly rows="8"></textarea></label>`, '开始导入',async()=>{
    const result = await ledger<Array<{status:string;message:string;row?:number}>>((name||path).toLowerCase().endsWith('.csv')?'importCsv':'importJson',{text});
    const counts:Record<string,number>={};result.forEach(r=>counts[r.status]=(counts[r.status]||0)+1);const labels:Record<string,string>={created:'新增',existing:'已存在',failed:'失败',duplicate:'疑似重复',needsConfirmation:'待确认',pending:'待确认'};message(`导入完成：${Object.entries(counts).map(([k,v])=>`${labels[k]||k} ${v} 笔`).join('，')}`);await refresh();const failures=result.map((r,i)=>({...r,row:r.row||i+1})).filter(r=>r.status==='failed');if(failures.length)setTimeout(()=>modal('导入失败的行',failures.map(r=>`<p>第 ${r.row} 行：${escape(r.message)}</p>`).join(''),'完成',async()=>{}),0);
  });
  document.querySelector<HTMLTextAreaElement>('dialog textarea')!.value=text.slice(0,8000);
}
export async function exportFile(format:'Csv'|'Json') {
  const path = await save({defaultPath:`轻账账单.${format.toLowerCase()}`,filters:[{name:format,extensions:[format.toLowerCase()]}]});if(!path)return;
  const {text}=await ledger<{text:string}>(`export${format}`);await system('saveFile',{path,text});message('账单已导出到所选文件');
}
export async function backup(restore:boolean,refresh:()=>Promise<void>) {
  const path=restore?await open({multiple:false,filters:[{name:'轻账加密备份',extensions:['qaccount']}]}):await save({defaultPath:'轻账备份.qaccount'});if(!path)return;
  modal(restore?'恢复加密备份':'创建加密备份',`<p>${restore?'将验证备份后恢复账本，请先导出当前备份。':'使用独立恢复密码。请妥善保存，恢复时需要输入相同密码。'}</p>${field('恢复密码','password','','password')}`,'确认',async data=>{await ledger(restore?'restoreBackup':'exportBackup',{path,password:data.get('password')});message(restore?'备份已恢复':'加密备份已创建');await refresh();});
}
