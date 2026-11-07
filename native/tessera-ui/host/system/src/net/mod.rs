//! group: net (see lib.rs): network, audio, brightness, power, resources.
//! Each submodule replaces the QML files named in its header.

use tessera_core::host::Service;

pub mod audio;
pub mod audio_profiles;
pub mod audio_streams;
pub mod battery_health;
pub mod brightness;
pub mod common;
pub mod network_alerts;
pub mod network_feed;
pub mod power;
pub mod power_policy;
pub mod pulse_events;
pub mod resources;
pub mod speed_test;
pub mod statusmenus_feed;
pub mod wifi_join;
pub mod wifi_share;

/// This group's services
pub fn services() -> Vec<Box<dyn Service>> {
    vec![
        Box::new(network_alerts::NetworkAlerts::default()),
        Box::new(wifi_join::WifiJoin::default()),
        Box::new(wifi_share::WifiShare::default()),
        Box::new(speed_test::SpeedTest::default()),
        Box::new(network_feed::NetworkFeed::default()),
        Box::new(statusmenus_feed::StatusMenusFeed::default()),
        Box::new(audio::Audio::default()),
        Box::new(audio_profiles::AudioProfiles::default()),
        Box::new(audio_streams::AudioStreams::default()),
        Box::new(pulse_events::PulseEvents::default()),
        Box::new(brightness::Brightness::default()),
        Box::new(power::Power::default()),
        Box::new(battery_health::BatteryHealth::default()),
        Box::new(resources::Resources::default()),
    ]
}
