use clap::{Parser, Subcommand};
use colored::*;
use comfy_table::{presets::UTF8_FULL, Table};
use rust_aggregator::{
    utils, Aggregator, Config, OptimizationStrategy, Result,
};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "dex")]
#[command(about = "DEX Aggregator - Find the best swap routes across multiple DEXs", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Output as JSON
    #[arg(short, long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch pools from a DEX factory
    FetchPools {
        /// Factory contract address
        #[arg(long)]
        factory: String,

        /// DEX name (e.g., "Uniswap", "SushiSwap")
        #[arg(long, default_value = "Uniswap")]
        name: String,

        /// Maximum number of pools to fetch
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Fetch pools from all supported DEXes
    FetchAllDexes {
        /// Maximum number of pools to fetch per DEX
        #[arg(long, default_value = "100")]
        limit: usize,
    },

    /// Get best swap quote
    Quote {
        /// Input token address or symbol
        token_in: String,

        /// Output token address or symbol
        token_out: String,

        /// Amount to swap
        amount: String,

        /// Optimization strategy
        #[arg(long, default_value = "balanced")]
        optimize: String,

        /// Refresh pool data before quoting
        #[arg(long)]
        refresh: bool,

        /// Show top N alternative routes for comparison
        #[arg(long)]
        show_alternatives: Option<usize>,
    },

    /// List cached pools
    ListPools {
        /// Filter by token address
        #[arg(long)]
        token: Option<String>,
    },

    /// Cache management
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
}

#[derive(Subcommand)]
enum CacheAction {
    /// Export cache to file
    Export {
        /// Output file path
        #[arg(default_value = "./cache/pools.json")]
        path: String,
    },

    /// Import cache from file
    Import {
        /// Input file path
        #[arg(default_value = "./cache/pools.json")]
        path: String,
    },

    /// Show cache statistics
    Stats,

