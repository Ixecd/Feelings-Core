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

    /// 根据触发源维度定制——源头维度不施加额外刺激。
    pub fn for_source(source_idx: usize) -> Self {
        let mut channels = [QUIET; D];
        if D > 0 && source_idx != 0 {
            channels[0] = CALM;
        }
        GroundingSignal { channels }
    }
}
