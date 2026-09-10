import { connected, desktop, ledger, system } from '../bridge';
import { escape, type Account } from '../types';
import { modal, field, run, message } from './ui';
import { backup, exportFile } from './files';
import { devices } from './devices';
import { rules } from './rules';
import { privacy } from './platform';
import { syncStatus, syncNow } from './sync-status';
export async function settings(host:HTMLElement,refresh:()=>Promise<void>) {
  const accounts = connected ? await ledger<Account[]>('accounts') : [];
  host.innerHTML=`<section class="card"><div class="section-heading"><div><h2>付款账户</h2><p>确认每一笔钱从哪里来。</p></div><button id="add-account">添加账户</button></div><div class="chips">${accounts.map(a=>`<span>${escape(a.name)}</span>`).join('')||'<p class="muted">尚未添加付款账户</p>'}</div></section><section class="card" id="rules"></section><section class="card"><h2>设备与同步</h2><p>连接你自己的 WebDAV 空间，账本内容在设备端加密。</p><div class="actions"><button id="configure-sync">配置 WebDAV</button><button id="sync-now">立即同步</button><button id="pair-device">设备配对</button></div><p class="muted">可以配对另一台轻账设备，共享个人账本或共同账本。</p><div id="sync-status"></div></section><section class="card"><h2>账本与备份</h2><p>CSV / JSON 是明文账单，加密备份需要独立恢复密码。</p><div class="actions"><button id="export-csv">导出 CSV</button><button id="export-json">导出 JSON</button><button id="backup">加密备份</button><button id="restore">恢复备份</button></div></section><section class="card" id="privacy"></section>`;
  const bind=(id:string,fn:()=>Promise<void>)=>host.querySelector<HTMLButtonElement>(`#${id}`)!.onclick=()=>void run(fn);
  bind('add-account',async()=>modal('添加付款账户',field('账户名称','name'),'添加',async data=>{await ledger('addAccount',{name:data.get('name')});await refresh();}));
  bind('configure-sync',async()=>modal('配置 WebDAV',`${field('服务地址（HTTPS）','url','','url')}${field('用户名','username')}${field('密码','password','','password')}<p class="muted">保存前会测试列目录、写入、读取与清理权限。</p>`,'测试并保存',async data=>{const payload=Object.fromEntries(data);await system('syncConfigure',payload);message('WebDAV 连接测试通过，配置已保存');await syncStatus(host.querySelector('#sync-status')!);}));
  bind('sync-now',async()=>{const button=host.querySelector<HTMLButtonElement>('#sync-now')!;button.disabled=true;try{await syncNow(host.querySelector('#sync-status')!);}finally{button.disabled=false;}});
  bind('pair-device',devices);
  await rules(host.querySelector('#rules')!);
  bind('export-csv',()=>exportFile('Csv'));bind('export-json',()=>exportFile('Json'));bind('backup',()=>backup(false,refresh));bind('restore',()=>backup(true,refresh));
  await syncStatus(host.querySelector('#sync-status')!);
  await privacy(host.querySelector('#privacy')!,refresh);
}
