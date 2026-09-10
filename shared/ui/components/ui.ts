import { icon } from './icons';
import { escape } from '../types';
export function empty(title: string, text: string): string { return `<div class="empty"><span class="empty-mark">${icon('plus')}</span><h3>${escape(title)}</h3><p>${escape(text)}</p></div>`; }
export function message(text: string, error = false) { const el = document.querySelector<HTMLElement>('#status')!; el.textContent = text; el.classList.toggle('error',error); el.hidden = !text; }
export async function run(work: () => Promise<void>) { document.body.classList.add('busy'); try { await work(); } catch(e) { message(e instanceof Error ? e.message : String(e),true); } finally { document.body.classList.remove('busy'); } }
export function modal(title: string, body: string, submit: string, callback: (data: FormData) => Promise<void>) {
  const dialog = document.createElement('dialog');
  dialog.innerHTML = `<form><header><h2>${escape(title)}</h2><button type="button" class="icon close" aria-label="关闭">${icon('close')}</button></header><div class="dialog-body">${body}</div><footer><button type="button" class="close">取消</button><button class="primary" type="submit">${escape(submit)}</button></footer><p class="form-error" role="alert"></p></form>`;
  document.body.append(dialog); dialog.querySelectorAll('.close').forEach(b => b.addEventListener('click',()=>dialog.close()));
  let saving=false;dialog.addEventListener('cancel',e=>{if(saving)e.preventDefault();});dialog.addEventListener('close',()=>dialog.remove()); dialog.querySelector('form')!.onsubmit = async e => { e.preventDefault(); const button = dialog.querySelector<HTMLButtonElement>('[type=submit]')!; if(saving)return;saving=true;button.disabled = true;dialog.querySelectorAll<HTMLButtonElement>('.close').forEach(b=>b.disabled=true); try { await callback(new FormData(e.currentTarget as HTMLFormElement)); dialog.close(); } catch(error) { dialog.querySelector('.form-error')!.textContent = String(error instanceof Error ? error.message : error); } finally { saving=false;button.disabled = false;dialog.querySelectorAll<HTMLButtonElement>('.close').forEach(b=>b.disabled=false); } }; dialog.showModal();
}
export const field = (label: string, name: string, value = '', type = 'text', required = true) => `<label>${escape(label)}<input name="${name}" type="${type}" value="${escape(value)}" ${required ? 'required' : ''}></label>`;
