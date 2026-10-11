import assert from 'node:assert/strict';
import test from 'node:test';
import { clock, elapsed, goalTarget, kindOf, totals } from './timing.ts';
import type { Log, Tag } from './types.ts';

const now = new Date(2026,9,12,12,0,0).getTime();
const tags: Tag[] = [
  { id:'study',name:'学习',kind:'positive',sortOrder:0,archivedAt:null },
  { id:'archived',name:'旧任务',kind:'positive',sortOrder:1,archivedAt:new Date(now).toISOString() },
  { id:'video',name:'刷视频',kind:'negative',sortOrder:2,archivedAt:null },
];
const iso = (delta:number) => new Date(now+delta*1000).toISOString();
test('absolute timing survives missed ticks, sleep and future timestamps',()=>{
  assert.equal(elapsed({id:'1',tag:'学习',startTime:iso(-3601.8)},now),3601);
  assert.equal(elapsed({id:'1',tag:'学习',startTime:iso(30)},now),0);
  assert.equal(elapsed({id:'1',tag:'学习',startTime:'invalid'},now),0);
  assert.equal(clock(90061),'25:01:01');
});
test('matches Nichijou positive goal; parallel tasks accumulate individually',()=>{
  const logs:Log[]=[{id:'1',tag:'旧任务',startTime:iso(-7200),endTime:iso(-3600),duration:3600}, {id:'2',tag:'刷视频',startTime:iso(-100),endTime:iso(-40),duration:60}];
  const values=totals(tags,logs,[{id:'a',tag:'学习',startTime:iso(-60)}, {id:'b',tag:'旧任务',startTime:iso(-60)}],now);
  assert.equal(values.positive,3720);assert.equal(values.negative,60);assert.equal(goalTarget,18000);
  assert.equal(kindOf(tags,'未知标签'),'neutral');
});
test('completed sessions belong to start date; running midnight session keeps full duration',()=>{
  const midnight=new Date(2026,9,12,0,10,0).getTime();
  const start=new Date(2026,9,11,23,50,0).toISOString();
  const log:Log={id:'1',tag:'学习',startTime:start,endTime:new Date(midnight).toISOString(),duration:1200};
  assert.equal(totals(tags,[log],[],midnight).positive,0);
  assert.equal(totals(tags,[],[{id:'1',tag:'学习',startTime:start}],midnight).positive,1200);
});
