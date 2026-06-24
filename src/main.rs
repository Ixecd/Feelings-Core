// Feelings-Core — 神经感受引擎入口
//
// 用法: feelings-core [--species human|canine|feline|psittacine]
//       默认: human
//
// 作为 Feelings-OS 的一个独立进程启动。
// 读取 FSIR + CoreConfig → PBM 校准 → PSIR/DSIR/ESIR 帧输出
// 不联网。不在云端。数据不离设备。

use std::env;

fn main() {
    let species = parse_species();

    println!("Feelings-Core v0.1 — species: {}", species);
    // v0.1: 启动骨架。Session<D,S> 已就位——下一阶段接入 FSIR 加载。
}

fn parse_species() -> String {
    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--species" && i + 1 < args.len() {
            let s = args[i + 1].to_lowercase();
            if matches!(s.as_str(), "human" | "canine" | "feline" | "psittacine") {
                return s;
            }
            eprintln!("未知物种 '{}'——使用默认 human", args[i + 1]);
            return "human".into();
        }
        i += 1;
    }
    "human".into()
}
