import { open, save } from '@tauri-apps/plugin-dialog';
import { system } from '../bridge';
import { escape } from '../types';
import { modal, field, message } from './ui';
interface Space {id:string;kind:string;epoch:number;devices:Record<string,unknown>}
interface Status{configured:boolean;deviceId:string;memberId:string;fingerprint:string;spaces:Space[]}
async function readInvite(){const path=await open({multiple:false,filters:[{name:'轻账邀请文件',extensions:['json']}]});if(!path)throw Error('未选择邀请文件');return JSON.parse((await system<{text:string}>('readFile',{path})).text);}
async function outputPath(name:string){const path=await save({defaultPath:name,filters:[{name:'轻账邀请文件',extensions:['json']}]});if(!path)throw Error('未选择保存位置');return path;}
async function writeInvite(value:unknown,path:string){try{await system('saveFile',{path,text:JSON.stringify(value,null,2)});}catch(error){setTimeout(()=>{modal('文件保存失败，请保留这份结果','<p>操作已经完成。请复制下方内容保存为 JSON 文件，再交给另一台设备，不要重复批准。</p><textarea readonly rows="10"></textarea>','已保存结果',async()=>{});document.querySelector<HTMLTextAreaElement>('dialog:last-of-type textarea')!.value=JSON.stringify(value,null,2);},0);throw error;}}
export async function devices(){
 const status=await system<Status>('syncStatus');
 modal('把账本带到另一台设备',`<p>个人设备只连接你自己的账本；邀请共同成员只授予所选共同账本。</p><label>你想做什么？<select name="operation"><option value="exportInfo">第一步：导出本机个人账本信息</option><option value="personalRequest">新个人设备：读取旧设备信息并生成请求</option><option value="sharedRequest">共同成员：生成加入请求</option><option value="pairingApprove">可信设备：读取并批准加入请求</option><option value="pairingAccept">新设备：读取批准文件，完成加入</option><option value="createSharedSpace">创建我们的共同账本</option><option value="revokeDevice">移除一台已配对设备</option><option value="applyRotation">接收设备移除后的更新文件</option></select></label><label>要共享的账本<select name="spaceId">${status.spaces.map(s=>`<option value="${escape(s.id)}">${s.kind==='personal'?'我的个人账本':'我们的共同账本'} · ${escape(s.id.slice(0,8))}</option>`).join('')}</select></label>${field('与对方屏幕核对的设备指纹（批准 / 加入时必填）','fingerprint','','text',false)}<label>要移除的设备<select name="deviceId"><option value="">请选择</option>${[...new Set(status.spaces.flatMap(s=>Object.keys(s.devices)))].filter(id=>id!==status.deviceId).map(id=>`<option value="${escape(id)}">设备 ${escape(id.slice(0,8))}</option>`).join('')}</select></label><details><summary>本机指纹与技术信息</summary><pre>${escape(status.fingerprint)}</pre><pre>${escape(JSON.stringify(status.spaces,null,2))}</pre></details>`,'继续',async data=>{
  const action=String(data.get('operation'));let result:unknown;
  if(action==='exportInfo'){await writeInvite({memberId:status.memberId,fingerprint:status.fingerprint},await outputPath('轻账-个人账本信息.json'));message('已导出账本信息，请在新设备选择此文件');return;}
  if(action==='personalRequest'){const info=await readInvite();const path=await outputPath('轻账-个人设备加入请求.json');result=await system('pairingRequest',{memberId:info.memberId});await writeInvite(result,path);}
  else if(action==='sharedRequest'){const path=await outputPath('轻账-共同成员加入请求.json');result=await system('pairingRequest');await writeInvite(result,path);}
  else if(action==='pairingApprove'||action==='pairingAccept'||action==='applyRotation'){
   const document=await readInvite();const path=action==='pairingApprove'?await outputPath('轻账-设备加入批准.json'):undefined;result=await system(action,{request:document,grant:document,fingerprint:data.get('fingerprint'),spaceId:data.get('spaceId')});if(path)await writeInvite(result,path);
  }else{const path=action==='revokeDevice'?await outputPath('轻账-设备权限更新.json'):undefined;result=await system(action,{spaceId:data.get('spaceId'),deviceId:data.get('deviceId')});if(path)await writeInvite(result,path);}
  message(action==='createSharedSpace'?'共同账本已创建':'设备操作已完成，请按所选步骤将文件交给另一台可信设备');
 });
}
