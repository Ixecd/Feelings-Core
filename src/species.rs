// Feelings-Core — 物种泛型。编译期展开，零运行时开销。
//
// FeelingTarget trait = 决定维度数量、维度类型、通路映射。
// CLI: --species human (默认) | canine | feline | psittacine
//
// 不同物种 = 不同的 FeelingTarget 实现。维度数、通路映射、安全参数
// 在编译期展开——不是运行时读配置。

use std::fmt::Debug;
use std::hash::Hash;

/// 物种特征——编译期确定维度数量和类型。
pub trait FeelingTarget: Debug + Clone + Copy + Send + Sync + 'static {
    /// 编译期确定的感受维度数量。
    const DIM_COUNT: usize;

    /// 关联的维度枚举类型。
    type Dimension: Copy + Eq + Hash + Debug + Send + Sync + 'static;

    /// 物种字面量标识 (CLI --species 参数)。
    fn species_name() -> &'static str;

    /// 维度 → 数组索引。
    fn dim_index(dim: Self::Dimension) -> usize;

    /// 数组索引 → 维度。
    fn index_to_dim(idx: usize) -> Option<Self::Dimension>;
}

// ── Human 特化——4 维 ──────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct Human;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HumanDimension {
    Visceral,
    Emotional,
    Tactile,
    Auditory,
}

impl FeelingTarget for Human {
    const DIM_COUNT: usize = 4;
    type Dimension = HumanDimension;

    fn species_name() -> &'static str {
        "human"
    }
    fn dim_index(dim: Self::Dimension) -> usize {
        match dim {
            HumanDimension::Visceral => 0,
            HumanDimension::Emotional => 1,
            HumanDimension::Tactile => 2,
            HumanDimension::Auditory => 3,
        }
    }
    fn index_to_dim(idx: usize) -> Option<Self::Dimension> {
        match idx {
            0 => Some(HumanDimension::Visceral),
            1 => Some(HumanDimension::Emotional),
            2 => Some(HumanDimension::Tactile),
            3 => Some(HumanDimension::Auditory),
            _ => None,
        }
    }
}

// ── Psittacine (金刚鹦鹉) 特化——3 维 ──────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct Psittacine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MacawDimension {
    FeathersTactile,
    OpticFlow,
    AcousticCochlear,
}

impl FeelingTarget for Psittacine {
    const DIM_COUNT: usize = 3;
    type Dimension = MacawDimension;

    fn species_name() -> &'static str {
        "psittacine"
    }
    fn dim_index(dim: Self::Dimension) -> usize {
        match dim {
            MacawDimension::FeathersTactile => 0,
            MacawDimension::OpticFlow => 1,
            MacawDimension::AcousticCochlear => 2,
        }
    }
    fn index_to_dim(idx: usize) -> Option<Self::Dimension> {
        match idx {
            0 => Some(MacawDimension::FeathersTactile),
            1 => Some(MacawDimension::OpticFlow),
            2 => Some(MacawDimension::AcousticCochlear),
            _ => None,
        }
    }
}

// ── Canine / Feline 骨架——待完实现 ────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct Canine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CanineDimension {
    Visceral,
    Emotional,
    Tactile,
    Auditory,
}

impl FeelingTarget for Canine {
    const DIM_COUNT: usize = 4;
    type Dimension = CanineDimension;
    fn species_name() -> &'static str {
        "canine"
    }
    fn dim_index(dim: Self::Dimension) -> usize {
        match dim {
            CanineDimension::Visceral => 0,
            CanineDimension::Emotional => 1,
            CanineDimension::Tactile => 2,
            CanineDimension::Auditory => 3,
        }
    }
    fn index_to_dim(idx: usize) -> Option<Self::Dimension> {
        match idx {
            0 => Some(CanineDimension::Visceral),
            1 => Some(CanineDimension::Emotional),
            2 => Some(CanineDimension::Tactile),
            3 => Some(CanineDimension::Auditory),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Feline;

impl FeelingTarget for Feline {
    const DIM_COUNT: usize = 4;
    type Dimension = CanineDimension;
    fn species_name() -> &'static str {
        "feline"
    }
    fn dim_index(dim: Self::Dimension) -> usize {
        Canine::dim_index(dim)
    }
    fn index_to_dim(idx: usize) -> Option<Self::Dimension> {
        Canine::index_to_dim(idx)
    }
}
