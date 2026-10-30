//! R7 retains the actually used Light bounds, routing authorization and DSL.
pub mod limits {
    pub const MAX_TRACE_BRANCHES: usize = 16;
    pub const MAX_SIGNALS_ACTIVE: usize = 1024;
    pub const CONTRACT_MAX_AGE_SECS: u64 = 86400;
    pub const RING_BUFFER_SIZE: usize = 64;
}

// ════════════════════════════════════════════════════════════════════════════
// §6  KOMUNIKIMI — rrugët e thirrjeve ndër-platformike
// ════════════════════════════════════════════════════════════════════════════

pub mod communication {
    /// Autorizimi ndër-platform: kush mund të thërrasë kë
    pub fn is_authorized(caller: &str, target: &str) -> bool {
        let authorized: &[(&str, &str)] = &[
            // Light internal
            ("light_coordinator", "trace_info"),
            ("light_coordinator", "evolve_trace"),
            ("light_coordinator", "nura_core"),
            ("light_coordinator", "software_contract"),
            ("light_coordinator", "quantum_platform"),
            ("light_coordinator", "shadow_judiciary"),
            ("trace_info", "lgc_algorithm"),
            ("evolve_trace", "lgc_algorithm"),
            ("evolve_trace", "legalgocrypt"),
            ("software_contract", "lgc_algorithm"),
            ("software_contract", "legalgocrypt"),
            ("algorithm_continuum", "progressive_debatic"),
            ("nura_core", "light_buss"),
            // Shadow → gjithkush (autoritet suprem)
            ("shadow_judiciary", "light_coordinator"),
            ("shadow_judiciary", "nk_system"),
            ("shadow_judiciary", "vok_store"),
            ("shadow_judiciary", "vnk_store"),
            // Quantum → Shadow (propozon vetëm tek Shadow)
            ("quantum_platform", "shadow_judiciary"),
            ("pim_engine", "shadow_judiciary"),
            ("lim_engine", "pro_engine"),
            ("pro_engine", "srk_engine"),
            ("srk_engine", "pim_engine"),
        ];
        authorized.contains(&(caller, target))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §7  DSL — Gjuha Algoritmike (nga nura_lgc_dsl)
//     Sufixet e knowledge, gjendjet e algoritmit, operacionet matematike
// ════════════════════════════════════════════════════════════════════════════

pub mod dsl {
    // ── SUFFIX CODES — kodi i degës në EvolveTrace ────────────────────────
    // Shembull: x1234_hiskng_1 = branch HISTORY nga trace x1234
    pub const SFX_INPUT: &str = "INPKNG"; // input fillestar
    pub const SFX_HISTORY: &str = "HISKNG"; // histori
    pub const SFX_GEOGRAPHY: &str = "GEOKNG"; // gjeografi
    pub const SFX_MATHEMATICS: &str = "MATHKNG"; // matematik
    pub const SFX_SCIENCE: &str = "SCIKNG"; // shkencë
    pub const SFX_INDUSTRIAL: &str = "INDKNG"; // industrial
    pub const SFX_SEMANTIC: &str = "SEMKNG"; // semantikë
    pub const SFX_PHILOSOPHICAL: &str = "PHIKNG"; // filozofi
    pub const SFX_TECHNICAL: &str = "TECKNG"; // teknik
    pub const SFX_FORMULATION: &str = "FRMKNG"; // formulim
    pub const SFX_NEGATIVE: &str = "NEGKNG"; // NK path
    pub const SFX_VERIFIED: &str = "VFYKNG"; // i verifikuar
    pub const SFX_SHADOW: &str = "SHKNG"; // nga Shadow

    /// Të gjithë sufixet si array
    pub const ALL_SUFFIXES: &[&str] = &[
        SFX_INPUT,
        SFX_HISTORY,
        SFX_GEOGRAPHY,
        SFX_MATHEMATICS,
        SFX_SCIENCE,
        SFX_INDUSTRIAL,
        SFX_SEMANTIC,
        SFX_PHILOSOPHICAL,
        SFX_TECHNICAL,
        SFX_FORMULATION,
        SFX_NEGATIVE,
        SFX_VERIFIED,
        SFX_SHADOW,
    ];

    /// A është suffiks valid?
    pub fn is_valid_suffix(s: &str) -> bool {
        ALL_SUFFIXES.contains(&s)
    }

    // ── GJENDJET E ALGORITMIT — state machine e pipeline ─────────────────
    #[derive(Debug, Clone, PartialEq)]
    pub enum AlgorithmState {
        Evolving,       // evolve_trace po proceson
        Failed(String), // gabim i papritur
    }

    impl AlgorithmState {
        pub fn label(&self) -> &str {
            match self {
                Self::Evolving => "EVOLVING",
                Self::Failed(_) => "FAILED",
            }
        }
    }

    // ── OPERACIONET MATEMATIKE — O(1) per path ────────────────────────────
    // Të njëjtat formula si Quantum Platform
}
