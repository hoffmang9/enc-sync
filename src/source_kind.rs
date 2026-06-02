//! OpenCPN ENC source kind model for embedded chart sources.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    All,
    State(&'static str),
    Region(&'static str),
    CoastGuardDistrict(&'static str),
    InlandMain,
    InlandBuoys,
    InlandOverlays,
}
