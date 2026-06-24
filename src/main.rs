// Feelings-Core — 神经感受引擎入口
//
// 用法: feelings-core [--species human|canine|feline|psittacine]
//       默认: human
//
// 静态分发——运行时 species 字符串被路由到编译期单态化的泛型引擎。
// 每物种生成独立优化路径——零运行时开销。

use std::env;

fn main() {
    let species = parse_species();
    println!("Feelings-Core v0.1 — monomorphized pipeline: {}", species);

    match species.as_str() {
        "human" => run::<4>(),
        "psittacine" => run::<3>(),
        "canine" => run::<4>(),
        "feline" => run::<4>(),
        _ => unreachable!(),
    }
}

fn run<const D: usize>() {
    let config = feelings_core::config::CoreConfig::default();
    config
        .validate_dimensions::<D>()
        .expect("config dimension mismatch");
    println!("  dimension: {} ✓", D);
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
