const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const test = require('node:test');
const root = path.resolve(__dirname, '..');
function method(file, name, ctx = {}) {
    const text = fs.readFileSync(path.join(root, file), 'utf8');
    const match = text.match(new RegExp('^    function ' + name + '\\([^]*?^    }', 'm'));
    assert.ok(match, name);
    return vm.runInNewContext('(' + match[0].trim() + ')', ctx);
}
const expand = method('shell/services/Calendar.qml', 'expand');
const day = s => new Date(s + 'T00:00:00');
const ev = rrule => ({start: new Date(2026,0,5,10), end: new Date(2026,0,5,11), exdates: [], rrule});
test('monthly weekdays and negative month days', () => {
    let out = expand(ev({FREQ:'MONTHLY',BYDAY:'MO'}), day('2026-02-01'), day('2026-03-01'));
    assert.deepEqual(Array.from(out, o=>o.start.getDate()), [2,9,16,23]);
    out = expand(ev({FREQ:'MONTHLY',BYMONTHDAY:'-1'}), day('2026-02-01'),day('2026-03-01'));
    assert.equal(out.length,1); assert.equal(out[0].start.getDate(),28);
});
test('leap day does not roll over, long COUNT recurrence remains visible', () => {
    const leap = {start:new Date(2024,1,29,10),end:new Date(2024,1,29,11),rrule:{FREQ:'YEARLY'}};
    assert.equal(expand(leap,day('2025-01-01'),day('2026-01-01')).length,0);
    const long = {start:new Date(2010,0,1,10),end:new Date(2010,0,1,11),rrule:{FREQ:'DAILY',COUNT:'10000'}};
    assert.equal(expand(long,day('2026-10-28'),day('2026-10-29')).length,1);
});
test('calendar overlap excludes exclusive end and includes zero-length appointments', () => {
    assert.equal(expand({start:day('2026-01-01'),end:day('2026-01-02')},day('2026-01-02'),day('2026-01-03')).length,0);
    assert.equal(expand({start:day('2026-01-02'),end:day('2026-01-02')},day('2026-01-02'),day('2026-01-03')).length,1);
});
test('filter reset cancels and ignores obsolete image results', () => {
    const requests=[];
    class Xhr { static DONE=4; constructor(){requests.push(this)} open(){} send(){} abort(){this.aborted=true;} }
    const ctx={generation:0,request:null,loading:false,exhausted:false,page:1,posts:[],error:'',XMLHttpRequest:Xhr,tags:{text:''},url:()=>'',normalize:p=>p};
    const search=method('shell/modules/sidebar/Booru.qml','search',ctx);
    search(true); search(true);
    assert.equal(requests.length,2); assert.equal(requests[0].aborted,true);
    Object.assign(requests[0],{readyState:4,status:200,responseText:'[{"preview":"obsolete"}]'});
    requests[0].onreadystatechange(); assert.equal(ctx.posts.length,0);
    Object.assign(requests[1],{readyState:4,status:200,responseText:'[{"preview":"current"}]'});
    requests[1].onreadystatechange(); assert.equal(ctx.posts[0].preview,'current');
});
test('translation cancellation and OCR failure clear pending state', () => {
    const translator={generation:0,input:{text:''},result:'old',error:'',activeRequest:{running:true}};
    method('shell/modules/sidebar/Translator.qml','translate',translator)();
    assert.equal(translator.activeRequest.running,false); assert.equal(translator.result,'');
    const screen={translation:{busy:true},Quickshell:{execDetached:()=>{}}};
    method('shell/services/ScreenTools.qml','failed',screen)('translate');
    assert.equal(screen.translation.busy,false);
});
test('desktop session Exec preserves quoted executable and arguments without shell expansion', () => {
    const ctx={}; vm.runInNewContext(fs.readFileSync(path.join(root,'shell/modules/greeter/session-exec.js'),'utf8'),ctx);
    assert.deepEqual(Array.from(ctx.parse('"/some path/session" "two words" literal$HOME %% %c %f','Session')),['/some path/session','two words','literal$HOME','%','Session']);
    assert.throws(()=>ctx.parse('"unfinished','Session'));
});
test('sleep dispatch waits for compositor lock confirmation', () => {
    const calls=[];
    const ctx={sleepPending:true,lockSecure:false,locked:true,powerCommands:{sleep:['systemctl','suspend']},Quickshell:{execDetached:c=>calls.push(c)}};
    const finish=method('shell/services/System.qml','finishSleep',ctx);
    finish(); assert.equal(calls.length,0);
    ctx.lockSecure=true; finish(); finish(); assert.equal(calls.length,1);
});
test('obsolete weather response cannot replace a newer location result', () => {
    const requests=[];
    class Xhr { static DONE=4; constructor(){requests.push(this)} open(){} send(){} abort(){this.aborted=true;} }
    const ctx={generation:0,request:null,error:'latest',Config:{options:{weather:{location:'First'}}},XMLHttpRequest:Xhr};
    const refresh=method('shell/services/Weather.qml','refresh',ctx);
    refresh(); ctx.Config.options.weather.location='Second'; refresh();
    Object.assign(requests[0],{readyState:4,status:500}); requests[0].onreadystatechange();
    assert.equal(ctx.error,'latest'); assert.equal(requests[0].aborted,true);
});
test('phase advance uses the new phase duration even before derived bindings update', () => {
    const ctx={Island:{enabled:true,wants:()=>true},phase:'focus',completed:0,focusMinutes:25,breakMinutes:5,longBreakMinutes:15,phaseLength:1500,remaining:0,deadline:0,phaseEnded:()=>{},Date:{now:()=>10000}};
    method('shell/services/Timers.qml','advance',ctx)();
    assert.equal(ctx.phase,'break'); assert.equal(ctx.remaining,300); assert.equal(ctx.deadline,310000);
});
