use std::{time::{Instant, Duration}};

#[derive(std::fmt::Debug)]
pub struct Bench {
    instant: Instant,
    durations: [u64; 256],
}

#[allow(dead_code)]
impl Bench {
    pub fn new() -> Self {
        Self {
            instant: Instant::now(),
            durations: [0; 256],
        }
    }

    pub fn combine(&mut self, other: &Bench) {
        for (i, v) in other.durations.iter().enumerate() {
            self.durations[i] += v;
        }
    }

    pub fn start(&mut self) {
        self.instant = Instant::now();
    }
    pub fn stop(&mut self, id: u8) {
        self.durations[id as usize] += self.instant.elapsed().as_nanos() as u64;
    }

    pub fn call_closure<F>(&mut self, id: u8, f: F) where F: FnOnce() {
        let start = Instant::now();
        f();
        self.durations[id as usize] += start.elapsed().as_nanos() as u64;
    }
    pub fn call_return_closure<F, T>(&mut self, id: u8, f: F) -> T where F: Fn() -> T {
        let start = Instant::now();
        let res = f();
        self.durations[id as usize] += start.elapsed().as_nanos() as u64;
        res
    }
    pub fn call_return_closure_mut<F, T>(&mut self, id: u8, mut f: F) -> T where F: FnMut() -> T {
        let start = Instant::now();
        let res = f();
        self.durations[id as usize] += start.elapsed().as_nanos() as u64;
        res
    }

    pub fn get_duration(&self, id: u8) -> Duration {
        Duration::from_nanos(self.durations[id as usize])
    }

    pub fn reset(&mut self, id: u8) {
        self.durations[id as usize] = 0;
    }
}

#[macro_export]
macro_rules! combench {
    ( $( $x:expr ),* ) => {
        {
            let mut temp_bench = crate::bench::Bench::new();
            $(
                temp_bench.combine($x);
            )*
            temp_bench
        }
    };
}