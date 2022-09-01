mod akpull;

use std::io::stdout;
use anyhow::Result;
use clap::{ValueEnum};
use clap::Parser;
use crate::akpull::{akpull, AkPullArgs};

#[derive(Clone, Debug, ValueEnum)]
enum BannerType {
  Standard,
  Limited,
  Event,
  Custom
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
  /// Verbose output
  #[clap(short, long, action)]
  verbose: bool,

  /// Type of banner
  #[clap(short, long, value_parser, default_value = "standard")]
  banner: BannerType,

  /// Number of pulls (can specify multiple)
  #[clap(short, long, value_parser, default_value = "150")]
  pulls: Vec<u64>,

  /// Number of trials
  #[clap(short, long, value_parser, default_value = "10000000")]
  ntrials: u64,

  /// Number of 6*s on the banner
  #[clap(long, value_parser)]
  n6: Option<u64>,

  /// Number of 5*s on the banner
  #[clap(long, value_parser)]
  n5: Option<u64>,

  /// Number of previous 6* limiteds on the banner
  #[clap(long, value_parser)]
  n6p: Option<u64>,

  /// Rate of 6*s on the banner
  #[clap(long, value_parser)]
  rate6b: Option<u64>,

  /// Rate of 5*s on the banner
  #[clap(long, value_parser)]
  rate5b: Option<u64>,

  /// Number of characters in the standard pool
  #[clap(long, value_parser)]
  stdpool: Option<u64>,

  /// Don't override built-in queries if own queries provided
  #[clap(long, action)]
  builtin: bool,

  /// <query_name>;<expression>, <expression> is a C expression that returns bool
  #[clap(short, long, value_parser)]
  query: Vec<String>
}

fn main() -> Result<()> {
  let args = Args::parse();
  // Default parameters
  let mut pull_args = AkPullArgs {
    verbose: args.verbose,
    ntrials: args.ntrials,
    npulls: args.pulls,
    queries: args.query.into_iter()
      .map(|x| Option::Some((x.split_once(";")?.0.to_owned(), x.split_once(";")?.1.to_owned())))
      .filter(|x| x.is_some())
      .map(|x| x.unwrap())
      .map(|x| (x.0.to_owned(), x.1.to_owned()))
      .collect(),
    n6: 1,
    n5: 1,
    n6p: 0,
    rate6b: 50,
    rate5b: 50,
    stdpool: 44 // After Goldenglow
  };
  // Preload queries for banner preset
  if pull_args.queries.is_empty() || args.builtin {
    // All banners
    let std_queries_a = vec![
      ("1x 6*".into(), "(banner6 + off6) >= 1".into()),
      ("2x 6*".into(), "(banner6 + off6) >= 2".into()),
      ("3x 6*".into(), "(banner6 + off6) >= 3".into()),
      ("4x 6*".into(), "(banner6 + off6) >= 4".into()),
      ("5x 6*".into(), "(banner6 + off6) >= 5".into()),
      ("6x 6*".into(), "(banner6 + off6) >= 6".into()),
      ("Specific 6*".into(), "banner6s[0] > 0".into()),
      ("Specific 6* Max Pot".into(), "banner6s[0] >= 6".into()),
      ("Specific 5*".into(), "banner5s[0] > 0".into()),
      ("Specific 5* Max Pot".into(), "banner5s[0] >= 6".into())
    ];
    // Only applies to banners with more than 2 rateups
    let std_queries_b = vec![
      ("Both 6*".into(), "banner6s[0] > 0 && banner6s[1] > 0".into()),
      ("Both 6* Max Pot".into(), "banner6s[0] >= 6 && banner6s[1] >= 6".into())
    ];
    // Only applies to limiteds
    let std_queries_c = vec![
      //TODO
    ];
    pull_args.queries.extend(match args.banner {
      BannerType::Standard => [std_queries_a, std_queries_b].concat(),
      BannerType::Limited => [std_queries_a, std_queries_b, std_queries_c].concat(),
      BannerType::Event => std_queries_a,
      BannerType::Custom => vec![]
    });
  }
  // Preload parameters for banner preset
  match args.banner {
    BannerType::Standard => {
      pull_args.n6 = 2;
      pull_args.n5 = 3;
      pull_args.n6p = 0;
      pull_args.rate6b = 50;
      pull_args.rate6b = 50;
    },
    BannerType::Event => {
      pull_args.n6 = 1;
      pull_args.n5 = 2;
      pull_args.n6p = 0;
      pull_args.rate6b = 50;
      pull_args.rate5b = 50;
    },
    BannerType::Limited => {
      pull_args.n6 = 2;
      pull_args.n5 = 1;
      pull_args.n6p = 0;
      pull_args.rate6b = 70;
      pull_args.rate5b = 50;
    },
    _ => {}
  };
  // Load remaining parameters
  if args.n6.is_some() { pull_args.n6 = args.n6.unwrap(); }
  if args.n5.is_some() { pull_args.n5 = args.n5.unwrap(); }
  if args.n6p.is_some() { pull_args.n6p = args.n6p.unwrap(); }
  if args.rate6b.is_some() { pull_args.rate6b = args.rate6b.unwrap(); }
  if args.rate5b.is_some() { pull_args.rate5b = args.rate5b.unwrap(); }
  if args.stdpool.is_some() { pull_args.stdpool = args.stdpool.unwrap(); }

  let result = akpull(&pull_args, &mut stdout().lock())?;
  print!("\x1b[92m{:25}", "");
  for n in pull_args.npulls.as_slice() {
    print!(" {:>6.5}", n);
  }
  println!("\x1b[0m");
  let mut i = 0;
  let total_counts = pull_args.ntrials as f64;
  for q in pull_args.queries {
    print!("\x1b[96m{:>25.25}\x1b[0m", q.0);
    for j in 0..pull_args.npulls.len() {
      let count = result.counts[i] as f64;
      let percentage = 100f64 * (count / total_counts);
      print!(" {:>6.2}", percentage);
      i += 1;
    }
    println!();
  }
  Ok(())
}
