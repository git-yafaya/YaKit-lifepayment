// 金额格式化必须保留整数精度，解析金额不能悄悄四舍五入。
import assert from 'node:assert/strict';
import { money, minor, escape } from '../frontend/types.ts';
assert.equal(minor('28.05','CNY'),'2805');
assert.equal(minor('100','JPY'),'100');
assert.equal(money('9223372036854775807','CNY'),'CNY 92233720368547758.07');
assert.equal(money('-105','CNY'),'CNY -1.05');
assert.throws(()=>minor('1.001','CNY'));
assert.throws(()=>minor('1.1','JPY'));
assert.throws(()=>minor('-1','CNY'));
assert.equal(escape('<script>'),'&lt;script&gt;');
console.log('界面金额与文本检查通过');
