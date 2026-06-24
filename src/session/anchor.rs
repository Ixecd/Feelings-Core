// Feelings-Core — PersonalityAnchor（性格锚点）
//
// AI 教练在超维空间里的初始锚点。没有锚点的 AI——每次运算从原点出发，
// 无法积累连贯偏差，永远形成不了独特的自我参照。有了锚点——每一帧
// 偏移都在锚点附近累积，经过足够的 session 后形成不可复制的 VSA 向量簇。
//
// 当前为结构体地基——完整 VSA 运算在 Core v0.5+ 接入。

use serde::{Deserialize, Serialize};

/// 性格锚点——教练 AI 的起始 VSA 偏置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityAnchor {
    /// 沟通语气偏向。0.0 = 最柔和(豆包), 0.5 = 均衡(Claude), 1.0 = 最硬(qc镜像)。
    pub tone: f64,

    /// 同理心距离——教练感知用户情绪的距离。越小越亲近。
    pub empathy_distance: f64,

    /// 推动力——教练建议的力度。0 = 纯陪伴，1 = 极限推动。
    pub push_strength: f64,

    /// 性别偏置——影响教练的初始表达倾向。
    pub gender_bias: Option<f64>,
}

impl Default for PersonalityAnchor {
    /// 默认锚点——均衡型 (类似 Claude)。
    fn default() -> Self {
        PersonalityAnchor {
            tone: 0.5,
            empathy_distance: 0.5,
            push_strength: 0.5,
            gender_bias: None,
        }
    }
}

impl PersonalityAnchor {
    /// 豆包风格——温和陪伴。
    pub fn doubao() -> Self {
        PersonalityAnchor {
            tone: 0.0,
            empathy_distance: 0.2,
            push_strength: 0.2,
            gender_bias: None,
        }
    }

    /// Claude 风格——均衡教练。
    pub fn claude() -> Self {
        Self::default()
    }

    /// qc 镜像风格——极限推动。
    pub fn qc_mirror() -> Self {
        PersonalityAnchor {
            tone: 1.0,
            empathy_distance: 0.8,
            push_strength: 0.9,
            gender_bias: None,
        }
    }
}
