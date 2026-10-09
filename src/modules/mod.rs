use std::collections::HashMap;

use anyhow::Result;

use crate::interceptor::Interceptor;

mod ccp_blocker;
pub mod fps;
mod http;
mod misc;
mod security;
mod winhttp;

pub use ccp_blocker::CcpBlocker;
pub use fps::Fps;
pub use http::Http;
pub use misc::Misc;
pub use security::Security;
pub use winhttp::WinHttp;

#[derive(Default)]
pub struct ModuleManager {
    modules: HashMap<ModuleType, Box<dyn MhyModule>>,
}
unsafe impl Sync for ModuleManager {}
unsafe impl Send for ModuleManager {}

impl ModuleManager {
    pub unsafe fn enable(&mut self, module: impl MhyModule + 'static) {
        let mut boxed_module = Box::new(module);
        boxed_module.init().unwrap();
        self.modules
            .insert(boxed_module.get_module_type(), boxed_module);
    }

    #[allow(dead_code)]
    pub unsafe fn disable(&mut self, module_type: ModuleType) {
        let module = self.modules.remove(&module_type);
        if let Some(mut module) = module {
            module.as_mut().de_init().unwrap();
        }
    }
}

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub enum ModuleType {
    Http,
    Security,
    Misc,
    CcpBlocker,
    WinHttp,
    Fps,
}

pub trait MhyModule {
    unsafe fn init(&mut self) -> Result<()>;
    unsafe fn de_init(&mut self) -> Result<()>;
    fn get_module_type(&self) -> ModuleType;
}

pub struct MhyContext<T> {
    pub assembly_name: &'static str,
    pub interceptor: Interceptor,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> MhyContext<T> {
    pub const fn new(assembly_name: &'static str) -> Self {
        Self {
            assembly_name,
            interceptor: Interceptor::new(),
            _phantom: std::marker::PhantomData,
        }
    }
}