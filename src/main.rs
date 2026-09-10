mod target {
    use std::cmp::Ordering;

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct Uint256(pub [u64; 4]);

    impl Uint256 {
        pub const ZERO: Self = Self([0; 4]);

        #[inline]
        pub fn is_zero(self) -> bool {
            self.0.iter().all(|&x| x == 0)
        }

        #[inline]
        pub fn to_be_bytes(self) -> [u8; 32] {
            let mut out = [0u8; 32];
            for (dst, word) in out.chunks_exact_mut(8).zip(self.0.iter().rev()) {
                dst.copy_from_slice(&word.to_be_bytes());
            }
            out
        }

    }

    impl Ord for Uint256 {
        #[inline]
        fn cmp(&self, other: &Self) -> Ordering {
            self.0.iter().rev().cmp(other.0.iter().rev())
        }
    }

    impl PartialOrd for Uint256 {
        #[inline]
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    pub fn target_from_difficulty(difficulty: f32) -> Result<Uint256, String> {
        if !difficulty.is_finite() || difficulty <= 0.0 {
            return Err(format!("invalid difficulty {difficulty}"));
        }

        let reciprocal = 1.0f32 / difficulty;
        let (mantissa, exponent, sign) = integer_decode_f32(reciprocal);
        if sign < 0 {
            return Err("difficulty reciprocal is negative".into());
        }

        const DIFF1_MANTISSA: u64 = 0xffff;
        const DIFF1_EXPONENT: i16 = 208;
        let new_mantissa = mantissa
            .checked_mul(DIFF1_MANTISSA)
            .ok_or_else(|| "difficulty target mantissa overflow".to_string())?;
        let new_exponent = DIFF1_EXPONENT as i32 + exponent as i32;
        if new_exponent < 0 {
            return Ok(Uint256::ZERO);
        }

        let start = (new_exponent as usize) / 64;
        let remainder = (new_exponent as usize) % 64;
        if start >= 4 {
            return Err("target is too big".into());
        }

        let mut buf = [0u64; 4];
        buf[start] = new_mantissa << remainder;
        if remainder != 0 {
            let carry = new_mantissa >> (64 - remainder);
            if start + 1 < 4 {
                buf[start + 1] = carry;
            } else if carry != 0 {
                return Err("target is too big".into());
            }
        }
        Ok(Uint256(buf))
    }

    fn integer_decode_f32(value: f32) -> (u64, i16, i8) {
        let bits = value.to_bits();
        let sign = if bits >> 31 == 0 { 1 } else { -1 };
        let mut exponent = ((bits >> 23) & 0xff) as i16;
        let mantissa = if exponent == 0 {
            (bits & 0x7fffff) << 1
        } else {
            (bits & 0x7fffff) | 0x800000
        };
        exponent -= 127 + 23;
        (mantissa as u64, exponent, sign)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn difficulty_one_matches_expected_target() {
            let t = target_from_difficulty(1.0).unwrap();
            assert_eq!(t.0, [0, 0, 0, 0x0000_0000_ffff_0000]);
        }

        #[test]
        fn higher_difficulty_lowers_target() {
            assert!(target_from_difficulty(100.0).unwrap() < target_from_difficulty(1.0).unwrap());
        }
    }
}

mod pow {
    use crate::target::Uint256;
    pub const POW_INITIAL_STATE: [u64; 25] = [
        1242148031264380989, 3008272977830772284, 2188519011337848018, 1992179434288343456,
        8876506674959887717, 5399642050693751366, 1745875063082670864, 8605242046444978844,
        17936695144567157056, 3343109343542796272, 1123092876221303306, 4963925045340115282,
        17037383077651887893, 16629644495023626889, 12833675776649114147, 3784524041015224902,
        1082795874807940378, 13952716920571277634, 13411128033953605860, 15060696040649351053,
        9928834659948351306, 5237849264682708699, 12825353012139217522, 6706187291358897596,
        196324915476054915,
    ];

    pub const HEAVY_INITIAL_STATE: [u64; 25] = [
        4239941492252378377, 8746723911537738262, 8796936657246353646, 1272090201925444760,
        16654558671554924250, 8270816933120786537, 13907396207649043898, 6782861118970774626,
        9239690602118867528, 11582319943599406348, 17596056728278508070, 15212962468105129023,
        7812475424661425213, 3370482334374859748, 5690099369266491460, 8596393687355028144,
        570094237299545110, 9119540418498120711, 16901969272480492857, 13372017233735502424,
        14372891883993151831, 5171152063242093102, 10573107899694386186, 6096431547456407061,
        1592359455985097269,
    ];

    const KECCAK_RNDC: [u64; 24] = [
        0x0000000000000001, 0x0000000000008082, 0x800000000000808a, 0x8000000080008000,
        0x000000000000808b, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
        0x000000000000008a, 0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
        0x000000008000808b, 0x800000000000008b, 0x8000000000008089, 0x8000000000008003,
        0x8000000000008002, 0x8000000000000080, 0x000000000000800a, 0x800000008000000a,
        0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
    ];
    const KECCAK_ROTC: [u32; 24] = [
        1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
    ];
    const KECCAK_PILN: [usize; 24] = [
        10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
    ];

    #[inline(always)]
    pub fn keccak_f1600(st: &mut [u64; 25]) {
        for &rc in &KECCAK_RNDC {
            let mut bc = [0u64; 5];
            for i in 0..5 {
                bc[i] = st[i] ^ st[i + 5] ^ st[i + 10] ^ st[i + 15] ^ st[i + 20];
            }
            for i in 0..5 {
                let t = bc[(i + 4) % 5] ^ bc[(i + 1) % 5].rotate_left(1);
                for j in (0..25).step_by(5) {
                    st[j + i] ^= t;
                }
            }

            let mut t = st[1];
            for i in 0..24 {
                let j = KECCAK_PILN[i];
                let tmp = st[j];
                st[j] = t.rotate_left(KECCAK_ROTC[i]);
                t = tmp;
            }

            for j in (0..25).step_by(5) {
                let row = [st[j], st[j + 1], st[j + 2], st[j + 3], st[j + 4]];
                for i in 0..5 {
                    st[j + i] = row[i] ^ ((!row[(i + 1) % 5]) & row[(i + 2) % 5]);
                }
            }
            st[0] ^= rc;
        }
    }

    #[derive(Clone, Debug)]
    pub struct Job {
        pub id: String,
        pub target: Uint256,
        pub nonce_mask: u64,
        pub nonce_fixed: u64,
        pub matrix: [u8; 4096],
        pub initial_state: [u64; 25],
    }

    impl Job {
        pub fn new(
            id: String,
            pre_pow_hash: [u64; 4],
            timestamp: u64,
            target: Uint256,
            nonce_mask: u64,
            nonce_fixed: u64,
        ) -> Self {
            let mut initial_state = POW_INITIAL_STATE;
            for i in 0..4 {
                initial_state[i] ^= pre_pow_hash[i];
            }
            initial_state[4] ^= timestamp;
            let matrix = generate_matrix(pre_pow_hash);
            Self { id, target, nonce_mask, nonce_fixed, matrix, initial_state }
        }

        #[cfg(test)]
        #[inline(always)]
        pub fn nonce_for_counter(&self, counter: u64) -> u64 {
            (counter & self.nonce_mask) | self.nonce_fixed
        }

        #[inline(always)]
        pub fn calculate_pow(&self, nonce: u64) -> Uint256 {
            let mut st = self.initial_state;
            st[9] ^= nonce;
            keccak_f1600(&mut st);
            heavy_hash(&self.matrix, [st[0], st[1], st[2], st[3]])
        }

    }

    #[derive(Clone, Copy)]
    struct Xoshiro256PlusPlus([u64; 4]);
    impl Xoshiro256PlusPlus {
        #[inline]
        fn next(&mut self) -> u64 {
            let result = self.0[0].wrapping_add(self.0[0].wrapping_add(self.0[3]).rotate_left(23));
            let t = self.0[1] << 17;
            self.0[2] ^= self.0[0];
            self.0[3] ^= self.0[1];
            self.0[1] ^= self.0[2];
            self.0[0] ^= self.0[3];
            self.0[2] ^= t;
            self.0[3] = self.0[3].rotate_left(45);
            result
        }
    }

    pub fn generate_matrix(seed: [u64; 4]) -> [u8; 4096] {
        let mut rng = Xoshiro256PlusPlus(seed);
        loop {
            let mut mat = [0u8; 4096];
            for r in 0..64 {
                let mut val = 0u64;
                for c in 0..64 {
                    let shift = c % 16;
                    if shift == 0 {
                        val = rng.next();
                    }
                    mat[r * 64 + c] = ((val >> (4 * shift)) & 0x0f) as u8;
                }
            }
            if matrix_rank_64(&mat) == 64 {
                return mat;
            }
        }
    }

    fn matrix_rank_64(mat: &[u8; 4096]) -> usize {
        const EPS: f64 = 1e-9;
        let mut a = [[0f64; 64]; 64];
        for r in 0..64 {
            for c in 0..64 {
                a[r][c] = mat[r * 64 + c] as f64;
            }
        }
        let mut rank = 0usize;
        let mut selected = [false; 64];
        for i in 0..64 {
            let mut j = 0usize;
            while j < 64 && (selected[j] || a[j][i].abs() <= EPS) {
                j += 1;
            }
            if j == 64 {
                continue;
            }
            rank += 1;
            selected[j] = true;
            for p in (i + 1)..64 {
                a[j][p] /= a[j][i];
            }
            for k in 0..64 {
                if k != j && a[k][i].abs() > EPS {
                    for p in (i + 1)..64 {
                        a[k][p] -= a[j][p] * a[k][i];
                    }
                }
            }
        }
        rank
    }

    #[inline(always)]
    fn heavy_hash(matrix: &[u8; 4096], hash: [u64; 4]) -> Uint256 {
        let mut bytes = [0u8; 32];
        for (i, word) in hash.iter().enumerate() {
            bytes[i * 8..i * 8 + 8].copy_from_slice(&word.to_le_bytes());
        }

        let mut vec = [0u8; 64];
        for i in 0..32 {
            vec[2 * i] = bytes[i] >> 4;
            vec[2 * i + 1] = bytes[i] & 0x0f;
        }

        let mut product = [0u8; 32];
        for i in 0..32 {
            let mut s0 = 0u32;
            let mut s1 = 0u32;
            for j in 0..64 {
                s0 += matrix[(2 * i) * 64 + j] as u32 * vec[j] as u32;
                s1 += matrix[(2 * i + 1) * 64 + j] as u32 * vec[j] as u32;
            }
            product[i] = (((s0 >> 10) << 4) | (s1 >> 10)) as u8;
            product[i] ^= bytes[i];
        }

        let mut st = HEAVY_INITIAL_STATE;
        for i in 0..4 {
            st[i] ^= u64::from_le_bytes(product[i * 8..i * 8 + 8].try_into().unwrap());
        }
        keccak_f1600(&mut st);
        Uint256([st[0], st[1], st[2], st[3]])
    }


    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn matrix_matches_kaspa_miner_reference_prefix() {
            let word = u64::from_le_bytes([42u8; 8]);
            let m = generate_matrix([word; 4]);
            let expected = [
                4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5,
                15, 3, 15, 3, 15, 3, 15, 3, 15, 3, 15, 3, 15, 3, 15, 3,
                2, 10, 2, 10, 2, 10, 2, 10, 2, 10, 2, 10, 2, 10, 2, 10,
                14, 1, 2, 2, 14, 10, 4, 12, 4, 12, 10, 10, 10, 10, 10, 10,
            ];
            assert_eq!(&m[..64], &expected);
            assert_eq!(matrix_rank_64(&m), 64);
        }

        #[test]
        fn u256_order_is_little_endian_words() {
            assert!(Uint256([9, 0, 0, 0]) < Uint256([10, 0, 0, 0]));
            assert!(Uint256([u64::MAX, 0, 0, 0]) < Uint256([0, 1, 0, 0]));
        }

        #[test]
        fn nonce_masking_matches_stratum_model() {
            let j = Job::new("x".into(), [1, 2, 3, 4], 5, Uint256([u64::MAX; 4]), 0xffff, 0xabcd_0000);
            assert_eq!(j.nonce_for_counter(0x12345), 0xabcd_2345);
        }
    }
}

