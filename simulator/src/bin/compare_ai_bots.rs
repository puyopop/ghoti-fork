use anyhow::Result;
use clap::Parser;
use cpu::bot::{BeamSearchAI, ChainFocusedAI, ChainPotentialAI, HybridAI, RandomAI, StableAI, AI};
use ghoti_simulator::simulate_1p::simulate_1p;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use logger::Logger;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

/// 異なるAI Botの性能を比較するツール
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// シミュレーション回数（各AIごと）
    #[clap(short = 'n', long, default_value = "20")]
    num_games: usize,

    /// 並列実行するスレッド数
    #[clap(short = 'p', long, default_value = "4")]
    parallel: usize,

    /// 最大手数
    #[clap(long, default_value = "50")]
    max_tumos: usize,

    /// AIが見える手数
    #[clap(long, default_value = "2")]
    visible_tumos: usize,

    /// 目標連鎖得点（これを超えたら終了）
    #[clap(long, default_value = "20000")]
    required_chain_score: usize,

    /// 比較するAI（カンマ区切り: BeamSearch,ChainFocused,ChainPotential,Stable,Hybrid,Random）
    #[clap(long, default_value = "BeamSearch,ChainFocused,ChainPotential,Stable,Hybrid")]
    ai_types: String,

    /// 結果をJSONファイルに出力
    #[clap(long)]
    output_json: Option<String>,

    /// シードの開始値（再現性のため）
    #[clap(long, default_value = "0")]
    seed_start: u32,

    /// 詳細な結果を表示
    #[clap(long)]
    verbose: bool,
}

/// AIのタイプ
#[derive(Debug, Clone)]
enum AIType {
    BeamSearch,
    ChainFocused,
    ChainPotential,
    Stable,
    Hybrid,
    Random,
}

impl AIType {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "beamsearch" | "beam" => Some(AIType::BeamSearch),
            "chainfocused" | "chain" => Some(AIType::ChainFocused),
            "chainpotential" | "potential" => Some(AIType::ChainPotential),
            "stable" => Some(AIType::Stable),
            "hybrid" => Some(AIType::Hybrid),
            "random" => Some(AIType::Random),
            _ => None,
        }
    }

    fn name(&self) -> &str {
        match self {
            AIType::BeamSearch => "BeamSearchAI",
            AIType::ChainFocused => "ChainFocusedAI",
            AIType::ChainPotential => "ChainPotentialAI",
            AIType::Stable => "StableAI",
            AIType::Hybrid => "HybridAI",
            AIType::Random => "RandomAI",
        }
    }

    fn create_ai(&self) -> Box<dyn AI> {
        match self {
            AIType::BeamSearch => Box::new(BeamSearchAI::new()),
            AIType::ChainFocused => Box::new(ChainFocusedAI::new()),
            AIType::ChainPotential => Box::new(ChainPotentialAI::new()),
            AIType::Stable => Box::new(StableAI::new()),
            AIType::Hybrid => Box::new(HybridAI::new()),
            AIType::Random => Box::new(RandomAI::new()),
        }
    }
}

/// 統計情報
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Statistics {
    ai_name: String,
    total_games: usize,
    avg_score: f64,
    max_score: usize,
    min_score: usize,
    median_score: f64,
    std_dev: f64,
    avg_moves: f64,
    avg_time_ms: f64,
    games_over_10k: usize,
    games_over_20k: usize,
    games_over_50k: usize,
    games_over_70k: usize,
    success_rate: f64,
}

impl Statistics {
    fn from_results(ai_name: String, results: &[GameResult], required_score: usize) -> Self {
        let n = results.len() as f64;

        // スコアの統計
        let scores: Vec<usize> = results.iter().map(|r| r.score).collect();
        let mut sorted_scores = scores.clone();
        sorted_scores.sort_unstable();

        let sum: usize = scores.iter().sum();
        let avg_score = sum as f64 / n;

        let max_score = *scores.iter().max().unwrap_or(&0);
        let min_score = *scores.iter().min().unwrap_or(&0);

        let median_score = if sorted_scores.is_empty() {
            0.0
        } else if sorted_scores.len() % 2 == 0 {
            let mid = sorted_scores.len() / 2;
            (sorted_scores[mid - 1] + sorted_scores[mid]) as f64 / 2.0
        } else {
            sorted_scores[sorted_scores.len() / 2] as f64
        };

        // 標準偏差
        let variance = scores
            .iter()
            .map(|&x| {
                let diff = x as f64 - avg_score;
                diff * diff
            })
            .sum::<f64>()
            / n;
        let std_dev = variance.sqrt();

        // その他の統計
        let avg_moves = results.iter().map(|r| r.moves).sum::<usize>() as f64 / n;
        let avg_time_ms = results.iter().map(|r| r.time_ms).sum::<u128>() as f64 / n;
        let games_over_10k = results.iter().filter(|r| r.score >= 10000).count();
        let games_over_20k = results.iter().filter(|r| r.score >= 20000).count();
        let games_over_50k = results.iter().filter(|r| r.score >= 50000).count();
        let games_over_70k = results.iter().filter(|r| r.score >= 70000).count();
        let success_rate =
            results.iter().filter(|r| r.score >= required_score).count() as f64 / n * 100.0;

        Self {
            ai_name,
            total_games: results.len(),
            avg_score,
            max_score,
            min_score,
            median_score,
            std_dev,
            avg_moves,
            avg_time_ms,
            games_over_10k,
            games_over_20k,
            games_over_50k,
            games_over_70k,
            success_rate,
        }
    }
}

