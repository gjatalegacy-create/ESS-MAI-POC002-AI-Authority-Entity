//! R7 minimal Light entity surface; original unused signal/IoT/device registries archived.
use crate::pd_light::VerifiedPdSurface;

#[derive(Debug, Clone, PartialEq)]
pub enum Domain {
    Work,    // NURA: biznes, industri, governance, SCADA
    Home,    // ALISAH: familje, shtëpi, IoT, MATTER
    Outside, // AMMY: mobile, web, jashtë
}

impl Domain {
    /// Detekton domain nga teksti — shqip + anglisht
    pub fn detect(text: &str) -> Self {
        let t = text.to_lowercase();

        // Work/NURA keywords
        let work: &[&str] = &[
            "biznes",
            "industri",
            "scada",
            "kontratë",
            "governance",
            "prodhim",
            "motor",
            "sensor",
            "vibration",
            "pressure",
            "plc",
            "mes",
            "cnc",
            "business",
            "industry",
            "contract",
            "production",
            "manufacturing",
            "enterprise",
            "corporate",
            "compliance",
            "audit",
            "invoice",
        ];
        // Home/ALISAH keywords
        let home: &[&str] = &[
            "shtëpi",
            "familje",
            "shtëpia",
            "shtëpinë",
            "matter",
            "iot",
            "pajisje",
            "ndriçim",
            "temperaturë",
            "termostati",
            "llambë",
            "kyç",
            "kamera",
            "home",
            "family",
            "house",
            "device",
            "light",
            "thermostat",
            "lock",
            "sensor",
            "appliance",
            "kitchen",
            "bedroom",
            "living",
            "garden",
        ];
        // Outside/AMMY keywords
        let outside: &[&str] = &[
            "mobile",
            "web",
            "jashtë",
            "rrugë",
            "internet",
            "shfleto",
            "kërko",
            "outside",
            "browse",
            "search",
            "online",
            "navigate",
            "location",
            "map",
            "travel",
            "trip",
            "route",
            "weather",
            "restaurant",
            "store",
        ];

        let score_work = work.iter().filter(|&&k| t.contains(k)).count();
        let score_home = home.iter().filter(|&&k| t.contains(k)).count();
        let score_outside = outside.iter().filter(|&&k| t.contains(k)).count();

        match (
            score_home > score_work && score_home > score_outside,
            score_outside > score_work && score_outside > score_home,
        ) {
            (true, _) => Domain::Home,
            (_, true) => Domain::Outside,
            _ => Domain::Work, // default: NURA
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Domain::Work => "WORK→NURA",
            Domain::Home => "HOME→ALISAH",
            Domain::Outside => "OUTSIDE→AMMY",
        }
    }
}

#[derive(Default)]
pub struct NuraCore;
impl NuraCore {
    pub fn new() -> Self {
        Self
    }
    pub fn route(&self, text: &str) -> Domain {
        Domain::detect(text)
    }
    pub fn speak_pd(&self, verified_surface: &VerifiedPdSurface) -> String {
        format!("Nura: {}", verified_surface.as_str())
    }
}
