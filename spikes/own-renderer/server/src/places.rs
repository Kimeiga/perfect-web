//! **Where a delivery goes, and whether a store reaches it** (track
//! `store-accounts`, ADR-XXXX, milestone 2).
//!
//! **The places are a fixture, not a geocoder.** A reader's address is a
//! label they give and one of the places below, each with coordinates stated
//! here, approximate, and fixed in the repository. Nothing resolves an
//! address the table does not hold, and no request leaves this machine to
//! find one: a geocoding service would be a download and an account.
//!
//! **A store reaches a place within its radius** ([`delivers_to`]), the
//! great-circle distance between the two (the haversine formula, on a sphere
//! of the Earth's mean radius), and a courier takes [`MINUTES_PER_KM`] for
//! each kilometre of it ([`travel_minutes`]). The store's estimate and its
//! refusals read the same two functions, in both of the store's layers, so a
//! page and a command can never disagree about one address.

/// **A place an address may name**: its id, which an address keeps, what a
/// reader is shown, and where it is, in millionths of a degree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Place {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) lat_e6: i64,
    pub(crate) lon_e6: i64,
}

/// **The places**, in the order the address page lists them. Coordinates are
/// each place's approximate centre, stated to five decimal places.
pub(crate) const PLACES: &[Place] = &[
    Place {
        id: "union-square",
        name: "Union Square, San Francisco",
        lat_e6: 37_787_990,
        lon_e6: -122_407_440,
    },
    Place {
        id: "ferry-building",
        name: "Ferry Building, San Francisco",
        lat_e6: 37_795_500,
        lon_e6: -122_393_700,
    },
    Place {
        id: "chinatown",
        name: "Chinatown, San Francisco",
        lat_e6: 37_794_100,
        lon_e6: -122_407_800,
    },
    Place {
        id: "dolores-park",
        name: "Dolores Park, San Francisco",
        lat_e6: 37_759_770,
        lon_e6: -122_427_060,
    },
    Place {
        id: "the-castro",
        name: "The Castro, San Francisco",
        lat_e6: 37_760_900,
        lon_e6: -122_435_000,
    },
    Place {
        id: "golden-gate-park",
        name: "Golden Gate Park, San Francisco",
        lat_e6: 37_769_400,
        lon_e6: -122_486_200,
    },
    Place {
        id: "ocean-beach",
        name: "Ocean Beach, San Francisco",
        lat_e6: 37_759_400,
        lon_e6: -122_510_700,
    },
    Place {
        id: "lake-merritt",
        name: "Lake Merritt, Oakland",
        lat_e6: 37_801_500,
        lon_e6: -122_258_300,
    },
    Place {
        id: "berkeley",
        name: "Downtown Berkeley",
        lat_e6: 37_870_200,
        lon_e6: -122_268_100,
    },
    Place {
        id: "palo-alto",
        name: "Downtown Palo Alto",
        lat_e6: 37_444_700,
        lon_e6: -122_161_100,
    },
];

/// **How long a courier takes for each kilometre**, in minutes: 15 km/h, a
/// stated model, not a measurement.
pub(crate) const MINUTES_PER_KM: f64 = 4.0;

/// The Earth's mean radius, in kilometres (IUGG).
const EARTH_KM: f64 = 6371.0088;

/// **The place an address names**, by its id.
pub(crate) fn place(id: &str) -> Option<&'static Place> {
    PLACES.iter().find(|p| p.id == id)
}

/// **The great-circle distance between two points**, in kilometres, each in
/// millionths of a degree: the haversine formula.
pub(crate) fn distance_km(lat_a: i64, lon_a: i64, lat_b: i64, lon_b: i64) -> f64 {
    let rad = |e6: i64| (e6 as f64 / 1e6).to_radians();
    let (phi_a, phi_b) = (rad(lat_a), rad(lat_b));
    let d_phi = phi_b - phi_a;
    let d_lambda = rad(lon_b) - rad(lon_a);
    let h =
        (d_phi / 2.0).sin().powi(2) + phi_a.cos() * phi_b.cos() * (d_lambda / 2.0).sin().powi(2);
    2.0 * EARTH_KM * h.sqrt().min(1.0).asin()
}