mod ascend {
    use crate::pow::Job;
    use libloading::Library;
    use std::io;
    use std::path::Path;

    type InitFn = unsafe extern "C" fn(i32) -> i32;
    type SetJobFn = unsafe extern "C" fn(*const u64, *const i8, *const u64) -> i32;
    type ScanFn = unsafe extern "C" fn(u64, u64, u64, *mut u64, *mut u64) -> i32;
    type WorkloadFn = unsafe extern "C" fn() -> u32;
    type FinalizeFn = unsafe extern "C" fn();

    pub struct AscendMiner {
        _lib: Library,
        set_job: SetJobFn,
        scan: ScanFn,
        workload: WorkloadFn,
        finalize: FinalizeFn,
    }

    impl AscendMiner {
        pub fn load<P: AsRef<Path>>(path: P, device: i32) -> io::Result<Self> {
            let path = path.as_ref();
            let lib = unsafe { Library::new(path) }
                .map_err(|e| io::Error::new(io::ErrorKind::NotFound, format!("load {}: {e}", path.display())))?;

            let init: InitFn = unsafe {
                *lib.get::<InitFn>(b"ascend_kas_init\0")
                    .map_err(symbol_err("ascend_kas_init"))?
            };
            let set_job: SetJobFn = unsafe {
                *lib.get::<SetJobFn>(b"ascend_kas_set_job\0")
                    .map_err(symbol_err("ascend_kas_set_job"))?
            };
            let scan: ScanFn = unsafe {
                *lib.get::<ScanFn>(b"ascend_kas_scan\0")
                    .map_err(symbol_err("ascend_kas_scan"))?
            };
            let workload: WorkloadFn = unsafe {
                *lib.get::<WorkloadFn>(b"ascend_kas_workload\0")
                    .map_err(symbol_err("ascend_kas_workload"))?
            };
            let finalize: FinalizeFn = unsafe {
                *lib.get::<FinalizeFn>(b"ascend_kas_finalize\0")
                    .map_err(symbol_err("ascend_kas_finalize"))?
            };

            let rc = unsafe { init(device) };
            if rc != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("ascend_kas_init({device}) failed with ACL/CANN code {rc}"),
                ));
            }

            Ok(Self { _lib: lib, set_job, scan, workload, finalize })
        }

        pub fn set_job(&mut self, job: &Job) -> io::Result<()> {
            let rc = unsafe {
                (self.set_job)(
                    job.initial_state.as_ptr(),
                    job.matrix.as_ptr().cast::<i8>(),
                    job.target.0.as_ptr(),
                )
            };
            if rc != 0 {
                return Err(io::Error::new(io::ErrorKind::Other, format!("ascend_kas_set_job failed: {rc}")));
            }
            Ok(())
        }

        pub fn scan(&mut self, nonce_base: u64, mask: u64, fixed: u64) -> io::Result<(Option<u64>, u64)> {
            let mut nonce = 0u64;
            let mut hashes = 0u64;
            let rc = unsafe { (self.scan)(nonce_base, mask, fixed, &mut nonce, &mut hashes) };
            if rc < 0 {
                return Err(io::Error::new(io::ErrorKind::Other, format!("ascend_kas_scan failed: {rc}")));
            }
            Ok(((rc == 1).then_some(nonce), hashes))
        }

        pub fn workload(&self) -> u32 {
            unsafe { (self.workload)() }
        }


    }

    impl Drop for AscendMiner {
        fn drop(&mut self) {
            unsafe { (self.finalize)() }
        }
    }

    fn symbol_err(name: &'static str) -> impl FnOnce(libloading::Error) -> io::Error {
        move |e| io::Error::new(io::ErrorKind::InvalidData, format!("missing {name}: {e}"))
    }
}

