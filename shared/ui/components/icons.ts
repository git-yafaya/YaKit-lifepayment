// 所有图标共用 24 像素网格和圆角线条，颜色跟随所在控件。
const shapes = {
  ledger: '<rect x="5" y="3.5" width="14" height="17" rx="2.5"/><path d="M9 3.5v17M12 8h4M12 12h4"/>',
  home: '<path d="m3 10 9-7 9 7M5 9v11h5v-6h4v6h5V9"/>',
  transactions: '<rect x="5" y="3.5" width="14" height="17" rx="2.5"/><path d="M9 8h6M9 12h6M9 16h4"/>',
  analysis: '<path d="M10 3.3a9 9 0 1 0 10.7 10.7H10Z"/><path d="M14 3.3V10h6.7A9 9 0 0 0 14 3.3Z"/>',
  pending: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7v5l3 2"/>',
  recycle: '<path d="M4 10a8 8 0 1 1 1 8M4 4v6h6"/>',
  settings: '<path d="M4 7h4M12 7h8M4 17h8M16 17h4"/><circle cx="10" cy="7" r="2"/><circle cx="14" cy="17" r="2"/>',
  sparkles: '<path d="m10 5 2.3 5.7L18 13l-5.7 2.3L10 21l-2.3-5.7L2 13l5.7-2.3ZM19 2v6M16 5h6"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  close: '<path d="m6 6 12 12M18 6 6 18"/>',
  arrowUpRight: '<path d="M6 18 18 6M6 6h12v12"/>',
  arrowDownLeft: '<path d="M18 6 6 18M6 6v12h12"/>',
  arrowRight: '<path d="M4 12h16m-6-6 6 6-6 6"/>',
  transfer: '<path d="M4 7h16m-4-4 4 4-4 4M20 17H4m4-4-4 4 4 4"/>',
  status: '<circle cx="12" cy="12" r="9" fill="currentColor" stroke="none"/>',
} as const;

export type IconName = keyof typeof shapes;

// 装饰图标不重复朗读；按钮和链接保留自己的文字或无障碍名称。
export function icon(name: IconName): string {
  return `<svg class="ui-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${shapes[name]}</svg>`;
}
