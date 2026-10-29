from pathlib import Path
import tempfile, unittest
import shutil, subprocess, os
ROOT=Path(__file__).resolve().parents[1]
QS=Path.home()/'.local/share/irice/quickshell/bin/qs'

@unittest.skipUnless(QS.exists(), 'requires bundled Quickshell')
class CalendarLayoutTests(unittest.TestCase):
 def test_bounded_agenda_and_all_tabs(self):
  with tempfile.TemporaryDirectory(prefix='irice-calendar-test-') as tmp:
   self.check_layout(Path(tmp))

 def check_layout(self, p):
  for d in ['common','services','modules/controlcenter','runtime']: (p/d).mkdir(parents=True)
  (p/'runtime').chmod(0o700)
  def put(f,s): (p/f).write_text(s)
  shutil.copy(ROOT/'shell/modules/notifications/CalendarWidget.qml',p/'CalendarWidget.qml')
  shutil.copy(ROOT/'shell/modules/notifications/TodoWidget.qml',p/'TodoWidget.qml')
  put('modules/controlcenter/Platter.qml','import QtQuick\nRectangle { property real shapeRadius: 24; color: "#332036"; radius: shapeRadius }')
  put('common/Theme.qml', '''pragma Singleton
  import QtQuick
  import Quickshell
  Singleton {
  property bool dark: true
  property color accent: '#ffa500'
  property color accentInk: '#ffb844'
  property var accents: ({orange:'#ffa500'})
  property var color: ({label:'white',secondary:'#c4b7c7',tertiary:'#9c8d9e',fill:'#665064',fillHover:'#776074',fillPressed:'#887085',onAccent:'white',danger:'red',toggleOff:'#665064',stroke:'#665064',separator:'#665064'})
  property var font: ({family:'Sans',body:13,small:12,caption:11,title:17})
  property var radius: ({control:8})
  property var motion: ({hover:100,fast:150,standard:200,popover:150,emphasized:[0.2,0,0,1,1,1]})
  property var glass: ({solid:'#332036',tier:'solid'})
  function accentAlpha(a) { return Qt.alpha(accent,a); }
  }''')
  put('common/Icon.qml', 'import QtQuick\nText { property string name; property bool fill; property bool bold; property int size: 14; text: name === "arrow-up" ? "↑" : name === "caret-left" ? "‹" : name === "caret-right" ? "›" : "+"; font.pixelSize: size }')
  put('common/ElasticControl.qml','import QtQuick\nQtObject { property bool pressed; property var effects: [] }')
  for f in ['IconButton.qml','Segmented.qml','TextField.qml']: shutil.copy((ROOT/'shell/common')/f,p/'common'/f)
  put('common/Switch.qml','import QtQuick\nItem { property bool checked; signal toggled(); implicitWidth: 40; implicitHeight: 24 }')
  put('common/Dropdown.qml','import QtQuick\nItem { property var options; property var value; signal chosen(var v); implicitHeight: 30 }')
  put('services/Calendar.qml','''pragma Singleton
  import QtQuick
  import Quickshell
  Singleton {
  property var local: []
  property var remote: []
  property int defaultAlert: 0
  function between(a,b) { return []; }
  function eventsOn(d) { return [{title:'Karaka Chaturthi',allDay:true,start:d,end:d,color:'#25cf91',calendar:'Holidays in India',local:false}]; }
  function timeText(e) { return 'All day'; }
  }''')
  put('services/Todos.qml','''pragma Singleton
  import QtQuick
  import Quickshell
  Singleton {
  property var list: Array.from({length:45},(_,i)=>({text:'Task '+(i+1),done:false,i:i,due:new Date(2026,9,29).getTime()}))
  function on(d) { return d.getDate()===29 ? list : []; }
  function dueText(t) {return 'Today';}
  function dueLabel(t) {return 'Today';}
  function overdue(t) { return false; }
  }''')
  put('shell.qml','''import QtQuick
  import Quickshell
  ShellRoot {
  settings.watchFiles: false
  FloatingWindow { id: win; visible: true; implicitWidth: 384; implicitHeight: 384; color: '#88706c'
  CalendarWidget { id: cal; x:20;y:20;width:344;height:344;padding:16;cellHeight:22;fitHeight:true;todoTab:true;week:true;selected:new Date(2026,9,29) }
  }
  function find(o,n) { if(o.objectName===n) return o; for(const c of o.children ?? []) { const r=find(c,n); if(r) return r; } return null; }
  property int stage: 0
  Timer { interval:350;running:true;repeat:true;onTriggered: {
  const v=find(cal,'calendarViewport'); const a=find(cal,'calendarAgenda');
  if(stage===0) {
   if(find(cal,'calendarQuickAdd')) throw new Error('quick input remains');
   const dates=find(cal,'calendarDates');
   if(dates.height<900) throw new Error('week entries truncated');
   const list=find(cal,'calendarItems');
   if(list.children.filter(c=>c.height===36).length!==46) throw new Error('agenda truncated');
   if(!v || !v.clip || v.y+v.height>cal.height || cal.items.length!==46 || v.contentHeight<2000) throw new Error('bounds/items regression');
   console.log('GEOMETRY',v.height,v.contentHeight,a.y,a.height);
   
  } else if(stage===1) {v.contentY=230;}
  else if(stage===2) {}
  else if(stage===3) {v.contentY=v.contentHeight-v.height;}
  else if(stage===4) {const list=find(cal,'calendarItems');const pos=list.mapToItem(cal,0,list.height);if(pos.y>cal.height || pos.y<cal.height-25) throw new Error('last task unreachable');cal.scrollToCalendar();}
  else if(stage===5) {if(v.contentY!==0) throw new Error('return failed');cal.week=false;}
  else if(stage===6) {if(v.y+v.height>cal.height) throw new Error('month bounds');cal.todoMode=true;}
  else if(stage===7) {if(v.contentHeight<1800) throw new Error('todo bounds');cal.todoMode=false;cal.fitHeight=false;}
  else if(stage===8) {if(v.interactive || v.clip || v.height!==v.contentHeight) throw new Error('notification center regression');console.log('CALENDAR_OK');Qt.quit();}
  stage++;
  }}
  }''')
  env=dict(os.environ,QT_QPA_PLATFORM='offscreen',QT_QUICK_BACKEND='software',XDG_RUNTIME_DIR=str(p/'runtime'),XDG_CONFIG_HOME=str(p/'config'),XDG_CACHE_HOME=str(p/'cache'),QML_IMPORT_PATH=str(QS.parent.parent/'lib/qt6/qml'))
  r=subprocess.run([str(QS),'-p',str(p)],env=env,capture_output=True,text=True,timeout=8)
  out=r.stdout+r.stderr
  self.assertEqual(r.returncode,0,out)
  self.assertIn('CALENDAR_OK',out)
  for error in ['ERROR','TypeError','ReferenceError','Binding loop','Unable to assign']:
   self.assertNotIn(error,out)

if __name__ == '__main__': unittest.main()
