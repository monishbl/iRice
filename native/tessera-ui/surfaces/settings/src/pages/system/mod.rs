//! The "system" page group: Privacy & Security (with Fingerprint and Face),
//! Battery & Power (and its Cool-down subpage), Accessibility. Its own
//! views: ui/settings-system.slint (the add-a-finger sheet, the Add finger
//! row, the apps' energy bars). What only the QML shell knows comes from
//! shell/modules/settings/RustBridgeSystem.qml (`cx.feed["system"]`); what
//! only it does (anything that needs root, the power daemon, installs)
//! goes there (`cx.ask("system", …)`).

pub mod access;
pub mod art;
pub mod power;
pub mod security;

use super::super::model::{Ctx, Page};

/// Build `page` if it's one of this group's; false when it isn't
pub fn build(page: &str, cx: &mut Ctx, p: &mut Page) -> bool {
    match page {
        "security" => security::security(cx, p),
        "power" => power::power(cx, p),
        "power/cooldown" => power::cooldown(cx, p),
        "accessibility" => access::accessibility(cx, p),
        _ => return false,
    }
    true
}
