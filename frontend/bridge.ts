import { invoke } from '@tauri-apps/api/core';
export const desktop = '__TAURI_INTERNALS__' in window;
const devBridge = import.meta.env.DEV ? import.meta.env.VITE_LEDGER_DEV_BRIDGE : undefined;
export const connected = desktop || Boolean(devBridge);
export async function ledger<T>(action: string, payload: object = {}): Promise<T> {
  if (desktop) return invoke<T>('ledger_command', { action, payload });
  // 开发验收也调用真实 Rust 账务代码，生产构建不包含此入口。
  if(devBridge){const response=await fetch(devBridge,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action,payload})});const result=await response.json();if(!response.ok||!result.ok)throw Error(result.error||'本地开发服务请求失败');return result.value as T;}
  throw Error('当前为界面预览。请在 Windows 桌面应用中连接本地账本。');
}
export async function system<T>(action: string, payload: object = {}): Promise<T> {
  if (!desktop) throw Error('此操作需要 Windows 桌面应用。');
  return invoke<T>('system_command', { action, payload });
}
