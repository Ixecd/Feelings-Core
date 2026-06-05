# Feelings-Core

> 神经感受引擎——PSIR/DSIR/ESIR 生成与 PBM 管理。
> Core = animi Pass 6-8（Personalize / DeviceMap / CodeGen）。
> Pass 0-5 在 [Anim](https://github.com/Ixecd/Anim) 完成（.anim 源码 → FSIR JSON）。
> Core 读 FSIR JSON → 个人基线矩阵校准 → 设备映射 → 感受帧输出。

## 核心价值观

Feelings-Core 不是"先写代码再对齐价值观"。代码就是价值观的编译产物。

所有实现决策以 [VALUES-TO-CODE.md](VALUES-TO-CODE.md) 为索引——
每一行代码都可以追溯到父仓库 `Feelings/` 里的一份或多份价值观文档。

| 文档 | 作用 |
|------|------|
| [VALUES-TO-CODE.md](VALUES-TO-CODE.md) | 价值观→代码的完整桥接地图 |
| [FORGET.md](FORGET.md) | 待修复项（P0+P1）——当前代码状态 |
| [../Feelings/Feelings-ROADMAP.md](../Feelings/Feelings-ROADMAP.md) | 阶段零→五的路线图 |
| [../Feelings/GOVERNANCE-FEELINGS.md](../Feelings/GOVERNANCE-FEELINGS.md) | 不可谈判的红线 |

## 架构

```
animi 编译器（Rust）
    ├── Pass 0: LexParse      ] 
    ├── Pass 1: TypeCheck      ]  Anim v1.0（已有代码）
    ├── Pass 2: SafetyCheck    ]  输入 .anim 源码 → 输出 FSIR JSON
    ├── Pass 3: FSIRGen        ]
    ├── Pass 4: Verify         ]
    ├── Pass 5: Link           ]
    │
    ├── Pass 6: Personalize    ]  
    ├── Pass 7: DeviceMap      ]  Feelings-Core（本仓库）
    └── Pass 8: CodeGen        ]  读 FSIR → PBM 校准 → 设备映射 → PSIR/DSIR/ESIR 帧输出

Feelings-OS（Rust + C）
    └── 六个守护进程——Core 作为独立进程运行在其上
```

## License

私有。GOVERNANCE 红线——Core 永久保留，不公开。