mod miner {
    use crate::ascend::AscendMiner;
    use crate::pow::Job;
    use log::{debug, error, info};
    use rand::{thread_rng, RngCore};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, RwLock};
    use std::thread;
    use std::time::{Duration, Instant};
    use tokio::sync::mpsc::UnboundedSender;

    #[derive(Clone)]
    pub struct PublishedJob {
        pub generation: u64,
        pub job: Arc<Job>,
    }

    #[derive(Default)]
    pub struct JobSlot {
        generation: AtomicU64,
        inner: RwLock<Option<PublishedJob>>,
    }

    impl JobSlot {
        pub fn publish(&self, job: Job) -> u64 {
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            *self.inner.write().expect("job slot poisoned") = Some(PublishedJob { generation, job: Arc::new(job) });
            generation
        }

        pub fn clear(&self) {
            self.generation.fetch_add(1, Ordering::SeqCst);
            *self.inner.write().expect("job slot poisoned") = None;
        }

        pub fn snapshot(&self) -> Option<PublishedJob> {
            self.inner.read().expect("job slot poisoned").clone()
        }

        #[inline(always)]
        pub fn current_generation(&self) -> u64 {
            self.generation.load(Ordering::Acquire)
        }
    }

    #[derive(Clone, Debug)]
    pub struct FoundShare {
        pub generation: u64,
        pub job_id: String,
        pub nonce: u64,
    }

    pub struct MinerThread {
        stop: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl MinerThread {
        pub fn spawn(
            device: i32,
            kernel_lib: PathBuf,
            jobs: Arc<JobSlot>,
            share_tx: UnboundedSender<FoundShare>,
            cpu_verify: bool,
        ) -> Self {
            let stop = Arc::new(AtomicBool::new(false));
            let stop2 = stop.clone();
            let handle = thread::spawn(move || {
                if let Err(e) = run_miner(device, kernel_lib, jobs, share_tx, cpu_verify, &stop2) {
                    error!("Ascend miner thread stopped: {e}");
                }
            });
            Self { stop, handle: Some(handle) }
        }
    }

    impl Drop for MinerThread {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn run_miner(
        device: i32,
        kernel_lib: PathBuf,
        jobs: Arc<JobSlot>,
        share_tx: UnboundedSender<FoundShare>,
        cpu_verify: bool,
        stop: &AtomicBool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut npu = AscendMiner::load(&kernel_lib, device)?;
        let workload = npu.workload() as u64;
        if workload == 0 {
            return Err("kernel returned zero workload".into());
        }
        info!(
            "Ascend device {} ready via {} ({} nonces/batch)",
            device,
            kernel_lib.display(),
            workload
        );

        let mut loaded_generation = 0u64;
        let mut current: Option<PublishedJob> = None;
        let mut counter = thread_rng().next_u64();
        let mut hashes_window = 0u64;
        let mut window_started = Instant::now();

        while !stop.load(Ordering::Relaxed) {
            let snapshot = jobs.snapshot();
            match snapshot {
                Some(ref published) if published.generation != loaded_generation => {
                    npu.set_job(&published.job)?;
                    loaded_generation = published.generation;
                    current = Some(published.clone());
                    counter = thread_rng().next_u64();
                    info!(
                        "NPU loaded job {} gen={} target=0x{}",
                        published.job.id,
                        loaded_generation,
                        hex::encode(published.job.target.to_be_bytes())
                    );
                }
                None => {
                    current = None;
                    loaded_generation = 0;
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
                _ => {}
            }

            let Some(ref published) = current else {
                thread::sleep(Duration::from_millis(10));
                continue;
            };

            // Fast stale-job check: the generation is already maintained in an
            // AtomicU64, so do not take a second RwLock on every NPU batch.
            if jobs.current_generation() != published.generation {
                continue;
            }

            let (found, done) = npu.scan(counter, published.job.nonce_mask, published.job.nonce_fixed)?;
            hashes_window = hashes_window.saturating_add(done);
            counter = counter.wrapping_add(done.max(workload));

            if let Some(nonce) = found {
                if cpu_verify {
                    let pow = published.job.calculate_pow(nonce);
                    if pow > published.job.target {
                        error!(
                            "NPU returned invalid nonce {:016x} for job {}; CPU pow=0x{} target=0x{}",
                            nonce,
                            published.job.id,
                            hex::encode(pow.to_be_bytes()),
                            hex::encode(published.job.target.to_be_bytes())
                        );
                        continue;
                    }
                    info!(
                        "share candidate job={} nonce={:016x} pow=0x{}",
                        published.job.id,
                        nonce,
                        hex::encode(pow.to_be_bytes())
                    );
                } else {
                    // The previous implementation still calculated the complete
                    // CPU PoW even when --no-cpu-verify was selected. Honor the
                    // flag literally and keep the rare share path off the CPU.
                    info!(
                        "share candidate job={} nonce={:016x} (CPU verification disabled)",
                        published.job.id,
                        nonce
                    );
                }

                let share = FoundShare {
                    generation: published.generation,
                    job_id: published.job.id.clone(),
                    nonce,
                };
                if share_tx.send(share).is_err() {
                    return Ok(());
                }
            }

            let elapsed = window_started.elapsed();
            if elapsed >= Duration::from_secs(10) {
                let hps = hashes_window as f64 / elapsed.as_secs_f64();
                info!("device {} hashrate {:.3} MH/s", device, hps / 1_000_000.0);
                hashes_window = 0;
                window_started = Instant::now();
            } else {
                debug!("scan {} hashes", done);
            }
        }
        Ok(())
    }
}

mod stratum {
    use crate::miner::{FoundShare, JobSlot};
    use crate::pow::Job;
    use crate::target::{target_from_difficulty, Uint256};
    use log::{info, warn};
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::error::Error;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;
    use tokio::sync::mpsc::UnboundedReceiver;

    pub type DynError = Box<dyn Error + Send + Sync>;

    #[derive(Clone, Debug)]
    struct Template {
        id: String,
        pre_pow_hash: [u64; 4],
        timestamp: u64,
    }

    #[derive(Debug, Default)]
    struct ShareStats {
        accepted: u64,
        stale: u64,
        low_diff: u64,
        duplicate: u64,
        rejected: u64,
        pending: HashMap<u64, String>,
    }

    pub struct StratumSession {
        address: String,
        wallet: String,
        jobs: Arc<JobSlot>,
        target: Uint256,
        nonce_mask: u64,
        nonce_fixed: u64,
        last_template: Option<Template>,
        next_id: u64,
        stats: ShareStats,
    }

    impl StratumSession {
        pub fn new(address: String, wallet: String, jobs: Arc<JobSlot>) -> Self {
            Self {
                address,
                wallet,
                jobs,
                target: Uint256::ZERO,
                nonce_mask: u64::MAX,
                nonce_fixed: 0,
                last_template: None,
                next_id: 1,
                stats: ShareStats::default(),
            }
        }

        pub async fn run(&mut self, shares: &mut UnboundedReceiver<FoundShare>) -> Result<(), DynError> {
            let endpoint = self.address.strip_prefix("stratum+tcp://").unwrap_or(&self.address);
            info!("connecting to stratum {}", endpoint);
            let stream = TcpStream::connect(endpoint).await?;
            stream.set_nodelay(true)?;
            let (read_half, mut write_half) = stream.into_split();
            let mut lines = BufReader::new(read_half).lines();

            let subscribe_id = self.next_id();
            send_json(
                &mut write_half,
                &json!({"id": subscribe_id, "method": "mining.subscribe", "params": [format!("ascend_kas/{}", env!("CARGO_PKG_VERSION"))]}),
            )
            .await?;
            let auth_id = self.next_id();
            send_json(
                &mut write_half,
                &json!({"id": auth_id, "method": "mining.authorize", "params": [self.wallet.clone(), "x"]}),
            )
            .await?;

            let mut stats_tick = tokio::time::interval(Duration::from_secs(30));
            stats_tick.tick().await;

            loop {
                tokio::select! {
                    line = lines.next_line() => {
                        match line? {
                            Some(line) => self.handle_line(&line).await?,
                            None => return Err("stratum connection closed".into()),
                        }
                    }
                    Some(share) = shares.recv() => {
                        let id = self.next_id();
                        self.stats.pending.insert(id, share.job_id.clone());
                        let req = json!({
                            "id": id,
                            "method": "mining.submit",
                            "params": [self.wallet.clone(), share.job_id, format!("{:016x}", share.nonce)]
                        });
                        info!("submitting share nonce={:016x} gen={}", share.nonce, share.generation);
                        send_json(&mut write_half, &req).await?;
                    }
                    _ = stats_tick.tick() => {
                        info!(
                            "shares accepted={} stale={} low_diff={} duplicate={} rejected={} pending={}",
                            self.stats.accepted,
                            self.stats.stale,
                            self.stats.low_diff,
                            self.stats.duplicate,
                            self.stats.rejected,
                            self.stats.pending.len()
                        );
                    }
                }
            }
        }

        async fn handle_line(&mut self, line: &str) -> Result<(), DynError> {
            let msg: Value = serde_json::from_str(line).map_err(|e| format!("invalid stratum JSON: {e}; line={line}"))?;

            if let Some(method) = msg.get("method").and_then(Value::as_str) {
                let params = msg.get("params").cloned().unwrap_or_else(|| json!([]));
                match method {
                    "mining.set_difficulty" => {
                        let difficulty = params.get(0).and_then(Value::as_f64).ok_or("mining.set_difficulty missing difficulty")? as f32;
                        self.target = target_from_difficulty(difficulty).map_err(|e| format!("difficulty conversion: {e}"))?;
                        info!("difficulty={} target=0x{}", difficulty, hex::encode(self.target.to_be_bytes()));
                        self.republish();
                    }
                    "mining.set_extranonce" | "set_extranonce" => {
                        let extra = params.get(0).and_then(Value::as_str).unwrap_or("");
                        let nonce_size = params.get(1).and_then(Value::as_u64).unwrap_or(8) as u32;
                        self.set_extranonce(extra, nonce_size)?;
                        self.republish();
                    }
                    "mining.notify" => {
                        let arr = params.as_array().ok_or("mining.notify params is not an array")?;
                        if arr.len() < 3 {
                            return Err("mining.notify short form needs [job_id, pre_pow_hash, timestamp]".into());
                        }
                        let id = arr[0].as_str().ok_or("job id is not a string")?.to_owned();
                        let pre_pow_hash = parse_hash_words(&arr[1])?;
                        let timestamp = arr[2].as_u64().ok_or("timestamp is not u64")?;
                        self.last_template = Some(Template { id, pre_pow_hash, timestamp });
                        self.republish();
                    }
                    other => warn!("ignoring unsupported stratum method {}", other),
                }
                return Ok(());
            }

            let Some(id) = msg.get("id").and_then(Value::as_u64) else {
                warn!("unhandled stratum message: {}", line);
                return Ok(());
            };

            if let Some(error) = msg.get("error").filter(|v| !v.is_null()) {
                let job = self.stats.pending.remove(&id);
                let code = error.get(0).and_then(Value::as_i64).unwrap_or(20);
                let text = error.get(1).and_then(Value::as_str).unwrap_or("unknown stratum error");
                match code {
                    21 => self.stats.stale += 1,
                    22 => self.stats.duplicate += 1,
                    23 => self.stats.low_diff += 1,
                    _ => self.stats.rejected += 1,
                }
                warn!("share/request id={} job={:?} rejected code={} error={}", id, job, code, text);
                return Ok(());
            }

            let result = msg.get("result").cloned().unwrap_or(Value::Null);
            if self.stats.pending.remove(&id).is_some() {
                if result == Value::Bool(true)
                    || result.as_array().and_then(|a| a.first()).and_then(Value::as_bool) == Some(true)
                {
                    self.stats.accepted += 1;
                    info!("share accepted");
                } else {
                    self.stats.rejected += 1;
                    warn!("share rejected: result={}", result);
                }
                return Ok(());
            }

            if let Some(arr) = result.as_array() {
                if arr.len() >= 3 {
                    if let (Some(extra), Some(size)) = (arr[1].as_str(), arr[2].as_u64()) {
                        self.set_extranonce(extra, size as u32)?;
                        info!("subscribed extranonce={} free_nonce_bytes={}", extra, size);
                        self.republish();
                    }
                }
            }
            Ok(())
        }

        fn republish(&self) {
            let Some(t) = self.last_template.as_ref() else { return; };
            if self.target.is_zero() {
                warn!("got job {} before pool difficulty; waiting for mining.set_difficulty", t.id);
                return;
            }
            let job = Job::new(
                t.id.clone(),
                t.pre_pow_hash,
                t.timestamp,
                self.target,
                self.nonce_mask,
                self.nonce_fixed,
            );
            let generation = self.jobs.publish(job);
            info!("published job {} generation={}", t.id, generation);
        }

        fn set_extranonce(&mut self, extra_hex: &str, nonce_size_bytes: u32) -> Result<(), DynError> {
            if nonce_size_bytes >= 8 {
                self.nonce_mask = u64::MAX;
                self.nonce_fixed = 0;
                return Ok(());
            }
            let free_bits = nonce_size_bytes * 8;
            let prefix = if extra_hex.is_empty() { 0 } else { u64::from_str_radix(extra_hex, 16)? };
            self.nonce_mask = if free_bits == 64 { u64::MAX } else { (1u64 << free_bits) - 1 };
            self.nonce_fixed = prefix
                .checked_shl(free_bits)
                .ok_or("extranonce prefix does not fit into 64-bit nonce")?;
            Ok(())
        }

        fn next_id(&mut self) -> u64 {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            id
        }
    }

    async fn send_json<W: AsyncWrite + Unpin>(writer: &mut W, value: &Value) -> Result<(), DynError> {
        let mut s = serde_json::to_vec(value)?;
        s.push(b'\n');
        writer.write_all(&s).await?;
        writer.flush().await?;
        Ok(())
    }

    fn parse_hash_words(value: &Value) -> Result<[u64; 4], DynError> {
        if let Some(arr) = value.as_array() {
            if arr.len() != 4 {
                return Err("pre_pow_hash array must have 4 u64 words".into());
            }
            let mut out = [0u64; 4];
            for (i, v) in arr.iter().enumerate() {
                out[i] = v.as_u64().ok_or("pre_pow_hash array item is not u64")?;
            }
            return Ok(out);
        }

        if let Some(s) = value.as_str() {
            let raw = hex::decode(s.strip_prefix("0x").unwrap_or(s))?;
            if raw.len() != 32 {
                return Err("pre_pow_hash hex string must be 32 bytes".into());
            }
            let mut out = [0u64; 4];
            for i in 0..4 {
                out[i] = u64::from_le_bytes(raw[i * 8..i * 8 + 8].try_into().unwrap());
            }
            return Ok(out);
        }

        Err("unsupported pre_pow_hash encoding".into())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_four_word_hash() {
            assert_eq!(parse_hash_words(&json!([1,2,3,4])).unwrap(), [1,2,3,4]);
        }

        #[test]
        fn extranonce_layout_matches_reference() {
            let jobs = Arc::new(JobSlot::default());
            let mut s = StratumSession::new("x".into(), "w".into(), jobs);
            s.set_extranonce("abcd", 2).unwrap();
            assert_eq!(s.nonce_mask, 0xffff);
            assert_eq!(s.nonce_fixed, 0xabcd_0000);
        }
    }
}

use clap::Parser;
use log::{error, info, warn};
use miner::{JobSlot, MinerThread};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Parser, Debug)]
#[command(name = "ascend_kas", version, about = "Standalone Kaspa Stratum miner for Huawei Ascend 910B")]
struct Opt {
    #[arg(long, env = "KAS_POOL")]
    pool: String,

