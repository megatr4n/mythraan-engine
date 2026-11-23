pub fn scoped_pool<R: Send>(threads: usize, run: impl FnOnce() -> R + Send) -> R {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_scoped(|thread| thread.run(), |pool| pool.install(run))
        .unwrap()
}
