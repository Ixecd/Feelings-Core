// Feelings-Core — D3 主动麻痹锚点（Grounding Signal）
//
// D3 = 极限防御。Aborted 前根据触发维度下发反向对冲信号——
// 迷走神经低频镇定 + 其余维度全静默。不是"停"——是"按回地面"。

/// 单个维度的 grounding 指令。
#[derive(Debug, Clone, Copy)]
pub struct GroundingChannel {
    pub intensity: u32,
    pub silenced: bool,
}

/// D3 主动麻痹锚点——按维度分发的 grounding 信号矩阵。
#[derive(Debug, Clone)]
pub struct GroundingSignal<const D: usize> {
    pub channels: [GroundingChannel; D],
}

const QUIET: GroundingChannel = GroundingChannel {
    intensity: 0,
    silenced: true,
};
const CALM: GroundingChannel = GroundingChannel {
    intensity: 5,
    silenced: false,
};

impl<const D: usize> GroundingSignal<D> {
    /// D3 默认 grounding——仅 Visceral(idx 0) 输出低频镇定，其余全静默。
    pub fn d3_default() -> Self {
        let mut channels = [QUIET; D];
        if D > 0 {
            channels[0] = CALM;
        }
        GroundingSignal { channels }
    }

    /// 根据触发源维度定制——无论哪个维度熔断，Visceral(idx 0)始终输出低频镇定。
    pub fn for_source(source_idx: usize) -> Self {
        assert!(source_idx < D, "source_idx {source_idx} out of dimension bounds {D}");
        let mut channels = [QUIET; D];
        if D > 0 {
            // 无论哪个维度熔断——内脏始终承担迷走神经低频镇定的职责。
            // 内脏自身熔断 → 仍输出 CALM，按回地面。
            // 其他维度熔断 → 内脏输出 CALM，源头维度保持 QUIET（全静默）。
            channels[0] = CALM;
        }
        GroundingSignal { channels }
    }
}
