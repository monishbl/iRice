//! A stand-in input method for testing text-input-v3 in the sandbox (there's
//! no fcitx5/IBus there): it registers as the seat's input method
//! (zwp_input_method_v2), and when a text field asks for one it shows a
//! preedit ("cafe" underlined), then commits "café ✓" and quits.

use smithay_client_toolkit::reexports::client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{wl_registry::WlRegistry, wl_seat::WlSeat},
    Connection, Dispatch, QueueHandle,
};
use std::time::{Duration, Instant};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
    zwp_input_method_v2::{self, ZwpInputMethodV2},
};

#[derive(Default)]
struct Ime {
    active: bool,
    serial: u32,
    stage: u8, // 0 waiting, 1 preedit shown, 2 committed
    since: Option<Instant>,
    gone: bool,
}

impl Dispatch<ZwpInputMethodV2, ()> for Ime {
    fn event(s: &mut Self, _: &ZwpInputMethodV2, ev: zwp_input_method_v2::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match ev {
            zwp_input_method_v2::Event::Activate => s.active = true,
            zwp_input_method_v2::Event::Deactivate => s.active = false,
            zwp_input_method_v2::Event::Done => s.serial += 1,
            zwp_input_method_v2::Event::Unavailable => s.gone = true,
            _ => {}
        }
    }
}
impl Dispatch<WlRegistry, GlobalListContents> for Ime {
    fn event(_: &mut Self, _: &WlRegistry, _: <WlRegistry as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlSeat, ()> for Ime {
    fn event(_: &mut Self, _: &WlSeat, _: <WlSeat as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwpInputMethodManagerV2, ()> for Ime {
    fn event(_: &mut Self, _: &ZwpInputMethodManagerV2, _: <ZwpInputMethodManagerV2 as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

fn main() {
    // Only inside a sandbox arena (dev.sh sets TESSERA_ARENA there), and only
    // as `fake-ime` with nothing after it: never the input method of a real
    // session it was started in by mistake (a harness's `fake-ime --list`)
    if std::env::var_os("TESSERA_ARENA").or_else(|| std::env::var_os("IRICE_ARENA")).is_none() || std::env::args().len() > 1 {
        eprintln!("fake-ime: only inside a sandbox arena (IRICE_ARENA=N ./dev.sh run fake-ime), without arguments");
        std::process::exit(2);
    }
    let conn = Connection::connect_to_env().expect("display");
    let (globals, mut queue) = registry_queue_init::<Ime>(&conn).expect("registry");
    let qh = queue.handle();
    let seat: WlSeat = globals.bind(&qh, 1..=7, ()).expect("seat");
    let mgr: ZwpInputMethodManagerV2 = globals.bind(&qh, 1..=1, ()).expect("no zwp_input_method_manager_v2");
    let im = mgr.get_input_method(&seat, &qh, ());
    let mut s = Ime::default();
    let end = Instant::now() + Duration::from_secs(30);
    while Instant::now() < end && !s.gone && s.stage < 2 {
        queue.roundtrip(&mut s).unwrap();
        if s.active && s.stage == 0 {
            eprintln!("fake-ime: activated, preedit");
            im.set_preedit_string("cafe".into(), 4, 4);
            im.commit(s.serial);
            s.stage = 1;
            s.since = Some(Instant::now());
        } else if s.active && s.stage == 1 && s.since.unwrap().elapsed() > Duration::from_millis(2500) {
            eprintln!("fake-ime: commit");
            im.set_preedit_string(String::new(), 0, 0);
            im.commit_string("café ✓".into());
            im.commit(s.serial);
            s.stage = 2;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    queue.roundtrip(&mut s).ok();
    eprintln!("fake-ime: done (stage {})", s.stage);
}