/// 個別ゲームの結果
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameResult {
    score: usize,
    moves: usize,
    time_ms: u128,
    max_chain: usize,
    seed: u32,
}

// 簡易的なLogger実装
struct SilentLogger;

impl Logger for SilentLogger {
    fn new(_: &str, _: Option<&str>) -> Result<Self, std::io::Error> {
        Ok(SilentLogger)
    }

    fn print(&mut self, _: String) -> std::io::Result<()> {
        Ok(())
    }
}

fn run_single_game(
    ai: Box<dyn AI>,
    seed: u32,
    max_tumos: usize,
    visible_tumos: usize,
    required_chain_score: usize,
) -> GameResult {
    let start = Instant::now();

    let mut logger: Box<dyn Logger> = Box::new(SilentLogger::new("benchmark", None).unwrap());

    let result = simulate_1p(
        &mut logger,
        &ai,
        visible_tumos,
        max_tumos,
        Some(seed as usize),
        Some(required_chain_score),
    );

    let time_ms = start.elapsed().as_millis();

    let (score, moves, max_chain) = match result {
        Ok(r) => {
            // 最大連鎖を推定（log_outputから）
            let max_chain = r
                .json_decisions
                .iter()
                .filter_map(|d| {
                    // JSONのフィールドは見えないので、とりあえず0とする
                    None
                })
                .max()
                .unwrap_or(0);
            (r.score, r.json_decisions.len(), max_chain)
        }
        Err(_) => (0, 0, 0),
    };

    GameResult {
        score,
        moves,
        time_ms,
        max_chain,
        seed,
    }
}

