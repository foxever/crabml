use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

use half::f16;

use super::primitives::gelu_single;
use super::thread_pool::ThreadPool;
use crate::tensor::TensorMetrics;

#[derive(Debug, Clone)]
pub struct CpuTensorDeviceOptions {
    /// when enabled, whenever tensor called with `with_name`, the name and the
    /// tensor will be recorded in the device. only used in test.
    pub debug_named_tensors: bool,

    pub metrics: TensorMetrics,

    pub thread_num: usize,
}

impl Default for CpuTensorDeviceOptions {
    fn default() -> Self {
        Self {
            debug_named_tensors: false,
            metrics: TensorMetrics::default(),
            thread_num: 1,
        }
    }
}

impl CpuTensorDeviceOptions {
    pub fn with_thread_num(mut self, thread_num: usize) -> Self {
        self.thread_num = thread_num;
        self
    }

    pub fn with_debug_named_tensors(mut self, debug_named_tensors: bool) -> Self {
        self.debug_named_tensors = debug_named_tensors;
        self
    }

    pub fn with_metrics(mut self, metrics: TensorMetrics) -> Self {
        self.metrics = metrics;
        self
    }
}

#[derive(Debug)]
pub struct CpuTensorDevice<'a> {
    pub(crate) opts: CpuTensorDeviceOptions,
    pub(crate) metrics: TensorMetrics,
    pub(crate) exp_cache: Arc<Vec<f16>>,
    pub(crate) gelu_cache: OnceLock<Vec<f16>>,
    pub(crate) thread_pool: Mutex<ThreadPool>,
    _phantom: std::marker::PhantomData<&'a ()>,
    pub(crate) debug_tensors: Mutex<HashMap<String, Vec<f32>>>,
}

pub type CpuTensorDeviceRef<'a> = Arc<CpuTensorDevice<'a>>;

impl<'a> CpuTensorDevice<'a> {
    pub fn new() -> CpuTensorDeviceRef<'a> {
        let opts = CpuTensorDeviceOptions::default();
        Self::with_options(opts)
    }

    pub fn with_options(opts: CpuTensorDeviceOptions) -> CpuTensorDeviceRef<'a> {
        let metrics = opts.metrics.clone();
        let thread_pool = Mutex::new(ThreadPool::new(opts.thread_num));
        let device = Self {
            opts,
            metrics,
            thread_pool,
            exp_cache: Arc::new(Self::init_exp_cache()),
            gelu_cache: OnceLock::new(),
            _phantom: std::marker::PhantomData,
            debug_tensors: Mutex::new(HashMap::new()),
        };
        Arc::new(device)
    }

    pub fn metrics(&self) -> &TensorMetrics {
        &self.metrics
    }

    pub fn thread_num(&self) -> usize {
        self.opts.thread_num
    }

    pub fn thread_pool(&self) -> &Mutex<ThreadPool> {
        &self.thread_pool
    }

    pub fn dump_debug_tensor(&self, name: &str) -> Option<Vec<f32>> {
        self.debug_tensors.lock().unwrap().get(name).cloned()
    }

    pub fn exp_cache(&self) -> Arc<Vec<f16>> {
        self.exp_cache.clone()
    }

    pub fn gelu_cache(&self) -> &Vec<f16> {
        self.gelu_cache.get_or_init(Self::init_gelu_cache)
    }

    fn init_exp_cache() -> Vec<f16> {
        (0..65536)
            .map(|x| {
                let exp32 = f16::from_bits(x as u16).to_f32().exp();
                f16::from_f32(exp32)
            })
            .collect()
    }

    fn init_gelu_cache() -> Vec<f16> {
        (0..65536)
            .map(|x| {
                let v = f16::from_bits(x as u16).to_f32();
                f16::from_f32(gelu_single(v))
            })
            .collect()
    }

    pub(crate) fn add_debug_tensor(&self, tensor: &super::CpuTensor<'a>) {
        let buf = tensor.buf().iter_f32().collect::<Vec<_>>();
        self.debug_tensors
            .lock()
            .unwrap()
            .insert(tensor.name.clone().unwrap(), buf);
    }
}