    /// Clear cache
    Clear,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .without_time()
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    // Load configuration
    let config = match Config::from_env() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            eprintln!("\nPlease create a .env file with your RPC_URL.");
            eprintln!("See .env.example for reference.");
            std::process::exit(1);
        }
    };

    // Create aggregator
    let aggregator = match Aggregator::new(config).await {
        Ok(agg) => agg,
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            std::process::exit(1);
        }
    };

    // Execute command
    let result = match cli.command {
        Commands::FetchPools { factory, name, limit } => {
            handle_fetch_pools(&aggregator, &factory, &name, limit, cli.json).await
        }
        Commands::FetchAllDexes { limit } => {
            handle_fetch_all_dexes(&aggregator, limit, cli.json).await
        }
        Commands::Quote {
            token_in,
            token_out,
            amount,
            optimize,
            refresh,
            show_alternatives,
        } => handle_quote(&aggregator, &token_in, &token_out, &amount, &optimize, refresh, show_alternatives, cli.json).await,
        Commands::ListPools { token } => handle_list_pools(&aggregator, token.as_deref(), cli.json),
        Commands::Cache { action } => handle_cache(&aggregator, action, cli.json),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

async fn handle_fetch_pools(
    aggregator: &Aggregator,
    factory: &str,
    name: &str,
    limit: Option<usize>,
    json_output: bool,
) -> Result<()> {
    let factory_addr = utils::parse_address(factory)?;

    if !json_output {
        println!("\n{}", "━".repeat(60).bright_cyan());
        println!("{}  {}", "".to_string(), "Fetching Pool Data".bright_cyan().bold());
        println!("{}", "━".repeat(60).bright_cyan());
        println!("  DEX:     {}", name.bright_white().bold());
        println!("  Factory: {}", factory.bright_black());
        println!("  Limit:   {}", limit.map(|l| l.to_string()).unwrap_or_else(|| "All".to_string()).bright_black());
        println!();
    }

    let pools = aggregator.fetch_pools(factory_addr, name.to_string(), limit).await?;

    // Export to cache
    aggregator.export_cache("./cache/pools.json")?;

    if json_output {
        let output = serde_json::json!({
            "success": true,
            "pools_fetched": pools.len(),
            "dex": name,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("{} {}", "".to_string(), "Success!".bright_green().bold());
        println!("  Pools fetched: {}", pools.len().to_string().bright_yellow().bold());
        println!("  Cache saved:   {}", "./cache/pools.json".bright_cyan());
        println!("{}", "━".repeat(60).bright_cyan());
        println!();
    }

    Ok(())
}

async fn handle_fetch_all_dexes(
    aggregator: &Aggregator,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    if !json_output {
        println!("\n{}", "━".repeat(60).bright_cyan());
        println!("{}  {}", "".to_string(), "Fetching Pools from All DEXes".bright_cyan().bold());
        println!("{}", "━".repeat(60).bright_cyan());
        println!();
    }

    let factories = aggregator.get_config().get_all_factories();
    let mut total_fetched = 0;
    let mut dex_results = Vec::new();

    for (dex_name, factory_addr) in factories {
        if !json_output {
            println!("  {} Fetching from {}...", "→".bright_yellow(), dex_name.bright_white().bold());
        }

        match aggregator.fetch_pools(factory_addr, dex_name.clone(), Some(limit)).await {
            Ok(pools) => {
                let count = pools.len();
                total_fetched += count;
                dex_results.push((dex_name.clone(), count, true));
                
                if !json_output {
                    println!("    {} {} pools", "✓".bright_green(), count.to_string().bright_yellow());
                }
            }
            Err(e) => {
                dex_results.push((dex_name.clone(), 0, false));
                if !json_output {
                    println!("    {} Failed: {}", "✗".bright_red(), e.to_string().bright_red());
                }
            }
        }
    }

    // Export to cache
    aggregator.export_cache("./cache/pools.json")?;

    if json_output {
        let results: Vec<_> = dex_results.iter().map(|(name, count, success)| {
            serde_json::json!({
                "dex": name,
                "pools_fetched": count,
                "success": success
            })
        }).collect();
        
        let output = serde_json::json!({
            "success": true,
            "total_pools": total_fetched,
            "dexes": results,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!();
        println!("{} {}", "".to_string(), "Summary".bright_green().bold());
        println!("  Total pools fetched: {}", total_fetched.to_string().bright_yellow().bold());
        println!("  Cache saved:         {}", "./cache/pools.json".bright_cyan());
        println!("{}", "━".repeat(60).bright_cyan());
        println!();
    }

    Ok(())
}

async fn handle_quote(
    aggregator: &Aggregator,
    token_in: &str,
    token_out: &str,
    amount_str: &str,
    optimize: &str,
    refresh: bool,
    show_alternatives: Option<usize>,
    json_output: bool,
) -> Result<()> {
    // Refresh pools if requested
    if refresh {
        if !json_output {
            println!("\n{} Refreshing pool data...", "".bright_cyan());
        }
        
        let factories = aggregator.get_config().get_all_factories();
        for (dex_name, factory_addr) in factories {
            let _ = aggregator.fetch_pools(factory_addr, dex_name, Some(100)).await;
        }
        aggregator.export_cache("./cache/pools.json")?;
        
        if !json_output {
            println!("{} Pool data refreshed!\n", "✓".bright_green());
        }
    }

    // Parse token symbols or addresses
    let token_in_addr = utils::parse_token(token_in)?;
    let token_out_addr = utils::parse_token(token_out)?;

    // Get token decimals for proper parsing
    let token_in_decimals = utils::get_token_decimals(token_in_addr);
    let amount_in = utils::parse_token_amount(amount_str, token_in_decimals)?;

    // Parse optimization strategy
    let strategy = match optimize.to_lowercase().as_str() {
        "price" => OptimizationStrategy::Price,
        "gas" => OptimizationStrategy::Gas,
        "slippage" => OptimizationStrategy::Slippage,
        "balanced" => OptimizationStrategy::Balanced,
        _ => OptimizationStrategy::Balanced,
    };

    if !json_output {
        println!();
        println!("{}", "═".repeat(70).bright_cyan());
        println!("{:^70}", "ROUTE SEARCH".bright_cyan().bold());
        println!("{}", "═".repeat(70).bright_cyan());
        println!("  {:<20} {:?}", "STRATEGY".bright_white().bold(), strategy);
        println!("  {:<20} {} → {}", 
            "PAIR".bright_white().bold(),
            utils::get_token_symbol(token_in_addr).bright_cyan(),
            utils::get_token_symbol(token_out_addr).bright_cyan()
        );
        println!();
    }

    // Get top N quotes if alternatives requested, otherwise just get best
    let limit = show_alternatives.map(|n| n + 1).unwrap_or(1); // +1 to include best route
    let quotes = aggregator.get_top_quotes(token_in_addr, token_out_addr, amount_in, strategy, limit)?;
    let quote = &quotes[0]; // Best quote

    if json_output {
        let output = serde_json::json!({
            "token_in": format!("{:?}", quote.token_in),
            "token_out": format!("{:?}", quote.token_out),
            "amount_in": quote.amount_in.to_string(),
            "amount_out": quote.amount_out.to_string(),
            "rate": quote.exchange_rate(),
            "hops": quote.hop_count(),
            "gas_estimate": quote.gas_estimate.to_string(),
            "price_impact_bps": quote.price_impact_bps,
            "route": quote.description,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        print_quote(quote);

        // Print alternative routes if requested
        if let Some(alt_count) = show_alternatives {
            if quotes.len() > 1 {
                print_alternative_routes(&quotes[1..], alt_count, token_in_addr, token_out_addr);
            }
        }
    }

    Ok(())
}

fn handle_list_pools(aggregator: &Aggregator, token_filter: Option<&str>, json_output: bool) -> Result<()> {
    let pools = if let Some(token_str) = token_filter {
        let token_addr = utils::parse_address(token_str)?;
        aggregator.get_pools_with_token(token_addr)
    } else {
        aggregator.get_pools()
    };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&pools).unwrap());
    } else {
        if pools.is_empty() {
            println!("\n{}", "━".repeat(60).bright_yellow());
            println!("{}  {}", "".to_string(), "No Pools Found".bright_yellow().bold());
            println!("{}", "━".repeat(60).bright_yellow());
            println!("\n  {}", "Tip: Fetch pools first with:".bright_black());
            println!("  {}", "dex fetch-pools --factory 0x5C69... --limit 100".bright_cyan());
            println!();
            return Ok(());
        }

        println!("\n{}", "━".repeat(60).bright_cyan());
        println!("{}  {} - {} pools", "".to_string(), "Cached Pools".bright_cyan().bold(), pools.len().to_string().bright_yellow().bold());
        println!("{}", "━".repeat(60).bright_cyan());
        println!();

        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(vec![
            "DEX".bright_white().bold().to_string(),
            "Token0".bright_white().bold().to_string(),
            "Token1".bright_white().bold().to_string(),
            "Reserve0".bright_white().bold().to_string(),
            "Reserve1".bright_white().bold().to_string(),
        ]);

        for pool in pools.iter().take(20) {
            table.add_row(vec![
                pool.dex_name.bright_cyan().to_string(),
                format!("{:?}", pool.token0)[..10].bright_black().to_string(),
                format!("{:?}", pool.token1)[..10].bright_black().to_string(),
                utils::format_token_amount(pool.reserve0, 18).bright_green().to_string(),
                utils::format_token_amount(pool.reserve1, 18).bright_green().to_string(),
            ]);
        }

        println!("{}", table);

        if pools.len() > 20 {
            println!("\n  {} and {} more pools", "...".bright_black(), (pools.len() - 20).to_string().bright_yellow());
        }
        println!();
    }

    Ok(())
}

fn handle_cache(aggregator: &Aggregator, action: CacheAction, json_output: bool) -> Result<()> {
    match action {
        CacheAction::Export { path } => {
            aggregator.export_cache(&path)?;
            if !json_output {
                println!("\n{} {}", "".to_string(), "Cache Exported".bright_green().bold());
                println!("  Location: {}", path.bright_cyan());
                println!();
            }
        }
        CacheAction::Import { path } => {
            let count = aggregator.import_cache(&path)?;
            if json_output {
                println!("{}", serde_json::json!({"pools_imported": count}));
            } else {
                println!("\n{} {}", "".to_string(), "Cache Imported".bright_green().bold());
                println!("  Pools loaded: {}", count.to_string().bright_yellow().bold());
                println!("  From: {}", path.bright_cyan());
                println!();
            }
        }
        CacheAction::Stats => {
            let stats = aggregator.get_cache_stats();
            if json_output {
                let output = serde_json::json!({
                    "total_pools": stats.total_pools,
                    "dex_counts": stats.dex_counts,
                });
                println!("{}", serde_json::to_string_pretty(&output).map_err(|e| {
                    rust_aggregator::AggregatorError::Other(anyhow::anyhow!("JSON error: {}", e))
                })?);
            } else {
                println!();
                println!("{}", "═".repeat(70).bright_cyan());
                println!("{:^70}", "CACHE STATISTICS".bright_cyan().bold());
                println!("{}", "═".repeat(70).bright_cyan());
                println!();
                println!("  {:<20} {}", 
                    "TOTAL POOLS".bright_white().bold(),
                    stats.total_pools.to_string().bright_yellow().bold()
                );
                println!();
                
                if !stats.dex_counts.is_empty() {
                    println!("  {}", "BREAKDOWN BY DEX".bright_white().bold());
                    for (dex, count) in stats.dex_counts {
                        println!("  {:<20} {} pools", 
                            format!("  {}", dex).bright_cyan(),
                            count.to_string().bright_yellow()
                        );
                    }
                    println!();
                }
                println!("{}", "═".repeat(70).bright_cyan());
                println!();
            }
        }
        CacheAction::Clear => {
            aggregator.clear_cache();
            if !json_output {
                println!();
                println!("{}", "Cache cleared successfully".bright_green().bold());
                println!();
                println!();
            }
        }
    }

    Ok(())
}

fn print_quote(quote: &rust_aggregator::RouteQuote) {
    // Get decimals and symbols for input and output tokens
    let token_in_decimals = utils::get_token_decimals(quote.token_in);
    let token_out_decimals = utils::get_token_decimals(quote.token_out);
    let token_in_symbol = utils::get_token_symbol(quote.token_in);
    let token_out_symbol = utils::get_token_symbol(quote.token_out);
    
    println!();
    println!("{}", "═".repeat(70).bright_green());
    println!("{:^70}", "BEST ROUTE FOUND".bright_green().bold());
    println!("{}", "═".repeat(70).bright_green());
    println!();

    // Route visualization with colors and token symbols
    let route_addresses: Vec<&str> = quote.description.split(" → ").collect();
    let route_parts: Vec<String> = route_addresses
        .iter()
        .enumerate()
        .map(|(i, addr)| {
            // Parse address and get symbol
            let token_addr = utils::parse_address(addr).unwrap_or_else(|_| quote.token_in);
            let symbol = utils::get_token_symbol(token_addr);
            
            if i == 0 {
                format!("{}", symbol.bright_cyan().bold())
            } else {
                format!("{} {}", "→".bright_black(), symbol.bright_cyan().bold())
            }
        })
        .collect();
    
    println!("  {:<20} {}", "ROUTE".bright_white().bold(), route_parts.join(" "));
    println!("  {:<20} {} {}", 
        "HOPS".bright_white().bold(),
        quote.hop_count().to_string().bright_yellow().bold(),
        if quote.hop_count() == 1 { "hop" } else { "hops" }.bright_black()
    );
    println!();

    // Show hop-by-hop breakdown for multi-hop routes
    if quote.hop_count() > 1 {
        println!("{}", "─".repeat(70).bright_black());
        println!("{:^70}", "ROUTE DETAILS".bright_yellow().bold());
        println!("{}", "─".repeat(70).bright_black());
        println!();
        
        for (i, hop) in quote.hops.iter().enumerate() {
            let hop_token_in = utils::get_token_symbol(hop.token_in);
            let hop_token_out = utils::get_token_symbol(hop.token_out);
            let hop_token_in_decimals = utils::get_token_decimals(hop.token_in);
            let hop_token_out_decimals = utils::get_token_decimals(hop.token_out);
            
            let hop_amount_in = utils::format_token_amount(hop.amount_in, hop_token_in_decimals);
            let hop_amount_out = utils::format_token_amount(hop.amount_out, hop_token_out_decimals);
            
            // Calculate hop rate
            let hop_in_f64 = hop.amount_in.as_u128() as f64 / 10f64.powi(hop_token_in_decimals as i32);
            let hop_out_f64 = hop.amount_out.as_u128() as f64 / 10f64.powi(hop_token_out_decimals as i32);
            let hop_rate = if hop_in_f64 > 0.0 {
                hop_out_f64 / hop_in_f64
            } else {
                0.0
            };
            
            println!("  {} {}", 
                format!("HOP {}", i + 1).bright_white().bold(),
                format!("{} → {}", hop_token_in, hop_token_out).bright_cyan(),
            );
            println!("  {:<18} {} {} {} {} {}", 
                "".to_string(),
                hop_amount_in.bright_white(),
                hop_token_in.bright_cyan(),
                "→".bright_black(),
                hop_amount_out.bright_white(),
                hop_token_out.bright_cyan()
            );
            println!("  {:<18} {}", 
                "".to_string(),
                format!("@ {:.6} {} per {}", hop_rate, hop_token_out, hop_token_in).bright_black()
            );
            println!("  {:<18} {} {}", 
                "".to_string(),
                "via".bright_black(),
                hop.dex_name.bright_yellow()
            );
            println!();
        }
    }

    println!("{}", "─".repeat(70).bright_black());
    println!("{:^70}", "QUOTE SUMMARY".bright_blue().bold());
    println!("{}", "─".repeat(70).bright_black());
    println!();
    
    // Calculate proper exchange rate using decimals
    let amount_in_f64 = quote.amount_in.as_u128() as f64 / 10f64.powi(token_in_decimals as i32);
    let amount_out_f64 = quote.amount_out.as_u128() as f64 / 10f64.powi(token_out_decimals as i32);
    let rate = if amount_in_f64 > 0.0 {
        amount_out_f64 / amount_in_f64
    } else {
        0.0
    };

    println!("  {:<20} {} {}", 
        "INPUT".bright_white().bold(),
        utils::format_token_amount(quote.amount_in, token_in_decimals).bright_cyan().bold(),
        token_in_symbol.bright_cyan()
    );
    println!("  {:<20} {} {}", 
        "OUTPUT".bright_white().bold(),
        utils::format_token_amount(quote.amount_out, token_out_decimals).bright_green().bold(),
        token_out_symbol.bright_green()
    );
    println!("  {:<20} {}", 
        "EXCHANGE RATE".bright_white().bold(), 
        format!("{:.6} {}/{}", rate, token_out_symbol, token_in_symbol).bright_yellow().bold()
    );
    println!();

    println!("{}", "─".repeat(70).bright_black());
    println!("{:^70}", "COST ANALYSIS".bright_magenta().bold());
    println!("{}", "─".repeat(70).bright_black());
    println!();
    
    let impact = quote.price_impact_bps as f64 / 100.0;
    let impact_color = if impact < 0.5 {
        "green"
    } else if impact < 1.0 {
        "yellow"
    } else {
        "red"
    };
    
    let impact_str = format!("{:.2}%", impact);
    let colored_impact = match impact_color {
        "green" => impact_str.bright_green(),
        "yellow" => impact_str.bright_yellow(),
        "red" => impact_str.bright_red(),
        _ => impact_str.normal(),
    };
    
    println!("  {:<20} {} gas", 
        "GAS ESTIMATE".bright_white().bold(), 
        quote.gas_estimate.to_string().bright_yellow()
    );
    
    println!("  {:<20} {}", 
        "PRICE IMPACT".bright_white().bold(), 
        colored_impact.bold()
    );
    
    println!();
    println!("{}", "═".repeat(70).bright_green());
    println!();
}

fn print_alternative_routes(
    alternatives: &[rust_aggregator::RouteQuote],
    limit: usize,
    _token_in: ethers::types::Address,
    token_out: ethers::types::Address,
) {
    use ethers::types::Address;
    
    let token_out_decimals = utils::get_token_decimals(token_out);
    let token_out_symbol = utils::get_token_symbol(token_out);

    println!("{}", "═".repeat(70).bright_yellow());
    println!("{:^70}", 
        format!("ALTERNATIVE ROUTES (Top {})", limit.min(alternatives.len())).bright_yellow().bold()
    );
    println!("{}", "═".repeat(70).bright_yellow());
    println!();

    for (idx, quote) in alternatives.iter().take(limit).enumerate() {

        println!("  {} {}", 
            format!("OPTION {}", idx + 1).bright_white().bold(),
            format!("(score: {:.0})", quote.score).bright_black()
        );

        // Show route path with symbols
        let route_tokens: Vec<Address> = quote.hops.iter()
            .flat_map(|hop| vec![hop.token_in, hop.token_out])
            .collect::<Vec<_>>();
        
        let mut seen = std::collections::HashSet::new();
        let unique_tokens: Vec<Address> = route_tokens.into_iter()
            .filter(|t| seen.insert(*t))
            .collect();

        let route_symbols: Vec<String> = unique_tokens.iter()
            .map(|addr| utils::get_token_symbol(*addr))
            .collect();

        println!("  {:<18} {}", 
            "".to_string(),
            route_symbols.join(" → ").bright_cyan()
        );

        // Show which DEXes are used
        let dex_names: Vec<String> = quote.hops.iter()
            .map(|hop| hop.dex_name.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        println!("  {:<18} {}", 
            "".to_string(),
            format!("via {}", dex_names.join(", ")).bright_black()
        );

        println!("  {:<18} {} {}", 
            "".to_string(),
            utils::format_token_amount(quote.amount_out, token_out_decimals).bright_green(),
            token_out_symbol.bright_green()
        );

        println!("  {:<18} {} gas | impact: {}",
            "".to_string(),
            quote.gas_estimate.to_string().bright_yellow(),
            format!("{:.2}%", quote.price_impact_bps as f64 / 100.0).bright_black()
        );
        println!();
    }

    println!("{}", "═".repeat(70).bright_yellow());
    println!();
}
