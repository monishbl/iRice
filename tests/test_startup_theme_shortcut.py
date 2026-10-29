"""Startup regression checks; no live compositor or user configuration changes."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class StartupTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which('lua'), 'requires Lua')
    def test_tap_has_explicit_release_for_real_and_nested_modifier(self):
        script=r'''
local root, mod = arg[1], arg[2]
IRICE_MOD=mod
IRICE_HYPR_DIR=root .. '/hypr/irice'
IRICE_USER_DIR='/nonexistent-irice-test-config'
local records={}
hl={dsp=setmetatable({}, {__index=function(_, key) return function(...) return {kind=key,args={...}} end end}),
 bind=function(keys,action,opts) table.insert(records,{keys=keys,action=action,opts=opts}) end}
dofile(root .. '/hypr/irice/keybinds.lua')
local press, release, cancel, overview = false,false,false,false
for _,r in ipairs(records) do
 if type(r.action)=='table' and r.action.kind=='global' then
  local name=r.action.args[1]
  if name=='irice:superTap' and r.opts.release then
   assert(r.keys==(mod=='SUPER' and 'Super_L' or 'Alt_L'))
   assert(r.opts.ignore_mods and r.opts.non_consuming)
   release=true
  elseif name=='irice:superTap' then
   assert(r.keys==(mod=='SUPER' and 'Super_L' or 'Alt_L'), 'tap press must match the pre-modifier keydown mask')
   press=true
  elseif name=='irice:superTapCancel' then cancel=true
  elseif name=='irice:overview' then overview=true end
 end
end
assert(press and release and cancel)
if mod=='SUPER' then assert(overview) end
'''
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'test.lua'; p.write_text(script)
            for mod in ['SUPER','ALT']:
                result=subprocess.run(['lua',str(p),str(ROOT),mod],capture_output=True,text=True)
                self.assertEqual(result.returncode,0,result.stdout+result.stderr)

    def test_shell_restart_supplies_palette_theme_and_keeps_explicit_choice(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d); (p/'bin').mkdir()
            (p/'bin/qs').write_text('#!/bin/sh\nprintf "%s" "$QT_QPA_PLATFORMTHEME"\n')
            (p/'bin/qs').chmod(0o755)
            env=dict(os.environ, HOME=d, XDG_CONFIG_HOME=str(p/'config'), XDG_DATA_HOME=str(p/'data'), PATH=str(p/'bin')+':'+os.environ['PATH'], IRICE_SANDBOX='1', IRICE_TTY='0')
            for supplied, expected in [('', 'kde'), ('qt6ct', 'qt6ct')]:
                env['QT_QPA_PLATFORMTHEME']=supplied
                result=subprocess.run(['bash',str(ROOT/'shell/assets/scripts/launch-shell.sh')],env=env,capture_output=True,text=True,timeout=8)
                self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual(result.stdout,expected)

    @unittest.skipUnless(shutil.which('g++') and Path('/usr/include/KF6/KColorScheme/KColorSchemeManager').exists() and Path('/usr/lib/qt6/plugins/platformthemes/KDEPlasmaPlatformTheme6.so').exists(), 'requires Qt/KDE development files')
    def test_kde_platform_palette_is_correct_before_graphics_widgets_exist(self):
        source=r'''
#include <QApplication>
#include <QGraphicsWidget>
#include <KColorSchemeManager>
int main(int argc,char**argv) {
 QApplication app(argc,argv);
 app.setApplicationName("dolphin");
 QGraphicsWidget view;
 const QColor initial=view.palette().color(QPalette::Text);
 auto manager=KColorSchemeManager::instance();
 const QColor after=app.palette().color(QPalette::Text);
 return initial==after && initial.lightness()>180 ? 0 : 1;
}
'''
        with tempfile.TemporaryDirectory() as d:
            p=Path(d); cfg=p/'config'; data=p/'data'; cfg.mkdir(); (data/'color-schemes').mkdir(parents=True)
            scheme='[General]\nName=Fixture\n[Colors:View]\nBackgroundNormal=20,20,20\nForegroundNormal=235,235,235\n[Colors:Window]\nBackgroundNormal=20,20,20\nForegroundNormal=235,235,235\n'
            (data/'color-schemes/Fixture.colors').write_text(scheme)
            (cfg/'kdeglobals').write_text(scheme.replace('Name=Fixture','ColorScheme=Fixture'))
            (cfg/'dolphinrc').write_text('[UiSettings]\nColorScheme=Fixture\n')
            (p/'probe.cpp').write_text(source)
            flags=subprocess.check_output(['pkg-config','--cflags','--libs','Qt6Widgets'],text=True).split()
            subprocess.run(['g++','-fPIC','-std=c++20',str(p/'probe.cpp'),'-I/usr/include/KF6/KColorScheme',*flags,'-lKF6ColorScheme','-o',str(p/'probe')],check=True,capture_output=True,timeout=30)
            env=dict(os.environ,QT_QPA_PLATFORM='offscreen',QT_QPA_PLATFORMTHEME='kde',XDG_CONFIG_HOME=str(cfg),XDG_DATA_HOME=str(data),XDG_CACHE_HOME=str(p/'cache'))
            subprocess.run([str(p/'probe')],env=env,check=True,capture_output=True,timeout=5)
            env['QT_QPA_PLATFORMTHEME']=''
            result=subprocess.run([str(p/'probe')],env=env,capture_output=True,timeout=5)
            self.assertNotEqual(result.returncode,0,'Control should reproduce the late-palette problem')


if __name__ == '__main__': unittest.main()
