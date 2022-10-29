use std::io::Write;
use ocl::{MemFlags, ProQue, SpatialDims};
use anyhow::Result;

fn debug_print(v: bool, msg: &str) {
  if v {
    eprintln!("akpull-cl debug: {}", msg);
  }
}

pub(crate) struct AkPullArgs {
  pub verbose: bool,
  pub ntrials: u64,
  pub npulls: Vec<u64>,
  pub queries: Vec<(String, String)>,

  pub n6: u64,
  pub n5: u64,
  pub n6p: u64,
  pub rate6b: u64,
  pub rate5b: u64,
  pub stdpool: u64
}

pub(crate) struct AkPullResult {
  pub counts: Vec<u64>
}

const ENABLE_EXTS: &'static str = r#"
#pragma OPENCL EXTENSION cl_khr_int64_base_atomics : enable
"#;

const XOROSHIRO256PP: &'static str = r#"
// Implementation of xoroshiro256++
ulong4 rng_step(ulong4 state) {
  ulong t = state.s1 << 17;
  state.s2 ^= state.s0;
  state.s3 ^= state.s1;
  state.s1 ^= state.s2;
  state.s0 ^= state.s3;
  state.s2 ^= t;
  state.s3 = rotate(state.s3, (ulong)45);
  return state;
}

ulong rng_read(ulong4 state) {
  return rotate(state.s0 + state.s3, (ulong)23) + state.s0;
}

ulong4 rng_init(ulong seed) {
  // Digits of pi in hexadecimal and e in hexadecimal
  ulong4 state = (ulong4)(seed, seed ^ 0x243F6A8885A308D3ull, ~seed, seed ^ 0x93C467E37DB0C7A4ull);
  for(int i = 0; i < 10; i++) {
    state = rng_step(state);
  }
  return state;
}
"#;

const KERNEL_GENERIC_START: &'static str = r#"
__kernel void k(__global ulong* counts, __constant ulong* npulls) {
  uint banner6s[16] = { };
  uint banner5s[16] = { };
  uint banner6 = 0;
  uint banner5 = 0;
  uint off6 = 0;
  uint off5 = 0;

  int pity6 = 0;
  int pity5 = 0;
  ulong4 state = rng_init(get_global_id(0));

  for(uint i = 0; i < NNPULLS; i++) {
    for(uint j = 0; j < npulls[i]; j++) {

      ulong r1 = rng_read(state);
      state = rng_step(state);
      ulong r2 = rng_read(state);
      state = rng_step(state);
      ulong r3 = rng_read(state);
      state = rng_step(state);

      int pity6_add = max((int)2, (int)(2 * (pity6 - 49)));
      r1 %= 100;
      r2 %= 100;
      if(r1 < pity6_add) {
        // 6*
        pity6 = 0;
        pity5 = 10;
        if(r2 < RATE6B) {
          r3 %= N6;
          banner6s[r3]++;
          banner6++;
        } else {
          off6++;
        }
      } else if(r1 < 8 + pity6_add || pity5 == 9) {
        // 5*
        pity5 = 10;
        if(r2 < RATE5B) {
          r3 %= N5;
          banner5s[r3]++;
          banner5++;
        } else {
          off5++;
        }
      } else {
        pity6++;
        pity5++;
      }

    }

"#;
const KERNEL_GENERIC_END: &'static str = r#"
  }
}
"#;

pub(crate) fn akpull(args: &AkPullArgs) -> Result<AkPullResult> {
  let v = args.verbose;
  debug_print(v, &format!("n6: {}", args.n6));
  debug_print(v, &format!("n5: {}", args.n5));
  debug_print(v, &format!("n6p: {}", args.n6p));
  debug_print(v, &format!("rate6b: {}", args.rate6b));
  debug_print(v, &format!("rate5b: {}", args.rate5b));
  debug_print(v, &format!("stdpool: {}", args.stdpool));
  debug_print(v, "preprocessing kernels...");
  // Preprocess and create shaders
  let mut src = String::new();
  src += ENABLE_EXTS;
  src += XOROSHIRO256PP;
  src += &format!(r#"
  __constant ulong N6 = {};
  __constant ulong N5 = {};
  __constant ulong N6PREVLIM = {};
  __constant ulong RATE6B = {};
  __constant ulong RATE5B = {};
  __constant ulong STDPOOL = {};
  __constant ulong NNPULLS = {};
"#, args.n6, args.n5, args.n6p, args.rate6b, args.rate5b, args.stdpool, args.npulls.len());
  src += KERNEL_GENERIC_START;
  for i in 0..args.queries.len() {
    src += &["atom_add(counts + ", &(i * args.npulls.len()).to_string(), " + i, !!(", &args.queries[i].1, "));\n"].concat();
  }
  src += KERNEL_GENERIC_END;
  let ctx = ProQue::builder()
    .src(src)
    .dims(SpatialDims::One(args.ntrials as usize))
    .build()?;
  let device = ctx.device();
  debug_print(v, &format!("selected device: {} {} CL {}", device.vendor()?, device.name()?, device.version()?));
  let npulls_buf = ctx.buffer_builder::<u64>()
    .len(SpatialDims::One(args.npulls.len()))
    .flags(MemFlags::READ_ONLY)
    .copy_host_slice(args.npulls.as_slice())
    .build()?;
  let result_buf = ctx.buffer_builder::<u64>()
    .len(SpatialDims::One(args.queries.len() * args.npulls.len())) // [query * nqueries + pullsidx]
    .fill_val(0)
    .build()?;
  let kernel = ctx.kernel_builder("k")
    .arg(&result_buf)
    .arg(&npulls_buf)
    .build()?;
  debug_print(v, "executing kernel...");
  unsafe {
    kernel.enq()?;
  }

  let mut data = vec![727; result_buf.len()];
  result_buf.read(&mut data).enq()?;

  Ok(AkPullResult {
    counts: data
  })
}