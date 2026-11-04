//! Filter types and slopes for parametric and graphic equalizers.

/// Parametric EQ filter types.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum FilterType {
    /// Bell / Peaking filter (boost or cut around center frequency).
    Bell,
    /// Low cut / High pass filter (attenuates low frequencies).
    LowCut,
    /// High cut / Low pass filter (attenuates high frequencies).
    HighCut,
    /// Low shelf filter (boosts or cuts low frequencies).
    LowShelf,
    /// High shelf filter (boosts or cuts high frequencies).
    HighShelf,
    /// Notch / Band stop filter (eliminates narrow frequency band).
    Notch,
    /// Band pass filter (passes only narrow band).
    BandPass,
    /// Tilt shelf filter (pivots frequency balance across spectrum).
    TiltShelf,
}

impl FilterType {
    /// All filter types in standard ordering.
    pub const ALL: [FilterType; 8] = [
        FilterType::Bell,
        FilterType::LowCut,
        FilterType::HighCut,
        FilterType::LowShelf,
        FilterType::HighShelf,
        FilterType::Notch,
        FilterType::BandPass,
        FilterType::TiltShelf,
    ];

    /// Short display name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Bell => "Bell",
            Self::LowCut => "Low Cut",
            Self::HighCut => "High Cut",
            Self::LowShelf => "Low Shelf",
            Self::HighShelf => "High Shelf",
            Self::Notch => "Notch",
            Self::BandPass => "Band Pass",
            Self::TiltShelf => "Tilt Shelf",
        }
    }

    /// Short 2-3 letter abbreviation.
    #[must_use]
    pub const fn abbreviation(&self) -> &'static str {
        match self {
            Self::Bell => "BELL",
            Self::LowCut => "HP",
            Self::HighCut => "LP",
            Self::LowShelf => "LS",
            Self::HighShelf => "HS",
            Self::Notch => "NOTCH",
            Self::BandPass => "BP",
            Self::TiltShelf => "TILT",
        }
    }

    /// Whether this filter type supports boost/cut gain adjustment.
    #[must_use]
    pub const fn has_gain(&self) -> bool {
        match self {
            Self::Bell | Self::LowShelf | Self::HighShelf | Self::TiltShelf => true,
            Self::LowCut | Self::HighCut | Self::Notch | Self::BandPass => false,
        }
    }
}

/// Filter cutoff slope in decibels per octave.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum FilterSlope {
    /// 6 dB/oct (1st-order)
    Db6,
    /// 12 dB/oct (2nd-order Butterworth)
    Db12,
    /// 18 dB/oct (3rd-order)
    Db18,
    /// 24 dB/oct (4th-order Linkwitz-Riley)
    Db24,
    /// 36 dB/oct (6th-order)
    Db36,
    /// 48 dB/oct (8th-order Brickwall)
    Db48,
    /// 96 dB/oct (Ultra steep)
    Db96,
}

impl FilterSlope {
    /// Display name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Db6 => "6 dB/oct",
            Self::Db12 => "12 dB/oct",
            Self::Db18 => "18 dB/oct",
            Self::Db24 => "24 dB/oct",
            Self::Db36 => "36 dB/oct",
            Self::Db48 => "48 dB/oct",
            Self::Db96 => "96 dB/oct",
        }
    }

    /// Number of biquad cascades required.
    #[must_use]
    pub const fn cascade_count(&self) -> usize {
        match self {
            Self::Db6 | Self::Db12 => 1,
            Self::Db18 | Self::Db24 => 2,
            Self::Db36 => 3,
            Self::Db48 => 4,
            Self::Db96 => 8,
        }
    }
}
