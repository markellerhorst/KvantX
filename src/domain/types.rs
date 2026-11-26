//! Small enumerations that describe a file, the quantification and the signal.

/// Role of a file in the analysis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Blank,
    Standard,
    QC,
    Unknown,
}

impl Kind {
    /// All kinds, in the order in which they are offered in menus.
    pub const ALL: [Kind; 4] = [Kind::Blank, Kind::Standard, Kind::QC, Kind::Unknown];
    /// Display name of the kind; it is also what a project file stores.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Blank => "Blank",
            Kind::Standard => "Standard",
            Kind::QC => "QC",
            Kind::Unknown => "Unknown",
        }
    }
    /// Parses a stored label; unknown labels become [`Kind::Unknown`].
    pub fn from_label(s: &str) -> Kind {
        match s {
            "Blank" => Kind::Blank,
            "Standard" => Kind::Standard,
            "QC" => Kind::QC,
            _ => Kind::Unknown,
        }
    }
}

/// How results are reported.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Areas,
    Curve,
}

/// Which signal of a target is integrated and quantified.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Basis {
    Ms2,
    Ms1,
}

/// How the boundaries of a peak are found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeakMethod {
    /// Boundaries at 5 % of the apex height.
    Legacy,
    /// Smoothing, noise estimate and derivative.
    Adaptive,
}

impl PeakMethod {
    /// All methods, in the order in which they are offered.
    pub const ALL: [PeakMethod; 2] = [PeakMethod::Legacy, PeakMethod::Adaptive];

    /// Text for menus.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PeakMethod::Legacy => "Legacy (5 % of the apex height)",
            PeakMethod::Adaptive => "Adaptive (Savitzky-Golay)",
        }
    }

    /// Short key stored in settings and projects.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            PeakMethod::Legacy => "legacy",
            PeakMethod::Adaptive => "adaptive",
        }
    }

    /// Parses a stored key; unknown text gives `None`.
    #[must_use]
    pub fn from_key(key: &str) -> Option<PeakMethod> {
        PeakMethod::ALL.into_iter().find(|m| m.key() == key)
    }
}

/// Color theme of the user interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    /// Follow the theme of the operating system.
    System,
    /// Light.
    Light,
    /// Dark.
    Dark,
}

impl Theme {
    /// Short key stored in the settings.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    /// Parses a stored key; unknown text gives `None`.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Theme> {
        [Theme::System, Theme::Light, Theme::Dark]
            .into_iter()
            .find(|t| t.key() == key)
    }
}

/// One of the two integration boundaries of a peak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    Start,
    End,
}

/// How a file was brought into KvantX.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ImportRoute {
    /// An mzML file read by KvantX itself.
    #[default]
    Direct,
    /// A vendor file converted to mzML by an external program (ThermoRawFileParser for Thermo RAW).
    Conversion,
}

impl ImportRoute {
    /// Short key stored in projects.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            ImportRoute::Direct => "direct",
            ImportRoute::Conversion => "conversion",
        }
    }

    /// Parses a stored key; unknown text gives `None`.
    #[must_use]
    pub fn from_key(key: &str) -> Option<ImportRoute> {
        [ImportRoute::Direct, ImportRoute::Conversion]
            .into_iter()
            .find(|r| r.key() == key)
    }
}

/// Provenance of an import, stored in the project.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ImportInfo {
    /// Direct reading or conversion.
    pub route: ImportRoute,
    /// Details of the import: the format, and for a conversion the converter, its version and parameters.
    pub detail: String,
}