fn benchmark_ai(
    ai_type: &AIType,
    args: &Args,
    progress: Arc<Mutex<ProgressBar>>,
) -> (String, Vec<GameResult>) {
    let mut results = Vec::new();

    // ゲームを並列実行
    let chunk_size = (args.num_games + args.parallel - 1) / args.parallel;
    let results_mutex = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..args.parallel)
        .map(|thread_id| {
            let ai_type = ai_type.clone();
            let results_mutex = Arc::clone(&results_mutex);
            let progress = Arc::clone(&progress);
            let start_idx = thread_id * chunk_size;
            let end_idx = ((thread_id + 1) * chunk_size).min(args.num_games);
            let max_tumos = args.max_tumos;
            let visible_tumos = args.visible_tumos;
            let required_chain_score = args.required_chain_score;
            let seed_start = args.seed_start;

            thread::spawn(move || {
                for i in start_idx..end_idx {
                    let seed = seed_start + i as u32;
                    let ai = ai_type.create_ai();
                    let result = run_single_game(
                        ai,
                        seed,
                        max_tumos,
                        visible_tumos,
                        required_chain_score,
                    );

                    results_mutex.lock().unwrap().push(result);
                    progress.lock().unwrap().inc(1);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    results = Arc::try_unwrap(results_mutex)
        .unwrap()
        .into_inner()
        .unwrap();

    (ai_type.name().to_string(), results)
}

fn print_comparison_table(stats: &[Statistics]) {
    println!("\n╔════════════════════════════════════════════════════════════════════════════╗");
    println!("║                        AI Bot Performance Comparison                       ║");
    println!("╚════════════════════════════════════════════════════════════════════════════╝");

    // ヘッダー
    println!(
        "\n{:<14} {:>10} {:>10} {:>10} {:>6} {:>6} {:>6} {:>6} {:>8}",
        "AI Bot", "Avg Score", "Max Score", "Std Dev", "10k+", "20k+", "50k+", "70k+", "Success%"
    );
    println!("{:-<86}", "");

    // データ行
    for stat in stats {
        println!(
            "{:<14} {:>10.0} {:>10} {:>10.0} {:>6} {:>6} {:>6} {:>6} {:>8.1}",
            stat.ai_name,
            stat.avg_score,
            stat.max_score,
            stat.std_dev,
            stat.games_over_10k,
            stat.games_over_20k,
            stat.games_over_50k,
            stat.games_over_70k,
            stat.success_rate
        );
    }

    // 追加統計
    println!(
        "\n{:<14} {:>10} {:>10}",
        "AI Bot", "Avg Moves", "Avg Time(ms)"
    );
    println!("{:-<36}", "");

    for stat in stats {
        println!(
            "{:<14} {:>10.1} {:>10.1}",
            stat.ai_name, stat.avg_moves, stat.avg_time_ms
        );
    }

    // 最優秀AIの判定
    println!("\n📊 Performance Summary:");

    if let Some(best_avg) = stats
        .iter()
        .max_by(|a, b| a.avg_score.partial_cmp(&b.avg_score).unwrap())
    {
        println!(
            "  🏆 Best Average Score: {} ({:.0} points)",
            best_avg.ai_name, best_avg.avg_score
        );
    }

    if let Some(best_max) = stats.iter().max_by_key(|s| s.max_score) {
        println!(
            "  💥 Best Max Score: {} ({} points)",
            best_max.ai_name, best_max.max_score
        );
    }

    if let Some(best_success) = stats
        .iter()
        .max_by(|a, b| a.success_rate.partial_cmp(&b.success_rate).unwrap())
    {
        if best_success.success_rate > 0.0 {
            println!(
                "  ✅ Best Success Rate: {} ({:.1}%)",
                best_success.ai_name, best_success.success_rate
            );
        }
    }

    if let Some(most_consistent) = stats
        .iter()
        .filter(|s| s.avg_score > 1000.0)  // 最低限のスコアは必要
        .min_by(|a, b| a.std_dev.partial_cmp(&b.std_dev).unwrap())
    {
        println!(
            "  📏 Most Consistent: {} (σ = {:.0})",
            most_consistent.ai_name, most_consistent.std_dev
        );
    }

    if let Some(fastest) = stats
        .iter()
        .min_by(|a, b| a.avg_time_ms.partial_cmp(&b.avg_time_ms).unwrap())
    {
        println!(
            "  ⚡ Fastest: {} ({:.1} ms/game)",
            fastest.ai_name, fastest.avg_time_ms
        );
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("🎮 Puyo Puyo AI Bot Comparison Tool");
    println!("=====================================");
    println!("Games per AI: {}", args.num_games);
    println!("Parallel threads: {}", args.parallel);
    println!("Max moves: {}", args.max_tumos);
    println!("Visible tumos: {}", args.visible_tumos);
    println!("Target score: {}", args.required_chain_score);

    // AI設定を作成
    let ai_types: Vec<AIType> = args
        .ai_types
        .split(',')
        .filter_map(|name| AIType::from_str(name.trim()))
        .collect();

    if ai_types.is_empty() {
        eprintln!("Error: No valid AI types specified");
        return Ok(());
    }

    println!(
        "\nComparing {} AI bots: {:?}",
        ai_types.len(),
        ai_types.iter().map(|t| t.name()).collect::<Vec<_>>()
    );

    // プログレスバーの設定
    let multi_progress = MultiProgress::new();
    let style = ProgressStyle::default_bar()
        .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
        .unwrap()
        .progress_chars("#>-");

    // 各AIでベンチマークを実行
    let mut all_results = HashMap::new();
    let mut all_stats = Vec::new();

    for ai_type in &ai_types {
        println!("\n⚡ Benchmarking: {}", ai_type.name());

        let progress = Arc::new(Mutex::new(
            multi_progress.add(ProgressBar::new(args.num_games as u64)),
        ));
        progress.lock().unwrap().set_style(style.clone());
        progress
            .lock()
            .unwrap()
            .set_message(format!("Running {}", ai_type.name()));

        let (name, results) = benchmark_ai(ai_type, &args, progress.clone());

        progress
            .lock()
            .unwrap()
            .finish_with_message(format!("✅ {} complete", ai_type.name()));

        let stats = Statistics::from_results(name.clone(), &results, args.required_chain_score);

        if args.verbose {
            println!("\n  Results for {}:", ai_type.name());
            println!(
                "    Average Score: {:.0} ± {:.0}",
                stats.avg_score, stats.std_dev
            );
            println!("    Max Score: {}", stats.max_score);
            println!("    Success Rate: {:.1}%", stats.success_rate);
        }

        all_stats.push(stats);
        all_results.insert(name, results);
    }

    // 比較表を表示
    print_comparison_table(&all_stats);

    // JSON出力
    if let Some(output_path) = args.output_json {
        #[derive(Serialize)]
        struct BenchmarkResult {
            timestamp: String,
            args: BenchmarkArgs,
            statistics: Vec<Statistics>,
            detailed_results: Option<HashMap<String, Vec<GameResult>>>,
        }

        #[derive(Serialize)]
        struct BenchmarkArgs {
            num_games: usize,
            parallel: usize,
            max_tumos: usize,
            visible_tumos: usize,
            required_chain_score: usize,
        }

        let benchmark_result = BenchmarkResult {
            timestamp: chrono::Utc::now().to_rfc3339(),
            args: BenchmarkArgs {
                num_games: args.num_games,
                parallel: args.parallel,
                max_tumos: args.max_tumos,
                visible_tumos: args.visible_tumos,
                required_chain_score: args.required_chain_score,
            },
            statistics: all_stats,
            detailed_results: if args.verbose {
                Some(all_results)
            } else {
                None
            },
        };

        let json = serde_json::to_string_pretty(&benchmark_result)?;
        std::fs::write(&output_path, json)?;
        println!("\n📁 Results saved to: {}", output_path);
    }

    Ok(())
}