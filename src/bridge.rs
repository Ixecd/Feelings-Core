// Feelings-Core — pbm/ ↔ species/ 泛型桥接
//
// PbmDimension (pbm 模块, 4 enum 变体) ↔ HumanDimension (species.rs)
// 两者映射相同的四维度——互转零成本。

use crate::pbm::PbmDimension;
use crate::species::HumanDimension;

impl From<HumanDimension> for PbmDimension {
    fn from(d: HumanDimension) -> Self {
        match d {
            HumanDimension::Visceral => PbmDimension::Visceral,
            HumanDimension::Emotional => PbmDimension::Emotional,
            HumanDimension::Tactile => PbmDimension::Tactile,
            HumanDimension::Auditory => PbmDimension::Auditory,
        }
    }
}

impl From<PbmDimension> for HumanDimension {
    fn from(d: PbmDimension) -> Self {
        match d {
            PbmDimension::Visceral => HumanDimension::Visceral,
            PbmDimension::Emotional => HumanDimension::Emotional,
            PbmDimension::Tactile => HumanDimension::Tactile,
            PbmDimension::Auditory => HumanDimension::Auditory,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_human_to_pbm() {
        let h = HumanDimension::Emotional;
        let p: PbmDimension = h.into();
        let h2: HumanDimension = p.into();
        assert_eq!(h, h2);
    }

    #[test]
    fn all_variants_roundtrip() {
        for h in [HumanDimension::Visceral, HumanDimension::Emotional, HumanDimension::Tactile, HumanDimension::Auditory] {
            let p: PbmDimension = h.into();
            let h2: HumanDimension = p.into();
            assert_eq!(h, h2);
        }
    }
}