    #[arg(long, env = "KAS_WALLET")]
    address: String,

    #[arg(long, default_value_t = 0, env = "ASCEND_DEVICE_ID")]
    device: i32,

    #[arg(long, default_value = "build/libascend_kas.so", env = "ASCEND_KAS_LIB")]
    kernel_lib: PathBuf,

    #[arg(long, default_value_t = false)]
    no_cpu_verify: bool,

    #[arg(long, default_value_t = 1000)]
    reconnect_ms: u64,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let opt = Opt::parse();

    info!("ascend_kas {} — standalone Ascend 910B Kaspa miner", env!("CARGO_PKG_VERSION"));
    info!("pool={} address={} device={}", opt.pool, opt.address, opt.device);

    loop {
        let jobs = Arc::new(JobSlot::default());
        let (share_tx, mut share_rx) = mpsc::unbounded_channel();
        let _miner = MinerThread::spawn(
            opt.device,
            opt.kernel_lib.clone(),
            jobs.clone(),
            share_tx,
            !opt.no_cpu_verify,
        );

        let mut session = stratum::StratumSession::new(opt.pool.clone(), opt.address.clone(), jobs.clone());
        tokio::select! {
            r = session.run(&mut share_rx) => {
                jobs.clear();
                match r {
                    Ok(()) => warn!("stratum session ended"),
                    Err(e) => error!("stratum session error: {e}"),
                }
            }
            _ = tokio::signal::ctrl_c() => {
                jobs.clear();
                info!("shutdown requested");
                return Ok(());
            }
        }

        drop(_miner);
        tokio::time::sleep(Duration::from_millis(opt.reconnect_ms)).await;
    }
}
