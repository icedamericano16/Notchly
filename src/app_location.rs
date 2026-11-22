//! Windows location (asks for permission when first used).

use windows::Devices::Geolocation::{GeolocationAccessStatus, Geolocator};

use crate::services::{com_mta, spawn, Bus, Event};

pub fn locate(bus: Bus) {
    spawn("location", move || {
        com_mta();
        let ok = Geolocator::RequestAccessAsync()
            .and_then(|o| o.join())
            .map(|s| s == GeolocationAccessStatus::Allowed)
            .unwrap_or(false);
        if !ok {
            bus.send(Event::Log("location access denied".into()));
            crate::platform::shell_open("ms-settings:privacy-location");
            return;
        }
        let Ok(g) = Geolocator::new() else { return };
        if let Ok(pos) = g.GetGeopositionAsync().and_then(|o| o.join()) {
            if let Ok(p) = pos
                .Coordinate()
                .and_then(|c| c.Point())
                .and_then(|p| p.Position())
            {
                bus.send(Event::Location(
                    (p.Latitude * 100.0).round() / 100.0,
                    (p.Longitude * 100.0).round() / 100.0,
                ));
            }
        }
    });
}