/// **Where a store is, and how far it delivers**: its location in millionths
/// of a degree and its radius in metres, as the store's rows hold them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Zone {
    pub(crate) lat_e6: i64,
    pub(crate) lon_e6: i64,
    pub(crate) radius_m: i64,
}

/// **Whether a store delivers to a place**: within its radius, the boundary
/// included. The one rule the estimate's `NoCoverage` and a command's refusal
/// both read.
pub(crate) fn delivers_to(zone: &Zone, place: &Place) -> bool {
    distance_km(zone.lat_e6, zone.lon_e6, place.lat_e6, place.lon_e6) * 1000.0
        <= zone.radius_m as f64
}

/// **How many minutes a courier takes from a store to a place**, whole
/// minutes, rounded up: at least one, since a delivery leaves the store.
pub(crate) fn travel_minutes(zone: &Zone, place: &Place) -> i64 {
    let km = distance_km(zone.lat_e6, zone.lon_e6, place.lat_e6, place.lon_e6);
    ((km * MINUTES_PER_KM).ceil() as i64).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A distance the haversine formula is known to give**: Union Square
    /// to the Ferry Building is about 1.5 km, and a place is no distance
    /// from itself.
    #[test]
    fn the_distance_is_the_great_circle_one() {
        let (a, b) = (
            place("union-square").unwrap(),
            place("ferry-building").unwrap(),
        );
        let d = distance_km(a.lat_e6, a.lon_e6, b.lat_e6, b.lon_e6);
        assert!((1.4..1.7).contains(&d), "{d}");
        assert_eq!(distance_km(a.lat_e6, a.lon_e6, a.lat_e6, a.lon_e6), 0.0);
        // Symmetric.
        assert_eq!(d, distance_km(b.lat_e6, b.lon_e6, a.lat_e6, a.lon_e6));
        // San Francisco to Palo Alto, about 44 km.
        let p = place("palo-alto").unwrap();
        let far = distance_km(a.lat_e6, a.lon_e6, p.lat_e6, p.lon_e6);
        assert!((40.0..50.0).contains(&far), "{far}");
    }

    /// **Within the radius, the boundary included; past it, not.**
    #[test]
    fn a_store_delivers_within_its_radius_and_not_past_it() {
        let a = place("union-square").unwrap();
        let b = place("ferry-building").unwrap();
        let d_m = distance_km(a.lat_e6, a.lon_e6, b.lat_e6, b.lon_e6) * 1000.0;
        let at = |radius_m: i64| Zone {
            lat_e6: a.lat_e6,
            lon_e6: a.lon_e6,
            radius_m,
        };
        assert!(delivers_to(&at(d_m.ceil() as i64), b), "at the boundary");
        assert!(
            !delivers_to(&at(d_m.floor() as i64 - 1), b),
            "a metre short"
        );
        assert!(delivers_to(&at(0), a), "its own place");
    }

    /// **A courier's minutes**: four a kilometre, rounded up, at least one.
    #[test]
    fn travel_is_four_minutes_a_kilometre_rounded_up() {
        let a = place("union-square").unwrap();
        let zone = Zone {
            lat_e6: a.lat_e6,
            lon_e6: a.lon_e6,
            radius_m: 100_000,
        };
        assert_eq!(travel_minutes(&zone, a), 1, "at least one");
        let b = place("ferry-building").unwrap();
        let d = distance_km(a.lat_e6, a.lon_e6, b.lat_e6, b.lon_e6);
        assert_eq!(travel_minutes(&zone, b), (d * 4.0).ceil() as i64);
    }

    /// Each place's id is its own, and each is in the San Francisco Bay Area.
    #[test]
    fn each_place_is_one_and_in_the_bay_area() {
        for (i, p) in PLACES.iter().enumerate() {
            assert!(PLACES.iter().skip(i + 1).all(|q| q.id != p.id), "{}", p.id);
            assert!((37_000_000..38_500_000).contains(&p.lat_e6), "{}", p.id);
            assert!((-123_000_000..-121_500_000).contains(&p.lon_e6), "{}", p.id);
        }
    }
}
